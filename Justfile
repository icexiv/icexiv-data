default:
    @just --list

fmt:
    cargo fmt --all
    nix fmt

check:
    cargo fmt --all --check
    cargo check --all-targets --all-features --locked
    cargo check --no-default-features --locked
    cargo check --all-targets --no-default-features --features local --locked
    cargo check --all-targets --no-default-features --features web --locked

test:
    cargo test --all-features --locked --quiet
    cargo test --no-default-features --locked --quiet

check-wasm:
    cargo check --lib --target wasm32-unknown-unknown --locked
    cargo check --lib --target wasm32-unknown-unknown --all-features --locked

build:
    cargo build --release --locked --lib

book:
    notist build

book-serve:
    notist serve

check-all: check test check-wasm

# Runs the isolated native/WASM checks; no server or GPU.
nix-check:
    nix flake check
