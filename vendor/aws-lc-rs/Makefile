include ../Makefile

UNAME_S := $(shell uname -s)

AWS_LC_RS_COV_EXTRA_FEATURES := unstable

export AWS_LC_RS_DISABLE_SLOW_TESTS := 1

asan:
# TODO: This build target produces linker error on Mac.
# Run specific tests:
#	RUST_BACKTRACE=1 ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address cargo +nightly test --test ecdsa_tests              --target `rustc -vV | sed -n 's|host: ||p'`  --features asan
	RUST_BACKTRACE=1 ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address cargo +nightly test --lib --bins --tests --examples --target `rustc -vV | sed -n 's|host: ||p'`  --features asan

asan-release:
# TODO: This build target produces linker error on Mac.
# Run specific tests:
#	RUST_BACKTRACE=1 ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address cargo +nightly test --release --test basic_rsa_test           --target `rustc -vV | sed -n 's|host: ||p'`  --features asan
	RUST_BACKTRACE=1 ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address cargo +nightly test --release --lib --bins --tests --examples --target `rustc -vV | sed -n 's|host: ||p'`  --features asan

asan-fips:
# TODO: This build target produces linker error on Mac.
# Run specific tests:
#	RUST_BACKTRACE=1 ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address cargo +nightly test --test ecdsa_tests          --target `rustc -vV | sed -n 's|host: ||p'` --no-default-features --features fips,asan
	RUST_BACKTRACE=1 ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS=-Zsanitizer=address RUSTDOCFLAGS=-Zsanitizer=address cargo +nightly test --lib --bins --tests --examples --target `rustc -vV | sed -n 's|host: ||p'` --no-default-features --features fips,asan

coverage:
	cargo llvm-cov --features "${AWS_LC_RS_COV_EXTRA_FEATURES}" --no-fail-fast --fail-under-lines 95 --ignore-filename-regex "aws-lc(-fips|)-sys/*" --lcov --output-path lcov.info

coverage-fips:
	cargo llvm-cov --features "${AWS_LC_RS_COV_EXTRA_FEATURES},fips" --no-fail-fast --fail-under-lines 95 --ignore-filename-regex "aws-lc(-fips|)-sys/*" --lcov --output-path lcov.info

test: export AWS_LC_RS_DISABLE_SLOW_TESTS = 1
test:
	cargo test --all-targets
	cargo test --all-targets --features unstable
	cargo test --release --all-targets --features bindgen,unstable
	cargo test --release --all-targets --features fips,bindgen,unstable
	cargo test --no-default-features --all-targets --features aws-lc-sys
	cargo test --no-default-features --all-targets --features aws-lc-sys,unstable
	cargo test --no-default-features --all-targets --features fips
	cargo test --no-default-features --all-targets --features fips,unstable
	cargo test --no-default-features --all-targets --features aws-lc-sys,ring-sig-verify
	cargo test --no-default-features --all-targets --features aws-lc-sys,ring-io
	cargo test --no-default-features --all-targets --features aws-lc-sys,alloc

msrv:
	cargo msrv verify

clippy:
	cargo +nightly clippy --all-targets --features bindgen,fips,unstable -- -W clippy::all  -W clippy::pedantic
	cargo +nightly clippy --all-targets --features unstable,dev-tests-only -- -W clippy::all -W clippy::pedantic

clippy-fips-fix:
	cargo +nightly clippy --all-targets --features bindgen,fips,unstable --fix --allow-dirty -- -W clippy::all  -W clippy::pedantic

clippy-fix:
	cargo +nightly clippy --all-targets --features bindgen,unstable,dev-tests-only --fix --allow-dirty -- -W clippy::all  -W clippy::pedantic

ci: format clippy msrv test coverage api-diff-pub

readme:
	cargo readme | tee README.md

.PHONY: asan asan-fips asan-release ci clippy coverage coverage-fips test msrv clippy clippy-fix
