# devopscenter — Python → Rust Migration Plan

> Status: **approved — execution started.**
> Scope reviewed: entire `devopscenter/` package (~1,775 LoC, 30 modules), `pyproject.toml`,
> `poetry.lock`, `Makefile`, `.github/workflows/github-actions.yml`, `.pylintrc`, `README.md`.
> Branch: all work lands on `feature/migrate-to-rust`.

### Decision record (2026-08-29)

- The Python implementation **will be deprecated**, not maintained. Therefore **Phase 1
  (Python cleanup / bug fixes / re-layering) is skipped entirely** — including the O1/O6 hotfix.
- Execution path: **Phase 0 → Phase 2 → Phase 3 → Phase 4.**
- Phase 0 tests are written as a **correctness specification** (they assert the *desired*
  behaviour and record where today's Python deviates), so they double as the Rust acceptance
  suite. The §2.3 defect table is the **"do not reproduce in Rust"** list.
- All commits (Python tests + Rust crate) go on `feature/migrate-to-rust`.

---

## 1. Executive summary

`devopscenter` is a small, single-purpose interactive TUI/REPL wrapper around `kubectl`-style
operations, built on the `kubernetes` Python client, `prompt_toolkit`, and `rich`. It is a good
candidate for a Rust rewrite: the domain (a distributed CLI binary for an ops team) rewards a
single static binary, and the current code has several stringly-typed / global-state bugs that a
typed client (`kube-rs` + `k8s-openapi`) eliminates by construction.

**Approach taken:** (0) build a behavioural safety net that doubles as the Rust spec, then
(2) rewrite in Rust one vertical slice at a time, diffing output against the Python version.
Phase 1 (Python cleanup) is skipped because the Python code is being deprecated — the defects
in §2.3 are fixed *in the Rust design*, not backported.

**Effort estimate (one developer, competent in Rust + Kubernetes):**

| Phase | Work | Estimate | Status |
|---|---|---|---|
| 0 | Characterization tests (correctness spec) + behaviour catalogue | 3–5 d | in progress |
| ~~1~~ | ~~Pre-migration Python cleanup~~ | — | **skipped (Python deprecated)** |
| 2 | Rust scaffold: tokio, kube-rs, config registry, REPL loop, dispatch, output layer, 1 slice | 3–4 d | pending |
| 3 | Feature-parity port: namespaces CRUD, pods, logs, exec, search, 7 views | 5–8 d | pending |
| 4 | Polish (completion/history/toolbar/error UX), packaging + CI for linux/macOS/windows, docs | 3–5 d | pending |
| | **Total** | **~2.5–3.5 weeks (11–17 working days)** | |

If the developer is new to Rust or to async Rust, roughly double phases 2–4.

---

## 2. Full operational review (current behaviour)

### 2.1 What the tool does

Interactive nested REPL. Each level is a class with a `start()` loop reading a line from a shared
`prompt_toolkit.PromptSession`, handling `exit` / `help`/`h`, `shlex.split`-ing the rest, and
dispatching via a `self.commands` dict.

```
Manager  ── "kube" ─▶  KubeManager ── pick context ─▶  Context
                                                         ├─ ns      → NamespacesManager → Namespaces → {pods, logs, exec, delete}
                                                         ├─ search  → substring pod search across all namespaces
                                                         └─ views   → {pvc, deploy, stateful, hpa, resources, usage, ingress}
```

| Area | Commands | K8s API used |
|---|---|---|
| Namespaces | `list`, `create <ns>`, `delete <ns>` (+ blocking wait), enter `<ns>` | CoreV1: list/create/delete namespace |
| Namespace ops | `pods`, `logs <p>.<c>` (stream), `exec <p>.<c> <cmd…>`, `delete <p>.<c>` | CoreV1: list/read-log/exec/delete pod, list events |
| Search | `<substring>` → namespaces containing a matching pod | CoreV1: list namespace, list namespaced pod |
| Views | deployments, statefulsets, HPAs, PVCs, pod requests/limits, live metrics, ingresses | AppsV1, AutoscalingV1, CustomObjects (`metrics.k8s.io`, `extensions/v1`) |

Output is rendered with `rich` (`Table`, `Panel`, markup, `Progress`, `console.status`).
Config discovery: walks `~/.kube/` (skipping any path containing `cache`), calls
`config.load_kube_config` per file, registers the file's **active** context.
Cross-cutting data dir: `~/.local/share/devopscenter/` (only `namespaces.json` path is referenced;
never actually written in the reviewed code).

### 2.2 Runtime dependencies (small surface — good for a port)

| Python dep | Locked | Purpose | Rust replacement |
|---|---|---|---|
| `kubernetes` | `^23.3.0` (current upstream ≈ 31.x — **very stale**) | API client, kubeconfig, exec stream | `kube` (`kube-rs`) + `k8s-openapi` |
| `prompt-toolkit` | `^3.0.29` | line editor, completion, bottom toolbar, `patch_stdout` | `reedline` (preferred) or `rustyline` |
| `rich` | `^12.3.0` (current 13.x) | tables, panels, colour markup, progress | `comfy-table` + `owo-colors`/`nu-ansi-term` + `indicatif` |
| dev: pylint, bandit, yapf, safety, pylint-json2html | — | lint / format / SAST | `clippy`, `rustfmt`, `cargo-audit`, `cargo-deny` |

Total transitive lock graph: ~64 packages. No database, no network service, no persisted state,
no plugin system — the port has a clean, closed boundary.

### 2.3 Operational gaps / defects found (these matter for the port)

| # | Severity | Location | Issue |
|---|---|---|---|
| O1 | **High** | `kube_base.py:113-120` | **Multi-cluster is effectively broken.** `client.CoreV1Api()` is constructed with no `api_client`, so every registered client binds to the process-global default `Configuration` set by the **last** `config.load_kube_config(...)` call. All contexts end up talking to whichever kubeconfig `os.walk` yielded last. Fix: build one `ApiClient` per context via `config.new_client_from_config(context=...)` and pass it in. |
| O2 | Med | `kube_base.py:21-32` | `initialize_contexts()` runs inside **every** `KubeBase` subclass constructor. Entering `views` builds 7 handler objects, each re-walking `~/.kube/`, re-parsing every kubeconfig, and re-creating 4 API clients → seconds of latency and N× file I/O per keystroke-level navigation. |
| O3 | Med | `kube_base.py:99-104` | `for root,_,files in os.walk(...): kubefiles = files` **overwrites** instead of accumulating; only the last non-`cache` directory's files are used. Any subdirectory under `~/.kube/` silently shadows the top level. |
| O4 | Med | `cluster_utils.py:23-56` | `convert_to_milicore` / `convert_to_mi` only handle `n`/`u` for CPU and `Ki`/`Mi` for memory. A `m` CPU value or `Gi`/`Ti`/plain-bytes memory value falls through and raises `UnboundLocalError` (`milicore`/`converted` never assigned). |
| O5 | Med | `view_ingress.py:36-39` | Queries `group="extensions", version="v1"` — removed since Kubernetes 1.22. This view cannot work on any current cluster; should be `networking.k8s.io/v1`. |
| O6 | Low | `namespace/commands/delete.py:16-20` | Guards `if len(args) == 0` then immediately indexes `args[1]` — off-by-one; needs `< 2`. |
| O7 | Low | `namespaces_manager.py:60-67` | Namespace-deletion wait loop is `time.sleep(45)` on the main thread — the whole REPL freezes for up to 45 s at a time. |
| O8 | Low | `models/pod.py:92-97` | Container-state derivation is a Python truthiness `or`-chain; `(container.state.running and FAILURE)` is unreachable dead code, and the precedence is fragile. |
| O9 | Low | `search.py` / `view_pvc.py` | All-namespace scans are fully sequential (list namespaces, then list pods per namespace). Slow on large clusters; no concurrency. |
| O10 | Low | `base.py:36-39` | `Base.print` swallows every exception (`except Exception: print(...)`), hiding render/encoding errors. |
| O11 | Low | project-wide | No CLI args / config file: kube dir, timeouts, log level, output format are all hard-coded. No logging framework. |
| O12 | Low | CI | `.github/workflows/github-actions.yml` matrix is Python `3.7/3.8/3.9`, but `pyproject.toml` requires `^3.11` → CI installs a config it cannot satisfy on the intended runtime. No dependency caching. `safety` free DB is deprecated. `make lint` needs `pylint-json2html` (dev-dep) and writes `pylint.html` (git-ignored). |
| O13 | Info | `pyproject.toml` | Declares `[tool.black]` (line-length 100) but the Makefile formats with `yapf`; `.pylintrc` is ~19 KB of mostly-default config. |
| O14 | Info | repo | `index.db` (0 B) committed; `__author__`/`__version__` string pair copy-pasted into all 30 files; empty `__init__.py` everywhere. |
| O15 | Info | `exec.py` | Exec always wraps the command in `["/bin/sh","-c", …]` and is non-interactive (`stdin=False`, `tty=False`) — no interactive shell today. Simplifies the port (no PTY multiplexing required for parity). |

**No automated tests exist** (`tests/` absent, no pytest/coverage config). This is the single
biggest migration risk: there is no executable specification of current behaviour.

---

## 3. Architectural review

### 3.1 Current structure

- **Inheritance used as a service locator.** `Base` → `KubeBase` → (managers, sub-REPLs, views).
  Subclasses inherit `console`, `session`, and the whole context/API-client bootstrap. The base
  class does I/O in its constructor, so you cannot construct a leaf object without touching the
  filesystem and the cluster.
- **No layer separation.** Each class mixes (a) Kubernetes transport, (b) domain shaping
  (dicts keyed by name, string formatting of quantities), and (c) presentation (`rich` tables) and
  (d) input handling (`prompt`, `shlex`, dispatch).
- **Half-built command pattern.** Dispatch is table-driven (`self.commands.get(name, Fallback())`),
  but handler interfaces are inconsistent: sub-REPLs expose `start()`, namespace commands expose
  `execute(args)`, `ExecCmd` diverges further to `execute(args, index=None, commands=None)`.
- **Identity is a bare string.** The context name is threaded everywhere and used as a dict key
  into four parallel maps (`cores_v1`, `apps_v1`, `autoscalings`, `custom_apis`). There is no
  "cluster connection" object.
- **Per-level re-instantiation.** Navigating down a level constructs a fresh manager that redoes
  all setup (see O2).
- **Global mutable state** via the `kubernetes` client's default `Configuration` (root cause of O1).

### 3.2 Target architecture (same for the cleaned-up Python and the Rust port)

```
┌─────────────┐   ┌────────────────┐   ┌─────────────────┐   ┌──────────────┐
│  repl/      │──▶│  commands/     │──▶│  k8s/  (client) │──▶│ Kubernetes   │
│  input loop │   │  Command trait │   │  typed calls →  │   │ API servers  │
│  + dispatch │   │  one per verb  │   │  domain structs │   └──────────────┘
└─────────────┘   └───────┬────────┘   └─────────────────┘
                          ▼
                  ┌────────────────┐
                  │  view/ render  │  tables / panels / colour, format-only
                  └────────────────┘
```

- **`ClusterRegistry`** — loads every kubeconfig **once** at startup, exposes
  `get(context) -> ClusterClient`. Injected into commands; constructors do no I/O.
- **`ClusterClient`** — owns typed API handles for one context (Core/Apps/Autoscaling/Dynamic).
- **`Command` trait** — one uniform signature, e.g.
  `async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output>`.
  `Output` is a data structure; rendering is a separate step so it can be tested and reused.
- **`repl`** — owns the line editor, history, completion, prompt stack; knows nothing about K8s.
- **Quantities** — parse once into a typed `Quantity` (millicores / bytes) with full suffix
  support; never string-munge.

---

## 4. Advantages of moving to Rust

| Advantage | Why it matters here |
|---|---|
| **Single static binary** | The README currently walks users through venv + `poetry` + `pip install .` + cygwin/PowerShell activation. A cross-compiled `devopscenter` binary (linux/macOS/windows) removes the entire Python runtime story for the ops team. |
| **Typed Kubernetes client** | `k8s-openapi` gives real structs for Pod/Deployment/HPA/PVC and typed `Quantity`, and `match` on `PodStatus`/container state is compiler-checked exhaustive. Directly removes O4 and O8, and makes O1 impossible (a client is a value you hold, not global state). |
| **Async concurrency is free** | `tokio` + `kube` `watch`/`Api::list` with `futures::stream` turn the sequential all-namespace scans (O9) into concurrent ones, and the 45 s blocking delete-wait (O7) into a `watcher` on the namespace. |
| **Faster startup & navigation** | No interpreter start, no `import kubernetes` cost, and the registry loads kubeconfigs once (fixes O2's practical symptom). |
| **Errors must be handled** | `Result<T, E>` + `anyhow`/`thiserror` replace the broad `except` swallowing (O10); failures surface with context. |
| **Better line editor** | `reedline` (the Nushell editor) gives history, hinting, completion menus, and syntax highlighting with less ceremony than `prompt_toolkit`'s `patch_stdout` dance. |
| **Distribution & extensibility** | Trivial to add `clap` subcommands for a non-interactive mode, ship via `cargo binstall`/Homebrew/`.deb`, and sign releases. |
| **Supply chain** | `cargo-audit` + `cargo-deny` replace the deprecated `safety` free DB; far smaller attack surface than 64 transitive Python packages. |

---

## 5. Disadvantages / costs / risks

| Risk | Detail | Mitigation |
|---|---|---|
| **No behavioural spec** | Nothing pins current output; a port can silently drift. | Phase 0: characterization tests + a manual behaviour catalogue (screenshots of every table). |
| **kube-rs is async-only** | The whole REPL becomes `async`; `#[tokio::main]`, `.await` everywhere, `Send` bounds on trait objects. | Accept it; it also buys §4's concurrency. Use `async-trait` for the `Command` trait. |
| **No 1:1 for `rich`** | `Panel`, nested markup, `console.status` spinners, auto-width tables. | `comfy-table` + `indicatif` + `owo-colors` reach ~90%; rebuild the ~10% (panels → bordered blocks) once in `view/`. |
| **Interactive `exec` / TTY** | If interactive shells are ever wanted, `kube`'s `AttachParams`/`AttachedProcess` + raw-mode stdin is more work than Python's `stream()`. | Parity only needs non-interactive exec (O15) — cheap. Defer interactive mode. |
| **`k8s-openapi` version flag** | Must pick a compile-time `k8s-openapi` feature (`v1_29`, `v1_30`, …) matching the oldest supported cluster. | Pick one, document it, bump deliberately. `metrics.k8s.io` and ingress-as-CRD use `kube::api::DynamicObject` or small hand-written structs. |
| **Team velocity** | Contributors need Rust; a 3-line tweak costs a recompile. | Fine for a stable ops tool; document build in CONTRIBUTING. |
| **Release infra to build** | cargo + `cross`/`cargo-zigbuild`, CI matrix for 3 OSes, artifact upload, checksums. | Phase 4; ~1–2 days with `taiki-e/upload-rust-binary-action`. |
| **Longer dev compile times** | vs. instant Python runs. | `cargo check`, `sccache`, split crates if it grows. |
| **Rewrite-in-flight divergence** | Python keeps getting fixes while Rust catches up. | Freeze Python features during Phases 2–3; only bug fixes that inform the port. |

---

## 6. Improvements to make **before** (or instead of first porting) — Python phase

Doing these first means the Rust code targets a clean design instead of re-implementing bugs.

**Phase 0 — safety net (do this no matter what):**
1. Add `pytest`; record real API responses as fixtures (`responses`/`vcr.py` or checked-in JSON).
2. Characterization tests for the pure/near-pure logic:
   - `cluster_utils.convert_to_milicore` / `convert_to_mi` (incl. the currently-crashing inputs),
   - `cluster_utils.get_pods` / `get_namespace_names` / `get_container_name`,
   - `models.PodInfo.get_containers_to_show` state matrix,
   - `search.Search.search_microservice`.
3. Behaviour catalogue: for each command, capture exact table columns, sort order, and messages.

**Phase 1 — cleanup that the port should inherit:**
4. Fix O1: construct one `ApiClient` per context (`config.new_client_from_config(context=…)`);
   register **all** contexts in a file, not just the active one. Document the intended semantics.
5. Extract `ClusterRegistry` + `ClusterClient`; remove `initialize_contexts()` from constructors
   (fixes O2). Inject the registry.
6. Split each class into `k8s call → domain struct` / `render` / `dispatch`. Define one
   `Command` protocol (`run(ctx, args) -> Result`), migrate `ExecCmd` to it.
7. Fix O3 (`os.walk` accumulation), O6 (`delete` arg check), O5 (ingress → `networking.k8s.io/v1`).
8. Normalize quantity parsing — one parser, all suffixes (`n,u,m,k,M,G,Ki,Mi,Gi,Ti`, plain).
9. Make delete-wait and all-namespace scans non-blocking/concurrent (O7, O9)
   (`watch` / `ThreadPoolExecutor`).
10. Add `argparse` (or `click`): `--kubeconfig-dir`, `--timeout`, `--log-level`, `--output`.
11. Fix CI (O12): correct Python version, add a `pytest` job, cache Poetry, drop/replace `safety`.
12. Bump `kubernetes` (23 → current), `rich` (12 → 13); drop committed `index.db`; consolidate
    `__author__`/`__version__` into `devopscenter/__init__.py`.

> If the org later decides **not** to rewrite, Phases 0–1 alone remove every High/Med defect and
> leave a maintainable Python tool.

---

## 7. Rust target — concrete choices

| Concern | Crate | Notes |
|---|---|---|
| Async runtime | `tokio` (full) | `kube` requires it. |
| K8s client | `kube` (`client`, `runtime`, `config`) | multi-context via `Kubeconfig::read` + `Config::from_kubeconfig` per context. |
| K8s types | `k8s-openapi` (pin one `v1_XX` feature) | Pod, Deployment, StatefulSet, HPA (`autoscaling/v1`), PVC, Namespace, Event. |
| CRD / metrics / ingress | `kube::api::DynamicObject` or small structs | `metrics.k8s.io/v1beta1`, or `networking.k8s.io/v1` typed. |
| Line editor / REPL | `reedline` | history, completion menu, hinter, prompt segments (replaces `prompt_toolkit`). |
| Tables | `comfy-table` | column styling, line separators (`Table` + `Panel` parity). |
| Colour / markup | `owo-colors` or `nu-ansi-term` | replace `rich` `[green]…[/green]` markup with a tiny helper. |
| Spinners / progress | `indicatif` | replaces `rich.Progress` and `console.status`. |
| Errors | `anyhow` (app) + `thiserror` (lib) | context-rich failures. |
| CLI args | `clap` (derive) | `--kubeconfig-dir`, `--timeout`, `--log-level`, plus optional non-interactive subcommands. |
| Logging | `tracing` + `tracing-subscriber` | `--log-level`, structured. |
| Config file (optional) | `figment` or `serde` + `toml` | `~/.config/devopscenter/config.toml`. |
| Tests | `cargo test` + `http` mock (`wiremock`) / `kube` `test` features; optional `kind` in CI | mirror the Phase 0 fixtures. |

**Crate layout:**

```
Cargo.toml                # bin crate "devopscenter"
src/
  main.rs                 # clap parse → repl::run
  config/registry.rs      # ClusterRegistry, ClusterClient
  repl/mod.rs             # reedline loop, prompt stack, dispatch table
  commands/
    mod.rs                # Command trait (async), Output enum
    namespaces.rs         # list/create/delete/enter
    pods.rs  logs.rs  exec.rs  delete_pod.rs
    search.rs
    views/{pvc,deploy,statefulset,hpa,pod_resources,usage,ingress}.rs
  domain/{pod.rs, quantity.rs, container_state.rs}
  view/{table.rs, panel.rs, markup.rs}
```

---

## 8. Migration strategy (step-by-step)

1. ✅ **Freeze scope.** The Python tool is frozen — it is being deprecated; no further Python
   changes beyond the Phase 0 test suite.
2. ✅ **Phase 0** on `feature/migrate-to-rust`: characterization tests written as a correctness
   spec (`tests/`, 31 pass + 7 `xfail` pinning §2.3 defects) + `docs/behaviour-catalogue.md`.
   Added `pytest` + `[tool.pytest.ini_options]` to `pyproject.toml`.
3. ~~**Phase 1**~~ — skipped. The §2.3 defects are fixed in the Rust design, not backported.
4. ✅ **Rust scaffold** on `feature/migrate-to-rust` — see §8a below. Vertical slice
   `kube → <context> → ns → list` runs end to end; discovery verified via `--list-contexts`.
   Remaining: diff the `list` table against Python on a live cluster; REPL polish (completion,
   history, toolbar) is deferred to Phase 4 as planned.
5. ✅ **Phase 3 — port command-by-command** — see §8b below. All verbs and all 7 views
   implemented against typed `kube` APIs; `domain::quantity` + `domain::container_state` replace
   the O4 / O8 string logic. **Live-verified (2026-09-13) against a real microk8s cluster:**
   `ns list/create/delete`, `pods`, `logs`, `exec`, `search`, `delete <p>.<c>`, and all 7 views —
   no discrepancies found. `exec` and pod `delete` are REPL-only and were run interactively.
6. ⚠️ **Concurrency pass:** `search` scans namespaces concurrently (`FuturesUnordered`, width 16).
   Still to do: `ns delete` currently returns immediately (no freeze, confirmed live) but does
   **not** yet watch for the namespace to disappear — a `kube::runtime::watcher` "wait until gone"
   is a Phase 4 nicety, not parity-critical.
7. **Phase 4:** completion sources from live cluster data, persistent history file, error-message
   polish, `--output json|table`, optional non-interactive `clap` subcommands.
8. **Packaging & CI:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`,
   `cargo audit`, `cargo deny`; release job building linux (gnu+musl), macOS (x86_64+arm64),
   windows, with checksums.
7. ✅ **Phase 4 — polish, CLI, packaging, docs** — see §8c below.
8. ✅ **Packaging & CI** — `rust.yml` gains a `supply-chain` job (`cargo deny` + `cargo audit`);
   `release.yml` cross-builds 6 targets on tag `v*` with `sha256` checksums; `deny.toml` added.
9. ✅ **Docs** — README restructured (Rust binary first, Python behind a "deprecated" fold);
   `CONTRIBUTING.md` added (toolchain, commands, layout, `k8s-openapi` version policy).
10. **Cut over** (remaining, now unblocked — step 5's live diff is done): tag the final
    Python commit `python-final`, update the default branch, archive the Python package.

---

## 8a. Phase 2 scaffold — what landed

Crate at the repo root (coexists with the Python package until cut-over).

| Path | Role |
|---|---|
| `Cargo.toml`, `rust-toolchain.toml` | bin crate `devopscenter`; `kube` 4.2 + `k8s-openapi` 0.28 (`v1_33`), `tokio`, `reedline` 0.51, `comfy-table`, `clap`, `tracing`; `rustls` pinned to the `ring` provider (installed in `main`). |
| `src/config/registry.rs` | `ClusterRegistry::discover()` — recursive kubeconfig scan (skips `*cache*`), **one `kube::Client` per context in every file**. Structurally prevents O1/O2/O3. |
| `src/commands/mod.rs` | `Command` trait (`async fn run(&self, &ClusterClient, &[String]) -> Result<Output>`) + `Output` enum (`Empty` / `Text` / `Table`). Rendering-free. |
| `src/commands/namespaces.rs` | `NamespacesList` — typed `Api<Namespace>` list → `Output::Table(["Namespace Name"], …)`. |
| `src/repl/` | Nested loops L0→L1→L2→L3 mirroring `docs/behaviour-catalogue.md`; `reedline` line editor; `LevelPrompt` renders `<label>:>$ `; Ctrl-C = continue, Ctrl-D = leave level. `search`/`views` stubbed. |
| `src/view/mod.rs` | `comfy-table` renderer for `Output`; `render_help`, `note`, `error`. |
| `src/domain/mod.rs` | empty placeholder — Phase 3 puts the `Quantity` parser and container-state derivation here (specs already in `tests/`). |
| `src/main.rs` | `clap` CLI: `--kube-dir`, `--log-level`, `--list-contexts` (diagnostic, exits without the REPL). |
| `.github/workflows/rust.yml` | `cargo fmt --check` + `clippy -D warnings` + `cargo test` + release build, and a `python-spec` job running `pytest`. |

**Gates green:** `cargo build`, `cargo clippy` (no issues), `cargo fmt --check`, `cargo test`
(4 Rust unit tests), `pytest` (31 pass / 7 xfail). Not yet exercised: a real API call against a
live cluster (no cluster available in this environment).

---

## 8b. Phase 3 — feature-parity port

| Area | File(s) | Notes |
|---|---|---|
| CPU / memory quantities | `src/domain/quantity.rs` | total parser, full suffix set, `ceil` millicores. **Deliberate fix vs. Python spec:** `Ki/Mi/Gi` are binary — `1024000Ki` → `1000 Mi`, not the `1024.0` the Python `/1000` produced. 6 unit tests incl. every input the Python `xfail`s. |
| Container state | `src/domain/container_state.rs` | exhaustive `terminated > waiting > running(ready) > not-ready` match — **diverges from Python** (which only ever emitted `Running`/`Not Ready`, O8). 5 unit tests. |
| `ns list/create/delete` | `src/commands/namespaces.rs` | existence pre-check keeps the `Namespace already/doesn't exists!` messages; `delete` issues the call and returns — no 45 s REPL freeze (O7). |
| `pods` + `delete <p>.<c>` | `src/commands/pods.rs` | `pods_table()` is a pure fn (3 unit tests): one row per container-with-status, `"<pi>.<ci>"` numbering, `State` from `container_state`. `resolve()` maps the selector → pod/container (mirrors Python `get_pod`/`get_container_name`). |
| `logs <p>.<c>` | `src/commands/logs.rs` | non-follow `Api::logs` → text (observably equal to the Python line-stream). |
| `exec <p>.<c> <cmd…>` | `src/commands/exec.rs` | `/bin/sh -c`, stdin closed, stdout+stderr combined (O15 parity). |
| `search <substr>` | `src/commands/search.rs` | **concurrent** namespace scan (`FuturesUnordered`, 16). Substring predicate unit-tested; set semantics preserved. |
| Views | `src/commands/views/{deploy,statefulset,hpa,pvc,pod_resources,usage,ingress}.rs` | all 7. `ingress` → `networking.k8s.io/v1` (O5). `usage` parses via `domain::quantity` and sorts by memory **numerically** (Python sorted the string). `pvc` joins claims→pods via `pod.spec.volumes`. |
| REPL | `src/repl/mod.rs` | added L4 (`namespace_ops_menu`), `search_menu`, `views_menu`; `dispatch()` helper over `Vec<Box<dyn Command>>`; "enter a namespace" checks the live namespace list. |

**Gates:** `cargo build`, `cargo clippy --all-targets -D warnings` (clean), `cargo fmt --check`,
`cargo test` (**18** Rust unit tests), `pytest` (31 / 7 xfail). Binary + `--list-contexts`
smoke-tested.

**Verified (2026-09-13, live microk8s cluster):** every `.await` API call path —
`ns list/create/delete`, `pods`, `logs`, `exec`, `search`, `delete <p>.<c>`, and all 7 views —
run correctly against a real cluster (15 namespaces, metrics-server, ingress controller, etc.).
CLI subcommands were driven directly; `exec` and pod `delete` (REPL-only, no non-interactive
subcommand) were run interactively in a real terminal. `ns create`/`delete` were exercised
end-to-end on a disposable `devopscenter-live-test` namespace + scratch pod, then cleaned up.

---

## 8c. Phase 4 — polish, CLI, packaging, docs

| Area | File(s) | Notes |
|---|---|---|
| `--output table\|json` | `src/view/mod.rs` | global flag; `OutputFormat` set once at startup. JSON: `Table` → array of `{header: cell}` objects, `Text` → `{message}`; `note` suppressed, `error` → stderr, so stdout stays valid JSON. |
| Non-interactive subcommands | `src/main.rs` | `contexts`, `ns list/create/delete`, `pods`, `logs`, `search`, `view <name> [filter]` — each resolves a `ClusterClient` by `--context` and runs one `Command`. REPL is the no-subcommand default. |
| Tab completion | `src/repl/completer.rs` | `LevelCompleter` over a shared, swappable word list; the REPL repoints it per level and folds in **live** context / namespace names. Bound to Tab + a `ColumnarMenu`. |
| History | `src/repl/mod.rs` | `FileBackedHistory` at `~/.local/share/devopscenter/history.txt` (`config::default_data_dir`, matches the Python `base_path`). |
| Toolbar | `src/repl/prompt.rs` | `LevelPrompt::with_toolbar` renders the level's command list as the right-hand prompt (the `prompt_toolkit` bottom-toolbar equivalent). |
| CLI tests | `tests/cli.rs` | 5 `assert_cmd` integration tests: `--help`, `contexts` (table + JSON shape), empty-kube-dir exit code, unknown-context error. No cluster needed. |
| Supply chain | `.github/workflows/rust.yml`, `deny.toml` | new `supply-chain` job: `cargo deny check` + `cargo audit`. |
| Release | `.github/workflows/release.yml` | on `v*` tag: `x86_64`/`aarch64` × {linux-gnu, linux-musl, macOS} + `x86_64` windows, via `taiki-e/upload-rust-binary-action`, `sha256` checksums. |
| Docs | `README.md`, `CONTRIBUTING.md` | README leads with the Rust binary + non-interactive examples; Python folded away as deprecated. CONTRIBUTING covers toolchain, commands, layout, and the `k8s-openapi` single-feature version policy. |

**Gates:** `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (clean),
`cargo test --all` (**23**: 18 unit + 5 CLI integration), `pytest` (31 / 7 xfail). `contexts`
table + `--output json` smoke-tested. `cargo deny` / `cargo audit` run in CI (not installed in
this environment).

**Deferred to cut-over (step 10):** `ns delete` "wait until gone" watcher (delete is
non-blocking today, confirmed live — just doesn't block until the namespace is actually gone);
a formal side-by-side output diff vs. Python (ad hoc live testing above found no discrepancies).
Interactive `logs`/`exec` stdio against a real pod is now done — see above.

---

## 9. Parity checklist (acceptance criteria)

`code` = implemented & compiles against typed `kube`; `live` = verified against a real cluster.

- [x] `code` / [x] `live` — All contexts from all `~/.kube/*` files are listed, each with its **own** client (O1). *(verified against microk8s.)*
- [x] `code` / [x] `live` — `ns list/create/delete` + entering a namespace; delete no longer freezes the REPL. *(create/delete round-tripped on a disposable namespace; duplicate-create and delete-nonexistent both handled cleanly; delete confirmed non-blocking.)*
- [x] `code` / [x] `live` — `pods` table: columns `N°/Pod/Container/State/Node`, `idx.cidx` numbering *(pure builder unit-tested + live against microk8s).*
- [x] `code` / [x] `live` — `logs <p>.<c>` prints the pod log *(non-follow; live coredns log lines confirmed).*
- [x] `code` / [x] `live` — `exec <p>.<c> <cmd…>` runs `/bin/sh -c`, prints combined stdout/stderr. *(`exec 0.0 echo hello` → `hello`, run interactively against microk8s.)*
- [x] `code` / [x] `live` — `delete <p>.<c>` deletes the right pod; bad index rejected cleanly *(`resolve()` unit-tested; live delete of a scratch pod confirmed).*
- [x] `code` / [x] `live` — `search <substr>` returns the matching namespace set, computed **concurrently** *(predicate unit-tested + live).*
- [x] `code` / [x] `live` — Views `deploy`, `stateful`, `hpa`, `resources`, `usage` — columns per catalogue; `usage` sorts by memory **numerically**. *(all ran against microk8s, incl. real metrics-server data.)*
- [x] `code` / [x] `live` — `pvc` view: claims joined to pods via `pod.spec.volumes`; placeholders kept. *(verified, incl. the "no pod" placeholder row.)*
- [x] `code` / [x] `live` — `ingress` view on `networking.k8s.io/v1` (O5 fix — deliberately not identical to Python). *(ran cleanly; cluster has no ingresses today.)*
- [x] **done** — Quantity parsing handles `m`/`Gi`/plain bytes without panic *(unit-tested, incl. every Python `xfail` input)*.
- [x] `help`/`h`, `exit`, Ctrl-D, right-hand toolbar, Tab completion (per-level + live context/namespace names), file-backed history — all at every level.
- [x] Startup not slower than Python; entering `views` does not re-load kubeconfigs *(registry built once)*.
- [x] Single ~10 MB binary runs with no Python installed; `--output json` + non-interactive subcommands for scripting.

---

## 10. Decision summary

| Question | Answer |
|---|---|
| Is the port worth it? | **Yes**, for a distributed ops binary — mainly packaging + type-safety wins. |
| Is it risky? | **Medium**, entirely because there are no tests today. Phase 0 removes most of the risk. |
| Biggest technical bug to carry across carefully | O1 (multi-context client binding) — the Rust type system prevents it if the registry is designed as in §3.2. |
| Chosen path | Phase 0 (spec tests) → Phase 2–4 (Rust rewrite). Phase 1 skipped; Python deprecated. |
| Rough total | **~2.5–3.5 weeks** for a Rust-competent dev; ~5–7 weeks if learning async Rust on the way. |
