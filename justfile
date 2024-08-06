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

serve-fast:
    ./target/server/release/home_flow.exe

release:
    git pull
    trunk build --release
    cargo build --release --no-default-features --target-dir target/server
    sudo systemctl restart home_flow