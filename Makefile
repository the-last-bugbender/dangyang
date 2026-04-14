.PHONY: setup test lint

## Point git at the committed hook scripts
setup:
	git config core.hooksPath .githooks

test:
	cargo test --all-features

lint:
	cargo fmt --check
	cargo clippy -- -D warnings
