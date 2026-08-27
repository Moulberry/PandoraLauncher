#!/usr/bin/env bash

set -ex

rustc --version && cargo --version  # Print version info for debugging
# Coverage testing would go here. However, OpenBSD rust is not built
# with the required tooling for profiling builds.
echo -e "\e[0Ksection_start:`date +%s`:test\r\e[0KRunning cargo nextest"
if [[ "$(uname -s)" = "OpenBSD" || "$(uname -s)" = "NetBSD" ]]; then
	cargo nextest run --workspace --cargo-verbose --tests;
else
	cargo llvm-cov nextest --workspace --cargo-verbose --tests;
fi
echo -e "\e[0Ksection_end:`date +%s`:test\r\e[0K"
