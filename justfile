default:
    cargo run --target-dir target/desktop

build-web:
    trunk build --release

serve:
    cargo run --no-default-features --target-dir target/server
