# Behaviour catalogue — devopscenter (Python, pre-migration)

Reference for the Rust port's parity checks (migration.md §9). Captures the
*observable* behaviour of every prompt and command: prompt strings, dispatch
rules, table columns, sort order, and user-facing messages. Derived by reading
the source at the tip of `feature/migrate-to-rust`; where behaviour is buggy it
is marked ⚠ with the defect id from migration.md §2.3 — the Rust port fixes
those rather than copying them.

Legend: `⇥` = word-completion active, `⌃C` = Ctrl-C, `⌃D` = Ctrl-D / EOF.

---

## Global conventions (all prompt levels)

| Behaviour | Detail |
|---|---|
| `exit` | leave the current level (return to parent loop) |
| `help` / `h` | print that level's help table |
| `⌃C` | abort the current line, stay in the loop |
| `⌃D` | leave the current level (top level prints `See You!!!!`) |
| empty line | ignored |
| unknown input | level-specific "not found" message (see each level) |
| parsing | `shlex.split(text)`; `args[0]` is the verb (except top + kube + search levels, which compare the whole trimmed line) |
| output | `rich` markup; colour is cosmetic and not part of parity |

---

## L0 — top level (`Manager`, `devops_center.py`)

- Prompt: `devops_center:>$`
- Greeting on entry: `Welcome to Devops Center !`
- Commands: `help`/`h` → table `[Commands, Description]` with one row `kube — Interact with the cluster`; `kube` → enter L1; `exit` → quit; `⌃D` → print `See You!!!!` and quit.
- Anything else: silently ignored (no error message).

## L1 — context picker (`KubeManager`, `kube_manager.py`)

- Prompt: `kube:>$`
- On entry and on `help`: print table `[Contexts]`, one row per context name.
- Completion: ⇥ over context names.
- Input equal to a known context → enter L2 for that context. Unknown non-empty input → ignored (no message).
- Context list source ⚠ O1/O3: walk `~/.kube/`, skip any path containing `cache`; for each file `config.load_kube_config(file)` then register `list_kube_config_contexts(file)[1]["name"]` (the file's **active** context only). Bugs: every API client ends up bound to the **last** loaded file (O1); only the last non-cache directory's files are scanned (O3). **Rust spec:** enumerate every context in every kubeconfig under the kube dir, each with its own client.

## L2 — context (`Context`, `context.py`)

- Prompt: `(<context>):context:>$`, bottom toolbar `Commands: ns search views`.
- Completion: ⇥ over `ns search views`.
- `help`/`h`: table `[Commands, Description]`:
  - `ns — Interact with namespaces`
  - `search — Look for a microservice into the namespaces`
  - `views — Shows distinct views`
- Dispatch: `ns` → L3; `search` → L3-search; `views` → L3-views. Unknown verb → `Command not found!!!` (via `NotFound`).

## L3 — namespaces (`NamespacesManager`, `namespaces_manager.py`)

- Prompt: `(<context>):namespaces:>$`, bottom toolbar `Commands list - create - delete`.
- Each loop iteration re-lists namespaces (`list_namespace`) and rebuilds ⇥ completion from current namespace names.
- Verbs:
  | Verb | Effect |
  |---|---|
  | `list` | table `[Namespace Name]`, one row per namespace, in API order |
  | `create <ns>` | if not present: `create_namespace`; else print `Namespace already exists!` |
  | `delete <ns>` | if present: `delete_namespace` with `grace_period_seconds=0`, then ⚠ O7 block the REPL, polling every 45 s (`console.status("Terminating namespace")`) until gone; else print `Namespace doesn't exists!`. On `ApiException`: `Somethig went wrong deleting namespace <err>` |
  | `<ns>` (a known namespace) | enter L4 for that namespace |
  | `exit` | back to L2 |
- ⚠ `create`/`delete` with no argument → `IndexError` (`args[1]`). **Rust spec:** print a usage message.

## L4 — namespace ops (`Namespaces`, `namespaces.py`)

- Prompt: `(<context>):<namespace>:>$`, bottom toolbar `Commands: pods logs exec delete`.
- `help`/`h`: table `[Commands, Description, Info]` for `pods`, `logs`, `exec` (note: `delete` is dispatchable but absent from the help table).
- Dispatch via `self.commands.get(verb, BaseCmd()).execute(args)`; unknown verb → `Command not found!!!`.

### L4 `pods` (`pods.py`)
- Lists pods in the namespace (`list_namespaced_pod`, `timeout_seconds=5`).
- Table columns: **`N°`, `Pod`, `Container`, `State`, `Node`**.
- One row per *container* of each pod that has container statuses; row key `N°` = `"<pod_index>.<container_index>"` (both 0-based, in list order).
- `State` ⚠ O8: only ever `Running` (container `ready` truthy) or `Not Ready` (otherwise). `Container` info suffix `-<info>` is always empty here. **Rust spec:** real state from container `state`/`last_state`/phase via exhaustive match; keep the `p.c` numbering.
- Pods with no container statuses produce no rows.
- **Rust divergence:** `pods [filter]` takes an optional trailing substring; pods whose name doesn't contain it (case-insensitive) are hidden from the table. The `N°` index is still computed against the *unfiltered* list, so a filtered row's index still resolves correctly against `logs`/`exec`/`delete`, which always re-list unfiltered. The Python version had no such filter.
- **Rust divergence:** `pods --unhealthy` (or `-u`) replaces the name filter with a health filter: keeps only pods with a non-`Running`/non-cleanly-`Completed` container, or a `Failed` pod, or a pod with no container statuses yet (still scheduling) — the last case gets a synthetic row with no `.container` index (same convention `resolve` gives a container-less selector), since it has no containers to enumerate.

### L4 `logs` (`logs.py`)
- Usage: `logs <pod_index>.<container_index>`. Missing/!`.`-formatted arg → `Error you should select the number of the pod to show the log. Eg logs 0.0`.
- Streams `read_namespaced_pod_log(..., _preload_content=False)` line by line to stdout, decoded UTF-8.
- `⌃C` during stream → prints `Breaking logs` and returns to prompt (does not exit level). `ApiException`/other exceptions are logged and swallowed.
- **Rust divergence:** `logs <selector> -p`/`--previous` shows the log of the container's previous (already-terminated) instance — the single most useful thing for a crash-looping pod, since the *current* instance's log is often empty. No Python equivalent.

### L4 `exec` (`exec.py`)
- Usage: `exec <pod_index>.<container_index> <cmd...>`.
- Runs `["/bin/sh", "-c", <cmd...>]` via `connect_get_namespaced_pod_exec`, `stdin=False, stdout=True, stderr=True, tty=False, _preload_content=True`; prints the returned string. Non-interactive (O15) — no PTY.

### L4 `delete` (`delete.py`)
- Usage: `delete <pod_index>.<anything>` (only the part before `.` is used).
- `delete_namespaced_pod(pod_name, namespace)`. Bad index → `The number is not in the list of pods` (from `BaseCmd.get_pod`). ⚠ O6: guard checks `len(args) == 0` then reads `args[1]`.

### L4 `describe` — Rust-only, no Python equivalent
- Usage: `describe <pod_index>` (a `.<container_index>` suffix is accepted but ignored — always describes every container).
- `kubectl describe pod`-style text: metadata, labels/annotations, owner, status/IP/QoS, one block per container, and conditions.
- Each container block: image, ports, **State** (kind + `Reason`/`Message`/`Exit Code`/`Started`/`Finished` as available — the `Message` is the actual "back-off restarting failed container..." text, not just the `CrashLoopBackOff` reason word), **Last State** (same detail for the previous run, shown only when it carries real data), ready, restart count, requests/limits.
- Trailing **Events** section: every event whose `involvedObject.name` matches this pod (best-effort — a fetch failure, e.g. no RBAC, just omits the section rather than failing the whole command), oldest-first with `Type/Reason/Age/Message`.

### L4 `events` — Rust-only, no Python equivalent
- Usage: `events` (no selector — shows every event in the namespace, not just one pod's).
- Table `Type, Reason, Object, Age, Message`, sorted oldest-first by last-seen time (falls back to first-seen, then `event_time`).

### L4 `rollout` — Rust-only, no Python equivalent
- Usage: `rollout <deployment-name>`.
- Polls the Deployment every 2s (up to ~60s) until `observedGeneration` has caught up and `updated`/`available`/`replicas` all equal the desired count, or times out; either way prints a final status line with the actual counts.

### L4 `summary` — Rust-only, no Python equivalent
- Usage: `summary` (no args). One-shot namespace rollup: pod count + how many are unhealthy (crash looping / pending / failed), deployment count + how many are fully rolled out, statefulset count + how many are ready, service count, and PVC count + how many are bound.

## L3-search (`Search`, `search.py`)

- Prompt: `(<context>)search:>$`.
- Whole trimmed line is the query (not `shlex`-split). `help`/`h` → `You have to add the microservice to search`.
- For a non-empty query: iterate every namespace (`list_namespace`), list its pods, collect namespaces where any `pod_name` **contains** the query as a substring. Shows a transient `rich` progress bar.
- Result: `Namespace: {<set>}` (a Python `set` repr) or `Not found`.
- ⚠ O9 sequential. **Rust spec:** same set semantics, scans run concurrently.

## L3-views (`CustomViews`, `custom_views.py` + `views/*`)

- Prompt: `(<context>):views:>$`.
- Dispatch `self.commands.get(verb, ViewBase()).execute(args)`; unknown verb → `View not found!!!`.
- Every view takes an optional `args[1]` substring filter applied to the resource **name** (`name_to_filter in name`).

| Verb | Source call | Table columns | Sort | Notes |
|---|---|---|---|---|
| `deploy` | `list_deployment_for_all_namespaces(timeout_seconds=60)` | `Cluster, Namespace, Replicas, Name` | API order | `Name` cell is `"<name> (<row_index>)"` |
| `stateful` | `list_stateful_set_for_all_namespaces(timeout_seconds=60)` | `Cluster, Namespace, Statefulset` | API order | `Statefulset` cell `"<name> (<row_index>)"` |
| `hpa` | `list_horizontal_pod_autoscaler_for_all_namespaces` (autoscaling/v1) | `Cluster, Namespace, Hpa name, min replicas, max replicas, current replicas` | API order | name cell `"<name> (<row_index>)"` |
| `pvc` | `list_persistent_volume_claim_for_all_namespaces` + all-cluster pod scan | `Cluster, Namespace, Pod Name, Pvc, Capacity` | dict order | joins PVCs to pods via `pod.spec.volumes[].persistent_volume_claim.claim_name`; placeholders `no namespace` / `sin pod` / `no storage` when unresolved |
| `resources` | all-cluster pod scan (`get_pods`) | `rich` **Panels**, not a table | pod order | one panel per container: title `Namespace: <ns> -Container: <name>`, body `Requests: <dict>\nLimits: <dict>` |
| `usage` | `list_cluster_custom_object(metrics.k8s.io/v1beta1, pods)` | `Namespace, Pod Name, Container Name, Cpu, Memory` | **by memory desc** (string sort ⚠) | `Cpu` via `convert_to_milicore`, `Memory` via `convert_to_mi(...)+"Mi"` — both ⚠ O4 |
| `ingress` | ⚠ O5 `list_cluster_custom_object(group="extensions", version="v1", plural="ingresses")` | `Namespace, Ingress Name, Annotations` | dict order | filters annotations to keys containing `ingress.kubernetes.io`; ⚠ O5b `pretty_annotations` is called with a list and raises `AttributeError`. **Rust spec:** `networking.k8s.io/v1`, render annotation `k:v` lines |
| `nodes` — **Rust-only, no Python equivalent** | `Api::<Node>::all` + `metrics.k8s.io/v1beta1 NodeMetrics` (best-effort — missing metrics-server shows `-`, not an error) | `Name, Status, Roles, Age, Version, Cpu%, Mem%` | API order | `Status` is the `Ready` condition plus any other condition currently `True` appended (e.g. `NotReady,MemoryPressure`); `Roles` from `node-role.kubernetes.io/*` labels |
| `describe-node` — **Rust-only, no Python equivalent** | `Api::<Node>::all().get(name)` | free text, not a table | — | `filter` arg is the (required) node name, not a substring — node names are unique so there's nothing to filter/index. `kubectl describe node`-style: labels/annotations, addresses, OS/kernel/runtime/kubelet versions, taints, capacity vs. allocatable, conditions |

---

## Data / config touch points

| Path | Use |
|---|---|
| `~/.kube/**` (excl. `*cache*`) | kubeconfig discovery |
| `~/.local/share/devopscenter/` | created on startup; `namespaces.json` path is computed but never written in current code |
| kube API timeouts | pods list 5 s; view lists 60 s; namespace delete poll 45 s |
