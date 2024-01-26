default:
    cargo run --target-dir target/desktop

build-web:
    trunk build --release

serve:
    cargo run --no-default-features --target-dir target/server

check:
    cargo check --all-targets
    cargo check --all-features --target wasm32-unknown-unknown
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features --  -D warnings -W clippy::all