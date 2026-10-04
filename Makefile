.PHONY: check rust web integration z3-check
check: rust web
	git diff --check
	python3 scripts/secret-check.py
rust:
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
web:
	pnpm typecheck
	pnpm lint
	pnpm test
	pnpm build
integration:
	cargo test --workspace --tests
z3-check:
	python3 scripts/z3-check.py
	cargo run -p zerant-zcash --example regtest_check
