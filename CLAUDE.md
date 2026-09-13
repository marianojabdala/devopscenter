# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`devopscenter` is an interactive terminal REPL for day-to-day Kubernetes work, inspired by the
pwncat UI. You launch it, pick a cluster context, then drill down through nested prompts
(context → namespace → pod → command). The only top-level module today is `kube`.

## Commands

There is no build step (it's a console script). Use Poetry + the Makefile.

```bash
make deps            # pip install poetry --upgrade && poetry install
poetry run devopscenter      # run the REPL (also: python -m devopscenter)

make format          # yapf -i --recursive devopscenter
make diff            # yapf --diff (check formatting without writing)
make lint_with_text  # pylint text output (use this locally; `make lint` needs pylint-json2html)
make analyze         # safety + bandit security scans
```

There is **no test suite** in this repo (no `tests/`, no pytest config). `make lint` / `make analyze`
are the only automated checks. CI (`.github/workflows/github-actions.yml`) only runs lint + analyze,
and its Python matrix (3.7–3.9) is stale — `pyproject.toml` requires `python ^3.11`.

## Runtime prerequisite

On startup the app scans `~/.kube/` for kubeconfig files (any filename, e.g. `config_<cluster>`;
the `cache` subdir is skipped) and loads every one. Without kubeconfigs there are no contexts and
the REPL is empty. Cross-cutting state (namespaces cache) lives in `~/.local/share/devopscenter/`.

## Architecture

### Nested-REPL / command pattern

Every interactive level is a class with a `start()` method running a `while True` loop that:
reads a line from a shared `prompt_toolkit` `PromptSession`, handles `exit` and `help`/`h`,
`shlex.split`s the rest, and dispatches. Levels:

```
Manager (devops_center.py)      top prompt: "kube"
  └─ KubeManager (kube_manager.py)   pick a context from ~/.kube
      └─ Context (context.py)        commands: ns | search | views
          ├─ NamespacesManager      list | create | delete | <enter a namespace>
          │   └─ Namespaces         commands: pods | logs | exec | delete
          ├─ Search                 substring search for a pod across all namespaces
          └─ CustomViews            read-only cluster reports
```

Dispatch is table-driven: a class fills `self.commands = {name: handler_instance}` and its
`_do_work()` does `self.commands.get(name, Fallback()).start()` (sub-REPL) or `.execute(args)`
(one-shot). Two handler families:
- **Sub-REPLs** subclass `KubeBase`, implement `start()`, and override `_get_label`,
  `_get_cmd_label`, `show_help`, `get_toolbar`.
- **One-shot commands** subclass `BaseCmd` (namespace commands) or `ViewBase` (views) and
  implement `execute(args)`.

### Base classes

- `base.py:Base` — `rich.Console` (`self.print`/`self.log`) + a `PromptSession` + creates the
  `~/.local/share/devopscenter` data dir.
- `kube_base.py:KubeBase(Base)` — its `__init__` calls `initialize_contexts()`, which loads every
  kubeconfig and builds per-context dicts of Kubernetes API clients:
  `cores_v1`, `apps_v1`, `autoscalings`, `custom_apis` (keyed by context name). It also provides
  the generic `start()` loop described above.

  **Gotcha:** every `KubeBase` subclass re-runs the full kubeconfig scan and rebuilds all API
  clients in its constructor. Instantiating handlers (as `_register_commands` does eagerly) repeats
  this work many times. Keep this in mind before adding constructors or new command classes.

### Supporting code

- `modules/kube/models/pod.py:PodInfo` — wraps a k8s pod object (+ its API client) with the
  container/status accessors the views and commands rely on.
- `modules/kube/cluster_utils.py` — pure helpers: `get_pods`, `get_namespace_names`, and unit
  conversions (`convert_to_milicore` nanocore/microcore→millicore, `convert_to_mi` Ki→Mi).
- `modules/kube/views/*` — one class per read-only report (`pvc`, `deploy`, `stateful`, `hpa`,
  `ingress`, `resources`, `pod_resources`, `usage`); all extend `ViewBase`.
- `not_found.py` / `BaseCmd.execute` / `ViewBase.execute` — the "command not found" fallbacks used
  as the default in every `self.commands.get(...)`.

### Conventions

- User-facing output goes through `self.print` / `self.log` with `rich` markup (`[green]...[/green]`).
- Every REPL loop swallows `KeyboardInterrupt` (continue) and `EOFError` (break) — preserve this
  when adding loops.
- Kubernetes calls should catch `kubernetes.client.exceptions.ApiException`.
- `pyproject.toml` sets black/yapf line length 100; formatting is enforced by `make diff`, not tox.

## Note on the current branch

`feature/migrate-to-rust` is checked out, but the codebase is still 100% Python — no Rust/Cargo
files exist yet. Recent history is a refactor of the original single-file tool into the
`modules/kube/...` package layout.
