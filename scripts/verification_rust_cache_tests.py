"""Public command checks for owned Rust build-cache generations."""

import json
import fcntl
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("verification_rust_cache.py")
CARGO_ARTIFACT = "deps/storyos_probe-aaaaaaaaaaaaaaaa.d"


class RustCacheTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "target/verification").mkdir(parents=True)

    def cli(self, *args):
        return subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), *args],
                              capture_output=True, text=True)

    def measured_workset(self):
        workset = self.root / "target/issue-763-workset"
        (workset / "debug").mkdir(parents=True)
        (workset / ".storyos-rust-cache.json").write_text(json.dumps({"version": 1, "id": "measured"}))
        return workset

    def queue_retirement(self, *, pending=False):
        path = self.root / "target/verification/rust-cache.json"
        state = json.loads(path.read_text())
        old = self.root / state["active"]["path"]
        state["retired"] = [state["active"]]
        state["active"] = {"id": "next", "path": "target/rust-cache/gen-next", "warmup": True}
        state["pending_create"] = pending
        new = self.root / state["active"]["path"]
        if not pending:
            new.mkdir(parents=True)
            (new / ".storyos-rust-cache.json").write_text(json.dumps({"version": 1, "id": "next"}))
        path.write_text(json.dumps(state))
        return path, old, new

    def legacy_debug(self):
        debug = self.root / "target/debug"
        (self.root / "target/CACHEDIR.TAG").write_text(
            "Signature: 8a477f597d28d172789f06886806bc55\n")
        for name in (".fingerprint", "build", "deps", "incremental"):
            (debug / name).mkdir(parents=True, exist_ok=True)
        (debug / CARGO_ARTIFACT).write_bytes(b"cache" * 8192)
        for name in (".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"):
            (debug / name).touch()
        return debug

    def test_public_run_adopts_existing_workset_and_records_target(self):
        workset = self.measured_workset()
        (workset / "debug/deps").mkdir(parents=True)
        result = self.cli("run", "--", sys.executable, "-c",
                          "import os; print(os.environ['CARGO_TARGET_DIR'])")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(workset.resolve()))
        state = json.loads((self.root / "target/verification/rust-cache.json").read_text())
        self.assertEqual(state["active"]["path"], "target/issue-763-workset")
        self.assertTrue((workset / ".storyos-rust-cache.json").is_file())

    def test_hardlinked_cache_object_counts_one_allocated_inode(self):
        workset = self.measured_workset()
        first = workset / "debug/first"
        first.write_bytes(b"x" * 8192)
        os.link(first, workset / "debug/second")
        result = self.cli("status")
        self.assertEqual(result.returncode, 0, result.stderr)
        usage = json.loads(result.stdout)["usage"]
        self.assertEqual(usage["logical_bytes"], first.stat().st_size +
                         (workset / ".storyos-rust-cache.json").stat().st_size)
        self.assertEqual(usage["files"], 3)

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_absent_legacy_cache_is_recorded_once(self):
        self.assertEqual(self.cli("status").returncode, 0)
        debug = self.legacy_debug()
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertTrue(debug.exists())
        record = json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
        self.assertEqual(record["reason"], "No historical default debug cache")

    def test_retired_generation_is_recovered_after_interrupted_cleanup(self):
        self.assertEqual(self.cli("status").returncode, 0)
        state_path, old, new = self.queue_retirement()
        result = self.cli("status")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(old.exists())
        self.assertTrue(new.exists())
        self.assertEqual(json.loads(state_path.read_text())["retired"], [])

    def test_process_death_recovers_pending_creation_and_partial_marker(self):
        hook = self.root / "sitecustomize.py"
        hook.write_text("import os\nfrom pathlib import Path\noriginal = Path.replace\n"
                        "def replace(self, target):\n"
                        "    result = original(self, target)\n"
                        "    if str(target).endswith('/rust-cache.json'): os._exit(77)\n"
                        "    return result\nPath.replace = replace\n")
        result = subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "status"],
                                env={**os.environ, "PYTHONPATH": str(self.root), "CARGO_TARGET_DIR": "",
                                     "STORYOS_RUST_CACHE_ROOT": ""})
        self.assertEqual(result.returncode, 77)
        state_path = self.root / "target/verification/rust-cache.json"
        state = json.loads(state_path.read_text())
        self.assertTrue(state["pending_create"])
        self.assertFalse((self.root / state["active"]["path"]).exists())
        hook.unlink()
        self.assertEqual(self.cli("status").returncode, 0)
        _, old, new = self.queue_retirement(pending=True)
        new.mkdir()
        (new / "user-data").write_text("keep")
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertTrue(old.exists())
        (new / "user-data").unlink()
        (new / ".storyos-rust-cache.json").write_text("{")
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(old.exists())

    def test_unknown_content_prevents_retirement(self):
        self.assertEqual(self.cli("status").returncode, 0)
        state_path, old, new = self.queue_retirement()
        (old / "user-data").write_text("keep")
        result = self.cli("status")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(old.exists())
        self.assertTrue(new.exists())
        self.assertEqual(len(json.loads(state_path.read_text())["retired"]), 1)

    def test_explicit_external_target_is_refused(self):
        result = subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "run", "--",
                                 sys.executable, "-c", "pass"], capture_output=True, text=True,
                                env={**os.environ, "STORYOS_RUST_CACHE_ROOT": "",
                                     "CARGO_TARGET_DIR": str(self.root / "other")})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("CARGO_TARGET_DIR", result.stderr)

    def test_linked_workset_is_not_adopted(self):
        source = self.root / "user-data"
        source.mkdir()
        (source / "keep").write_text("keep")
        (self.root / "target/issue-763-workset").symlink_to(source)
        result = self.cli("status")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((source / "keep").read_text(), "keep")

    def test_unmarked_workset_is_not_claimed(self):
        workset = self.root / "target/issue-763-workset"
        (workset / "debug").mkdir(parents=True)
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertFalse((workset / ".storyos-rust-cache.json").exists())

    def test_linked_cache_parent_is_not_used(self):
        user = self.root / "user-data"
        (user / "gen-next").mkdir(parents=True)
        (user / "gen-next/keep").write_text("keep")
        (self.root / "target/rust-cache").symlink_to(user)
        state = {"version": 1, "active": {"id": "next", "path": "target/rust-cache/gen-next",
                                        "warmup": True}, "retired": [], "events": []}
        (self.root / "target/verification/rust-cache.json").write_text(json.dumps(state))
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertEqual((user / "gen-next/keep").read_text(), "keep")

    def test_linked_old_state_temporary_does_not_overwrite_user_data(self):
        user = self.root / "user-data"
        user.write_text("keep")
        legacy_temporary = self.root / "target/verification/rust-cache.tmp"
        legacy_temporary.symlink_to(user)
        result = self.cli("status")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(user.read_text(), "keep")
        self.assertTrue(legacy_temporary.is_symlink())

    def test_changed_quarantine_marker_stops_recovery(self):
        self.assertEqual(self.cli("status").returncode, 0)
        state_path, old, _ = self.queue_retirement()
        state = json.loads(state_path.read_text())
        entry = state["retired"][0]
        entry["quarantine"] = f"target/rust-cache/deleting-{entry['id']}"
        quarantine = self.root / entry["quarantine"]
        old.rename(quarantine)
        state_path.write_text(json.dumps(state))
        marker = quarantine / ".storyos-rust-cache.json"
        marker.write_text(json.dumps({"version": 1, "id": "wrong"}))
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertTrue(quarantine.exists())
        marker.unlink()
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertTrue(quarantine.exists())

    def test_high_water_rotates_once_and_retains_a_cold_generation(self):
        policy = self.root / "docs/agents/verification-policy.json"
        policy.parent.mkdir(parents=True)
        policy.write_text(json.dumps({"rust_cache": {"high_water_bytes": 4096,
                                                     "total_limit_bytes": 16384}}))
        old = self.measured_workset()
        (old / "debug/artifact").write_bytes(b"a" * 8192)
        result = self.cli("status")
        self.assertEqual(result.returncode, 0, result.stderr)
        state = json.loads((self.root / "target/verification/rust-cache.json").read_text())
        self.assertFalse(old.exists())
        self.assertTrue((self.root / state["active"]["path"]).exists())
        self.assertTrue(state["active"]["warmup"])
        self.assertEqual(len(state["events"]), 1)

    def test_necessary_set_over_limit_refuses_without_rotation_storm(self):
        policy = self.root / "docs/agents/verification-policy.json"
        policy.parent.mkdir(parents=True)
        policy.write_text(json.dumps({"rust_cache": {"high_water_bytes": 4096,
                                                     "total_limit_bytes": 8192}}))
        self.assertEqual(self.cli("status").returncode, 0)
        state_path = self.root / "target/verification/rust-cache.json"
        initial = json.loads(state_path.read_text())
        active = self.root / initial["active"]["path"]
        (active / "debug").mkdir()
        (active / "debug/artifact").write_bytes(b"a" * 16384)
        for _ in range(2):
            result = self.cli("status")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("managed limit", result.stderr)
            self.assertEqual(json.loads(state_path.read_text())["active"], initial["active"])
        self.assertTrue(active.exists())

    def test_busy_run_keeps_retired_generation_until_lock_release(self):
        child = "import sys; print('ready', flush=True); sys.stdin.readline()"
        process = subprocess.Popen([sys.executable, str(SCRIPT), "--root", str(self.root),
                                    "run", "--", sys.executable, "-c", child],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(process.stdout.readline(), "ready\n")
            _, old, _ = self.queue_retirement()
            result = self.cli("status")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("busy", result.stderr)
            self.assertTrue(old.exists())
        finally:
            process.stdin.write("\n")
            process.stdin.flush()
            process.communicate(timeout=10)
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(old.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_public_status_retires_only_owned_legacy_debug_and_empty_scratch(self):
        debug = self.legacy_debug()
        scratch = self.root / "target/issue-763-isolated"
        scratch.mkdir()
        protected = self.root / "target/observation/data"
        protected.mkdir(parents=True)
        (protected / "keep").write_text("keep")
        workset = self.measured_workset()
        result = self.cli("status")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(debug.exists())
        self.assertFalse(scratch.exists())
        self.assertTrue(workset.exists())
        self.assertEqual((protected / "keep").read_text(), "keep")
        legacy = json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
        self.assertEqual(legacy["state"], "complete")
        self.assertEqual(legacy["removed_bytes"]["logical_bytes"], len(b"cache" * 8192))
        self.assertEqual(self.cli("status").returncode, 0)

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_nonempty_unknown_scratch_blocks_retirement(self):
        debug = self.legacy_debug()
        scratch = self.root / "target/issue-763-isolated"
        scratch.mkdir()
        (scratch / "user-data").write_text("keep")
        result = self.cli("status")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(debug.exists())
        self.assertEqual((scratch / "user-data").read_text(), "keep")

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_unknown_or_linked_legacy_content_stops_whole_directory_retirement(self):
        debug = self.legacy_debug()
        (debug / "user-notes").write_text("keep")
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertEqual((debug / "user-notes").read_text(), "keep")
        (debug / "user-notes").unlink()
        external = self.root / "external"
        external.write_text("keep")
        (debug / "deps/link").symlink_to(external)
        self.assertNotEqual(self.cli("status").returncode, 0)
        self.assertEqual(external.read_text(), "keep")
        self.assertTrue(debug.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_unknown_nested_content_stops_retirement_in_each_cargo_subtree(self):
        debug = self.legacy_debug()
        paths = ("deps/user-notes", "build/storyos-core-aaaaaaaaaaaaaaaa/user-notes",
                 ".fingerprint/storyos-core-aaaaaaaaaaaaaaaa/user-notes",
                 "incremental/storyos_core-abc123/s-abc-123/user-notes")
        for relative in paths:
            with self.subTest(relative=relative):
                file = debug / relative
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text("keep")
                self.assertNotEqual(self.cli("status").returncode, 0)
                self.assertEqual(file.read_text(), "keep")
                file.unlink()

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_open_file_and_queued_replaced_lock_defer_retirement(self):
        debug = self.legacy_debug()
        child = subprocess.Popen([sys.executable, "-c",
                                  "import sys; f=open(sys.argv[1]); print('ready',flush=True); sys.stdin.readline()",
                                  str(debug / CARGO_ARTIFACT)], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(child.stdout.readline(), "ready\n")
            result = self.cli("status")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(debug.exists())
            self.assertEqual(json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
                             ["deferred"]["reason"], "open Cargo path or lock")
        finally:
            child.stdin.write("\n")
            child.stdin.flush()
            child.communicate(timeout=10)
        lock = debug / ".cargo-lock"
        child = subprocess.Popen([sys.executable, "-c",
                                  "import sys; f=open(sys.argv[1]); print('ready',flush=True); sys.stdin.readline()",
                                  str(lock)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(child.stdout.readline(), "ready\n")
            replacement = debug / ".replacement"
            replacement.touch()
            replacement.replace(lock)
            result = self.cli("status")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(debug.exists())
        finally:
            child.stdin.write("\n")
            child.stdin.flush()
            child.communicate(timeout=10)
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(debug.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS path seal")
    def test_direct_default_path_cannot_enter_quarantine_after_seal(self):
        debug = self.legacy_debug()
        ready = self.root / "ready"
        go = self.root / "go"
        os.mkfifo(ready)
        os.mkfifo(go)
        hook = self.root / "sitecustomize.py"
        hook.write_text("import os\nfrom pathlib import Path\noriginal=Path.chmod\n"
                        "def chmod(self, mode, *args, **kwargs):\n"
                        "    result=original(self, mode, *args, **kwargs)\n"
                        "    if self.name.startswith('deleting-legacy-') and mode==0:\n"
                        "        with open(os.environ['AUDIT_READY'],'w') as f: f.write('ready\\n')\n"
                        "        with open(os.environ['AUDIT_GO']) as f: f.read()\n"
                        "    return result\nPath.chmod=chmod\n")
        env = {**os.environ, "PYTHONPATH": str(self.root), "AUDIT_READY": str(ready),
               "AUDIT_GO": str(go)}
        process = subprocess.Popen([sys.executable, str(SCRIPT), "--root", str(self.root), "status"],
                                   env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            with os.fdopen(os.open(ready, os.O_RDONLY | os.O_NONBLOCK)) as signal:
                readable, _, _ = select.select([signal], [], [], 10)
                self.assertTrue(readable)
                self.assertEqual(signal.readline(), "ready\n")
            record = json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
            quarantine = self.root / record["path"]
            attempted = subprocess.run([sys.executable, "-c", "import sys; open(sys.argv[1])",
                                        str(quarantine / ".cargo-lock")], capture_output=True, text=True)
            self.assertNotEqual(attempted.returncode, 0)
            debug.mkdir()
            (debug / ".cargo-lock").write_text("new default Cargo target")
            with go.open("w") as signal:
                signal.write("go\n")
            stdout, stderr = process.communicate(timeout=15)
            self.assertEqual(process.returncode, 0, stderr + stdout)
            self.assertFalse(quarantine.exists())
            self.assertEqual((debug / ".cargo-lock").read_text(), "new default Cargo target")
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=10)

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_process_death_after_seal_recovers_without_deleting_active_workset(self):
        debug = self.legacy_debug()
        workset = self.measured_workset()
        hook = self.root / "sitecustomize.py"
        hook.write_text("import os\nfrom pathlib import Path\noriginal=Path.chmod\n"
                        "def chmod(self, mode, *args, **kwargs):\n"
                        "    result=original(self, mode, *args, **kwargs)\n"
                        "    if self.name.startswith('deleting-legacy-') and mode==0: os._exit(77)\n"
                        "    return result\nPath.chmod=chmod\n")
        result = subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "status"],
                                env={**os.environ, "PYTHONPATH": str(self.root)})
        self.assertEqual(result.returncode, 77)
        record = json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
        quarantine = self.root / record["path"]
        self.assertFalse(debug.exists())
        self.assertEqual(quarantine.stat().st_mode & 0o777, 0)
        hook.unlink()
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(debug.exists())
        self.assertTrue(workset.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_open_old_directory_descriptor_defers_retirement(self):
        debug = self.legacy_debug()
        child = subprocess.Popen([sys.executable, "-c",
                                  "import os,sys; fd=os.open(sys.argv[1],os.O_RDONLY); "
                                  "print('ready',flush=True); sys.stdin.readline()", str(debug / "deps")],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(child.stdout.readline(), "ready\n")
            result = self.cli("status")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(debug.exists())
        finally:
            child.stdin.write("\n")
            child.stdin.flush()
            child.communicate(timeout=10)
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(debug.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS Cargo lock")
    def test_real_queued_cargo_cannot_overlap_retirement(self):
        debug = self.legacy_debug()
        (self.root / "Cargo.toml").write_text("[package]\nname='storyos-probe'\nversion='0.1.0'\nedition='2021'\n")
        (self.root / "src").mkdir()
        (self.root / "src/main.rs").write_text("fn main() {}\n")
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root,
                       check=True, capture_output=True)
        with (debug / ".cargo-lock").open("a") as held:
            fcntl.flock(held, fcntl.LOCK_EX)
            cargo = subprocess.Popen(["cargo", "build", "--offline"], cwd=self.root,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            try:
                line = cargo.stderr.readline()
                self.assertIn("Blocking waiting for file lock", line)
                result = self.cli("status")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(debug.exists())
                replacement = debug / ".new-cargo-lock"
                replacement.touch()
                replacement.replace(debug / ".cargo-lock")
                result = self.cli("status")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(debug.exists())
            finally:
                fcntl.flock(held, fcntl.LOCK_UN)
                stdout, stderr = cargo.communicate(timeout=60)
                self.assertEqual(cargo.returncode, 0, stderr + stdout)
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(debug.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS historical migration")
    def test_rename_and_partial_delete_interruptions_resume(self):
        debug = self.legacy_debug()
        hook = self.root / "sitecustomize.py"
        hook.write_text("import os\nfrom pathlib import Path\noriginal=Path.rename\n"
                        "def rename(self, target):\n"
                        "    result=original(self,target)\n"
                        "    if str(target).find('deleting-legacy-')>=0: os._exit(77)\n"
                        "    return result\nPath.rename=rename\n")
        result = subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "status"],
                                env={**os.environ, "PYTHONPATH": str(self.root)})
        self.assertEqual(result.returncode, 77)
        record = json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
        quarantine = self.root / record["path"]
        self.assertFalse(debug.exists())
        self.assertTrue(quarantine.exists())
        self.assertNotEqual(quarantine.stat().st_mode & 0o777, 0)
        hook.write_text("import os,shutil\nfrom pathlib import Path\noriginal=shutil.rmtree\n"
                        "def rmtree(path,*args,**kwargs):\n"
                        "    if Path(path).name.startswith('deleting-legacy-'):\n"
                        "        (Path(path)/'deps/storyos_probe-aaaaaaaaaaaaaaaa.d').unlink()\n"
                        "        os._exit(77)\n"
                        "    return original(path,*args,**kwargs)\nshutil.rmtree=rmtree\n")
        result = subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "status"],
                                env={**os.environ, "PYTHONPATH": str(self.root)})
        self.assertEqual(result.returncode, 77)
        self.assertFalse((quarantine / CARGO_ARTIFACT).exists())
        hook.unlink()
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(quarantine.exists())

    @unittest.skipUnless(sys.platform == "darwin", "macOS open-file audit")
    def test_reference_acquired_in_rename_window_keeps_quarantine(self):
        self.legacy_debug()
        ready = self.root / "ready"
        go = self.root / "go"
        os.mkfifo(ready)
        os.mkfifo(go)
        hook = self.root / "sitecustomize.py"
        hook.write_text("import os\nfrom pathlib import Path\noriginal=Path.rename\n"
                        "def rename(self,target):\n"
                        "    result=original(self,target)\n"
                        "    if str(target).find('deleting-legacy-')>=0:\n"
                        "        with open(os.environ['AUDIT_READY'],'w') as f: f.write('ready\\n')\n"
                        "        with open(os.environ['AUDIT_GO']) as f: f.read()\n"
                        "    return result\nPath.rename=rename\n")
        process = subprocess.Popen([sys.executable, str(SCRIPT), "--root", str(self.root), "status"],
                                   env={**os.environ, "PYTHONPATH": str(self.root),
                                        "AUDIT_READY": str(ready), "AUDIT_GO": str(go)},
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        opened = None
        try:
            with os.fdopen(os.open(ready, os.O_RDONLY | os.O_NONBLOCK)) as signal:
                readable, _, _ = select.select([signal], [], [], 10)
                self.assertTrue(readable)
                self.assertEqual(signal.readline(), "ready\n")
            record = json.loads((self.root / "target/verification/legacy-rust-cache.json").read_text())
            quarantine = self.root / record["path"]
            opened = (quarantine / CARGO_ARTIFACT).open("rb")
            with go.open("w") as signal:
                signal.write("go\n")
            stdout, stderr = process.communicate(timeout=15)
            self.assertEqual(process.returncode, 0, stderr + stdout)
            self.assertTrue(quarantine.exists())
            self.assertEqual(quarantine.stat().st_mode & 0o777, 0)
        finally:
            if opened is not None:
                opened.close()
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=10)
        hook.unlink()
        self.assertEqual(self.cli("status").returncode, 0)
        self.assertFalse(quarantine.exists())


if __name__ == "__main__":
    unittest.main()
