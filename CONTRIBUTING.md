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

## Project layout

| Path | Responsibility |
|---|---|
| `src/config/` | `ClusterRegistry` — one `kube::Client` per context, discovered once at startup |
| `src/commands/` | one type per verb, `Command` trait, rendering-free `Output` |
| `src/commands/views/` | the seven read-only reports |
| `src/domain/` | typed helpers: `quantity` (CPU/memory parsing), `container_state` |
| `src/repl/` | nested prompt loops, completion, history — no Kubernetes types |
| `src/view/` | `Output` → table or JSON |
| `docs/behaviour-catalogue.md` | the observable contract, level by level |
| `tests/cli.rs` | non-interactive CLI integration tests |

## Conventions

- **Commands never print.** They return `Output`; `src/view` renders it. This
  keeps them unit-testable and lets `--output json` work everywhere.
- **No fragile string logic.** Quantities and container states go through
  `src/domain`. If you find yourself matching on substrings of a Kubernetes
  field, add a typed helper instead.
- Add a unit test with each new `Command` (mock the data, test the pure parts —
  see `src/commands/pods.rs`).
- User-facing wording stays close to the Python original where
  `docs/behaviour-catalogue.md` records it; deliberate divergences are noted
  there and in `migration.md` §2.3 / §8b.

## `k8s-openapi` version policy

`k8s-openapi` is pinned to a single Kubernetes API feature
(`features = ["v1_33"]` in `Cargo.toml`). This is the **oldest** cluster version
the binary supports. Bump it deliberately in its own commit, updating this note
and the `Cargo.toml` comment; never let `cargo update` change it implicitly.
