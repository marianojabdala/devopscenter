# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`devopscenter` is a single-binary, interactive terminal REPL for day-to-day Kubernetes work,
inspired by the pwncat UI. You launch it, pick a cluster context, then drill down through nested
prompts (context → namespace → pod → command). It also exposes each verb as a non-interactive
`clap` subcommand for scripting. Originally written in Python, it was rewritten in Rust — see
[`migration.md`](migration.md) for the full rationale, the defects the rewrite fixes, and the
live-verification record.

## Commands

```bash
cargo run --                 # interactive REPL
cargo run -- contexts        # one-shot subcommand (also: ns, pods, logs, search, view)
cargo build --release        # release binary (target/release/devopscenter)

cargo test --all             # unit + CLI integration tests
cargo fmt --all --check      # formatting (CI runs this)
cargo clippy --all-targets -- -D warnings
cargo deny check             # licenses / advisories / bans
cargo audit                  # RUSTSEC advisories
```

`make build` / `make check` / `make format` wrap the same commands (see `Makefile`).

## Runtime prerequisite

On startup the tool scans `~/.kube/` recursively for kubeconfig files (any filename, e.g.
`config_<cluster>`; any path containing `cache` is skipped) and registers **every** context in
every file, each bound to its own `kube::Client` — there is no shared/global client state.
History is kept at `~/.local/share/devopscenter/history.txt`.

## Architecture

### Layout

```
src/
  main.rs              clap CLI: global flags + non-interactive subcommands, REPL is the default
  config/               ClusterRegistry (discover kubeconfigs once) + ClusterClient (typed API handles)
  commands/             Command trait: async fn run(&self, &ClusterClient, &[String]) -> Result<Output>
    namespaces.rs        list / create / delete
    pods.rs               list + delete
    logs.rs  exec.rs  search.rs
    views/                deploy, statefulset, hpa, pvc, pod_resources, usage, ingress
  domain/               typed helpers with no I/O: quantity (CPU/memory parsing), container_state
  repl/                 nested prompt loops (reedline): L0 top → L1 context picker → L2 context →
                         {L3 namespaces → L4 namespace ops | L3 search | L3 views}; completion,
                         file-backed history, right-hand toolbar
  view/                 Output -> table (comfy-table) or JSON
```

### Conventions

- **Commands never print.** They return an `Output` enum (`Empty`/`Text`/`Table`); `src/view`
  renders it. This keeps commands unit-testable and makes `--output json` work everywhere.
- **No fragile string logic.** CPU/memory quantities and container state go through
  `src/domain` — never match on substrings of a Kubernetes field.
- **One `kube::Client` per context**, built once by `ClusterRegistry::discover()` and injected
  into commands; constructors do no I/O. This structurally prevents the multi-cluster binding bug
  the Python version had (documented as O1 in `migration.md`).
- Kubernetes calls return `anyhow::Result`; errors carry context rather than being swallowed.
- `docs/behaviour-catalogue.md` records the observable contract (prompts, columns, messages)
  level by level; deliberate divergences from the original Python behaviour are noted there and
  in `migration.md` §2.3 / §8b.

## CI / release

`.github/workflows/rust.yml` runs fmt/clippy/test plus a `cargo deny` + `cargo audit`
supply-chain job on every push/PR. Tagging `vX.Y.Z` triggers `.github/workflows/release.yml`,
which cross-builds Linux (gnu + musl, x86_64 + aarch64), macOS (x86_64 + aarch64), and Windows
binaries with `sha256` checksums and publishes them as GitHub Release assets.

## Note on history

The original Python implementation (a `prompt_toolkit` + `rich` + `kubernetes` client REPL) has
been removed. `migration.md` is the record of that rewrite: the defects found in the Python code,
the architectural decisions, and the live-cluster verification performed before cutover.
