default:
    trunk build --release
    cargo run --release

web:
    trunk build --release

server:
    cargo run --release

check:
    cargo check --all-targets
    cargo check --all-features --lib --target wasm32-unknown-unknown
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features --  -D warnings -W clippy::all