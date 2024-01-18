default:
    cargo run --release

build-web:
    trunk build --release

serve:
    cargo run --release --no-default-features

check:
    cargo check --all-targets
    cargo check --all-features --target wasm32-unknown-unknown
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features --  -D warnings -W clippy::all