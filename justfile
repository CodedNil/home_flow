default:
    cargo run --target-dir target/desktop

build-web:
    trunk build --release

serve:
    cargo run --release --no-default-features --target-dir target/server
