"""Public command checks for owned Rust build-cache generations."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("verification_rust_cache.py")


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


if __name__ == "__main__":
    unittest.main()
