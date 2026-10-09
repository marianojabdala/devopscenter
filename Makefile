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
# Pushes a release/vX.Y.Z branch off origin/main; open a PR, merge it, then:
# make tag-release VERSION=1.2.3
release:
	@./scripts/release.sh $(if $(VERSION),$(VERSION),$(if $(BUMP),$(BUMP),$(error pass BUMP=patch|minor|major or VERSION=X.Y.Z)))

# make tag-release VERSION=1.2.3   (run after the release/vX.Y.Z PR is merged)
tag-release:
	@./scripts/tag-release.sh $(if $(VERSION),$(VERSION),$(error pass VERSION=X.Y.Z))
