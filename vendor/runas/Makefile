.PHONY: build
build:
	@cargo build

.PHONY: test
test:
	@cargo test

.PHONY: docs
docs:
	@cargo doc --no-deps

.PHONY: format
format:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all

.PHONY: format-check
format-check:
	@rustup component add rustfmt 2> /dev/null
	@cargo fmt --all -- --check

.PHONY: lint
lint:
	@rustup component add clippy 2> /dev/null
	@cargo clippy
