all: check

check:
	cargo +stable clippy \
		--target aarch64-apple-darwin \
		--target x86_64-apple-darwin \
		--target x86_64-unknown-linux-gnu \
		--target x86_64-pc-windows-gnu \
		--all-features

fmt:
	cargo +nightly fmt
