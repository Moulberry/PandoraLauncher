#!/usr/bin/env bash

set -ex

if [[ "$(uname -s)" = "Darwin" ]]; then
	PATH="${PATH}:/Users/gitlab/.cargo/bin:/opt/homebrew/opt/rustup/bin/"
fi

if [[ "$(uname -s)" = "Darwin" || "$(uname -s)" = "FreeBSD" ]]; then
	cargo llvm-cov report --cobertura --output-path target/llvm-cov-target/cobertura.xml || :
fi
