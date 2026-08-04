.PHONY: contracts generate-contracts verify

contracts:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-targets --all-features
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-versioned-protocol-route-catalog.py --self-test
	PYTHONDONTWRITEBYTECODE=1 python3 docs/foundation/verify-postgresql-release-1-persistence-catalog.py --self-test
	cargo run --quiet -p storyos-contracts -- check
generate-contracts:
	cargo run --quiet -p storyos-contracts -- generate
verify: contracts
	@cargo metadata --no-deps --format-version 1 | python3 -c 'import json, pathlib, sys; root = pathlib.Path.cwd().resolve(); data = json.load(sys.stdin); manifests = sorted(pathlib.Path(package["manifest_path"]).resolve() for package in data["packages"]); expected = [root / "crates/storyos-contracts/Cargo.toml"]; forbidden = [path for path in manifests if "/prototypes/" in str(path) or "/.reference/" in str(path)]; sys.exit(0 if manifests == expected and not forbidden else f"unexpected workspace manifests: {manifests}; forbidden: {forbidden}")'
