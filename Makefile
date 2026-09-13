PROJECT_NAME=devopscenter

build:
	cargo build --release

check:
	cargo fmt --all --check
	cargo clippy --all-targets -- -D warnings
	cargo test --all

format:
	cargo fmt --all

# make release BUMP=patch|minor|major   (or VERSION=1.2.3)
release:
	@./scripts/release.sh $(if $(VERSION),$(VERSION),$(if $(BUMP),$(BUMP),$(error pass BUMP=patch|minor|major or VERSION=X.Y.Z)))
