# Contributing

`devopscenter` is a Rust binary. It replaced a Python implementation; see
[`migration.md`](migration.md) for the rationale and the defects the rewrite
fixes. All work happens in `src/`.

## Toolchain

- Stable Rust, pinned by [`rust-toolchain.toml`](rust-toolchain.toml)
  (`rustup` picks it up automatically). Minimum: the `rust-version` in
  `Cargo.toml`.
- No system dependencies — TLS is `rustls` + `ring`, statically linked.

## Everyday commands

```bash
cargo run --                       # interactive REPL
cargo run -- contexts              # one-shot subcommand
cargo test --all                   # unit + integration tests
cargo fmt --all                    # format (CI runs --check)
cargo clippy --all-targets -- -D warnings
cargo deny check                   # licenses / advisories / bans
cargo audit                        # RUSTSEC advisories
```

CI (`.github/workflows/rust.yml`) runs all of the above on every push/PR.
Tagging `vX.Y.Z` triggers `release.yml`, which cross-builds Linux
(gnu + musl, x86_64 + aarch64), macOS (x86_64 + aarch64) and Windows binaries
with `sha256` checksums.

## Cutting a release

`Cargo.toml`'s `version` (what `devopscenter --version` prints, via clap's
`version` derive) and the git tag are two separate things — nothing keeps
them in sync automatically. The version bump goes through a normal PR, same
as any other change to `main`; nothing is ever pushed to `main` directly.

```bash
make release BUMP=patch      # 1.2.3 -> 1.2.4
make release BUMP=minor      # 1.2.3 -> 1.3.0
make release BUMP=major      # 1.2.3 -> 2.0.0
make release VERSION=1.2.3   # explicit version
```

This creates a `release/vX.Y.Z` branch off the latest `origin/main`, bumps
`Cargo.toml`, refreshes `Cargo.lock`, commits, and pushes **that branch**
(your local branch is left as it was). Open a PR for it and merge as usual.

Once merged, tag `main`'s new HEAD and push *only* the tag — that's what
triggers `release.yml`:

```bash
make tag-release VERSION=1.2.3
```

## Project layout

| Path | Responsibility |
|---|---|
| `src/config/` | `ClusterRegistry` — one `kube::Client` per context, discovered once at startup |
| `src/commands/` | one type per verb, `Command` trait, rendering-free `Output` |
| `src/commands/views/` | the seven read-only reports |
| `src/domain/` | typed helpers: `quantity` (CPU/memory parsing), `container_state` |
| `src/repl/` | nested prompt loops, completion, history — no Kubernetes types |
| `src/view/` | `Output` → table or JSON |
| `tests/cli.rs` | non-interactive CLI integration tests |

## Conventions

- **Commands never print.** They return `Output`; `src/view` renders it. This
  keeps them unit-testable and lets `--output json` work everywhere.
- **No fragile string logic.** Quantities and container states go through
  `src/domain`. If you find yourself matching on substrings of a Kubernetes
  field, add a typed helper instead.
- Add a unit test with each new `Command` (mock the data, test the pure parts —
  see `src/commands/pods.rs`).

## `k8s-openapi` version policy

`k8s-openapi` is pinned to a single Kubernetes API feature
(`features = ["v1_33"]` in `Cargo.toml`). This is the **oldest** cluster version
the binary supports. Bump it deliberately in its own commit, updating this note
and the `Cargo.toml` comment; never let `cargo update` change it implicitly.
