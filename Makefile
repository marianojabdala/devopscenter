PROJECT_NAME=devopscenter

build:
	cargo build --release

check:
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --all

format:
	cargo fmt --all
