# SPDX-FileCopyrightText: The mpeg-audio-header authors
# SPDX-License-Identifier: CC0-1.0

# just manual: https://github.com/casey/just/#readme

_default:
    @just --list

# Format source code
fmt:
    cargo fmt --all

# Run clippy
check:
    cargo clippy --locked --workspace --no-deps --all-targets -- -D warnings --cap-lints warn

# Run unit tests
test:
    RUST_BACKTRACE=1 cargo test --locked --workspace -- --nocapture

# Set up (and update) tooling
setup:
    # Ignore rustup failures, because not everyone might use it
    rustup self update || true
    # cargo-edit is needed for `cargo upgrade`
    cargo install cargo-edit
    pip install -U pre-commit
    pre-commit autoupdate
    pre-commit install --hook-type commit-msg --hook-type pre-commit

# Upgrade (and update) dependencies
upgrade:
    RUST_BACKTRACE=1 cargo upgrade --workspace
    cargo update

# Run pre-commit hooks
pre-commit:
    pre-commit run --all-files
