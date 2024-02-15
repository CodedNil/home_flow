default:
    cargo run --target-dir target/desktop

build-web:
    trunk build

build-web-release:
    trunk build --release

serve:
    cargo run --no-default-features --target-dir target/server

serve-release:
    cargo run --release --no-default-features --target-dir target/server

servefast:
    ./target/server/release/home_flow.exe