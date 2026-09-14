//! L4 `pods` and `delete` (`docs/behaviour-catalogue.md` L4).

use anyhow::{anyhow, Context as _, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, DeleteParams, ListParams};

use super::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::age;
use crate::domain::container_state::{self, DisplayState};

pub(crate) fn pods_api(ctx: &ClusterClient, namespace: &str) -> Api<Pod> {
    Api::namespaced(ctx.client(), namespace)
}

pub(crate) async fn list_pods(ctx: &ClusterClient, namespace: &str) -> Result<Vec<Pod>> {
    Ok(pods_api(ctx, namespace)
        .list(&ListParams::default())
        .await
        .with_context(|| format!("listing pods in {namespace}"))?
        .items)
}

/// Resolve a `"<pod_index>.<container_index>"` selector (container part
/// optional) against a pod list. Mirrors Python `get_pod` + `get_container_name`.
pub(crate) fn resolve<'a>(pods: &'a [Pod], selector: &str) -> Result<(&'a Pod, Option<String>)> {
    let (pod_part, container_part) = match selector.split_once('.') {
        Some((p, c)) => (p, Some(c)),
        None => (selector, None),
    };
    let pod_idx: usize = pod_part
        .parse()
        .map_err(|_| anyhow!("selector must be <pod>.<container>, e.g. 0.0"))?;
    let pod = pods
        .get(pod_idx)
        .ok_or_else(|| anyhow!("The number is not in the list of pods"))?;

    let container = match container_part {
        None => None,
        Some(c) => {
            let cidx: usize = c
                .parse()
                .map_err(|_| anyhow!("selector must be <pod>.<container>, e.g. 0.0"))?;
            container_names(pod).into_iter().nth(cidx)
        }
    };
    Ok((pod, container))
}

fn container_names(pod: &Pod) -> Vec<String> {
    pod.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|cs| cs.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default()
}

/// How `pods` narrows the list. The `N°` index in every row is always the
/// pod's position in the *unfiltered* list (see [`pods_table`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PodsFilter<'a> {
    All,
    /// Case-insensitive substring match on pod name.
    Name(&'a str),
    /// Any container not `Running` (or cleanly `Completed`), or a pod with no
    /// container statuses yet (still `Pending`/scheduling) — the "what's
    /// actually broken in this namespace" view.
    Unhealthy,
}

impl<'a> PodsFilter<'a> {
    /// `args[1]` decides the filter: `--unhealthy`/`-u` selects
    /// [`PodsFilter::Unhealthy`], anything else is a name substring.
    pub(crate) fn from_args(args: &'a [String]) -> Self {
        match args.get(1).map(String::as_str) {
            None => PodsFilter::All,
            Some("--unhealthy" | "-u") => PodsFilter::Unhealthy,
            Some(name) => PodsFilter::Name(name),
        }
    }
}

/// A container is healthy if it's running, or terminated because it
/// completed normally (a Job's pod, say) — anything else (crash looping,
/// image pull errors, OOMKilled, ...) counts as unhealthy.
fn container_is_healthy(state: &DisplayState) -> bool {
    match state {
        DisplayState::Running => true,
        DisplayState::Terminated { reason: Some(r) } if r == "Completed" => true,
        _ => false,
    }
}

pub(crate) fn pod_is_unhealthy(pod: &Pod) -> bool {
    let phase = pod.status.as_ref().and_then(|s| s.phase.as_deref());
    if phase == Some("Failed") {
        return true;
    }
    match pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
    {
        None => phase != Some("Succeeded"),
        Some(statuses) if statuses.is_empty() => phase != Some("Succeeded"),
        Some(statuses) => statuses
            .iter()
            .any(|cs| !container_is_healthy(&container_state::derive(cs))),
    }
}

/// `<unknown>` if the pod has no `creationTimestamp` yet (shouldn't happen for
/// a pod the API server has returned, but the field is optional in the type).
fn pod_age(pod: &Pod, now: i64) -> String {
    pod.metadata
        .creation_timestamp
        .as_ref()
        .map(|t| age::format_secs(age::secs_since(now, t)))
        .unwrap_or_else(|| "<unknown>".into())
}

/// `key=value` pairs, comma-separated, sorted by key (`BTreeMap` iteration
/// order). `<none>` if the pod has no labels.
fn pod_labels(pod: &Pod) -> String {
    match &pod.metadata.labels {
        Some(labels) if !labels.is_empty() => labels
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(","),
        _ => "<none>".into(),
    }
}

/// Build the `pods` table: one row per container that has a status, keyed
/// `"<pod_index>.<container_index>"`. Pure so it can be unit-tested.
///
/// A name filter hides pods whose name doesn't match, but the `N°` index
/// still reflects each pod's position in the *unfiltered* list, so a printed
/// index like `3.0` still resolves correctly against `logs`/`exec`/`delete`,
/// which always re-list unfiltered. Under [`PodsFilter::Unhealthy`], a pod
/// with no container statuses yet still gets a row (index with no `.`, same
/// convention `resolve` already gives a container-less selector) so a stuck
/// `Pending` pod is visible instead of silently dropped.
pub(crate) fn pods_table(pods: &[Pod], filter: PodsFilter) -> Output {
    let now = age::now_secs();
    let mut rows: Vec<[String; 7]> = Vec::new();
    for (pi, pod) in pods.iter().enumerate() {
        let name = pod.metadata.name.clone().unwrap_or_default();
        match filter {
            PodsFilter::All => {}
            PodsFilter::Name(f) => {
                if !name.to_lowercase().contains(&f.to_lowercase()) {
                    continue;
                }
            }
            PodsFilter::Unhealthy => {
                if !pod_is_unhealthy(pod) {
                    continue;
                }
            }
        }
        let node = pod
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .unwrap_or_default();
        let age = pod_age(pod, now);
        let labels = pod_labels(pod);
        let statuses = pod
            .status
            .as_ref()
            .and_then(|s| s.container_statuses.as_ref());
        match statuses {
            Some(statuses) if !statuses.is_empty() => {
                for (ci, cs) in statuses.iter().enumerate() {
                    rows.push([
                        format!("{pi}.{ci}"),
                        name.clone(),
                        cs.name.clone(),
                        container_state::derive(cs).label(),
                        node.clone(),
                        age.clone(),
                        labels.clone(),
                    ]);
                }
            }
            _ if filter == PodsFilter::Unhealthy => {
                let phase = pod
                    .status
                    .as_ref()
                    .and_then(|s| s.phase.clone())
                    .unwrap_or_else(|| "Unknown".into());
                rows.push([
                    pi.to_string(),
                    name.clone(),
                    "<none>".into(),
                    phase,
                    node,
                    age,
                    labels,
                ]);
            }
            _ => {}
        }
    }
    Output::table(
        ["N°", "Pod", "Container", "State", "Node", "Age", "Labels"],
        rows,
    )
}

/// `pods [filter | --unhealthy]` — list pods/containers in the namespace,
/// optionally restricted to a name substring or to unhealthy pods.
pub struct PodsList {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodsList {
    fn name(&self) -> &'static str {
        "pods"
    }
    fn help(&self) -> &'static str {
        "Shows the pods (pods <name-substring>, or pods --unhealthy)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let pods = list_pods(ctx, &self.namespace).await?;
        Ok(pods_table(&pods, PodsFilter::from_args(args)))
    }
}

/// `delete <pod>.<container>` — deletes the pod (container part ignored, as in
/// the Python version).
pub struct PodDelete {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodDelete {
    fn name(&self) -> &'static str {
        "delete"
    }
    fn help(&self) -> &'static str {
        "Delete the selected pod"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text("usage: delete <pod>.<container>".into()));
        };
        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, _) = resolve(&pods, selector)?;
        let name = pod
            .metadata
            .name
            .clone()
            .ok_or_else(|| anyhow!("pod has no name"))?;
        pods_api(ctx, &self.namespace)
            .delete(&name, &DeleteParams::default())
            .await
            .with_context(|| format!("deleting pod {name}"))?;
        Ok(Output::Text(format!("pod/{name} deleted")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateRunning, ContainerStatus, PodSpec, PodStatus,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};

    fn cs(name: &str, ready: bool) -> ContainerStatus {
        ContainerStatus {
            name: name.into(),
            ready,
            image: "i".into(),
            image_id: String::new(),
            restart_count: 0,
            state: Some(ContainerState {
                running: Some(ContainerStateRunning::default()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn pod(name: &str, node: &str, statuses: Option<Vec<ContainerStatus>>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.into()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                node_name: Some(node.into()),
                ..Default::default()
            }),
            status: Some(PodStatus {
                container_statuses: statuses,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn table_has_one_row_per_container_with_pc_numbering() {
        let pods = vec![
            pod(
                "web-0",
                "node-a",
                Some(vec![cs("app", true), cs("proxy", false)]),
            ),
            pod("job-1", "node-b", Some(vec![cs("run", true)])),
        ];
        let Output::Table { headers, rows } = pods_table(&pods, PodsFilter::All) else {
            panic!("expected table");
        };
        assert_eq!(
            headers,
            ["N°", "Pod", "Container", "State", "Node", "Age", "Labels"]
        );
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            [
                "0.0",
                "web-0",
                "app",
                "Running",
                "node-a",
                "<unknown>",
                "<none>"
            ]
        );
        assert_eq!(
            rows[1],
            [
                "0.1",
                "web-0",
                "proxy",
                "Not Ready",
                "node-a",
                "<unknown>",
                "<none>"
            ]
        );
        assert_eq!(
            rows[2],
            [
                "1.0",
                "job-1",
                "run",
                "Running",
                "node-b",
                "<unknown>",
                "<none>"
            ]
        );
    }

    #[test]
    fn table_reports_age_and_labels_when_present() {
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "web".to_string());
        labels.insert("version".to_string(), "v2".to_string());
        let mut p = pod("web-0", "node-a", Some(vec![cs("app", true)]));
        p.metadata.creation_timestamp = Some(Time(k8s_openapi::jiff::Timestamp::now()));
        p.metadata.labels = Some(labels);

        let Output::Table { rows, .. } = pods_table(&[p], PodsFilter::All) else {
            panic!("expected table");
        };
        assert_eq!(rows[0][5], "0s");
        assert_eq!(rows[0][6], "app=web,version=v2");
    }

    #[test]
    fn pods_without_statuses_contribute_no_rows() {
        let pods = vec![pod("pending-0", "", None)];
        let Output::Table { rows, .. } = pods_table(&pods, PodsFilter::All) else {
            panic!("expected table");
        };
        assert!(rows.is_empty());
    }

    #[test]
    fn resolve_pod_and_container() {
        let pods = vec![pod(
            "web-0",
            "n",
            Some(vec![cs("app", true), cs("proxy", true)]),
        )];
        let (p, c) = resolve(&pods, "0.1").unwrap();
        assert_eq!(p.metadata.name.as_deref(), Some("web-0"));
        assert_eq!(c.as_deref(), Some("proxy"));

        let (_, c_none) = resolve(&pods, "0").unwrap();
        assert!(c_none.is_none());

        assert!(resolve(&pods, "9").is_err());
    }

    #[test]
    fn filter_hides_non_matching_pods_but_keeps_original_index() {
        let pods = vec![
            pod("web-0", "node-a", Some(vec![cs("app", true)])),
            pod("worker-1", "node-b", Some(vec![cs("run", true)])),
        ];
        let Output::Table { rows, .. } = pods_table(&pods, PodsFilter::Name("web")) else {
            panic!("expected table");
        };
        // Only the matching pod is shown, but its index (0) is the position
        // in the *unfiltered* list, so `exec`/`logs`/`delete 0.0` still
        // resolve to the right pod.
        assert_eq!(
            rows,
            vec![[
                "0.0",
                "web-0",
                "app",
                "Running",
                "node-a",
                "<unknown>",
                "<none>"
            ]]
        );
    }

    #[test]
    fn filter_is_case_insensitive() {
        let pods = vec![pod("Web-0", "n", Some(vec![cs("app", true)]))];
        let Output::Table { rows, .. } = pods_table(&pods, PodsFilter::Name("WEB")) else {
            panic!("expected table");
        };
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn from_args_recognises_the_unhealthy_flag() {
        assert_eq!(PodsFilter::from_args(&["pods".into()]), PodsFilter::All);
        assert_eq!(
            PodsFilter::from_args(&["pods".into(), "web".into()]),
            PodsFilter::Name("web")
        );
        assert_eq!(
            PodsFilter::from_args(&["pods".into(), "--unhealthy".into()]),
            PodsFilter::Unhealthy
        );
        assert_eq!(
            PodsFilter::from_args(&["pods".into(), "-u".into()]),
            PodsFilter::Unhealthy
        );
    }

    #[test]
    fn unhealthy_filter_keeps_only_broken_pods_with_original_index() {
        let pods = vec![
            pod("web-0", "node-a", Some(vec![cs("app", true)])), // healthy
            pod("web-1", "node-a", Some(vec![cs("app", false)])), // not ready
        ];
        let Output::Table { rows, .. } = pods_table(&pods, PodsFilter::Unhealthy) else {
            panic!("expected table");
        };
        assert_eq!(
            rows,
            vec![[
                "1.0",
                "web-1",
                "app",
                "Not Ready",
                "node-a",
                "<unknown>",
                "<none>"
            ]]
        );
    }

    #[test]
    fn unhealthy_filter_surfaces_pods_with_no_container_statuses() {
        let pods = vec![pod("pending-0", "", None)];
        let Output::Table { rows, .. } = pods_table(&pods, PodsFilter::Unhealthy) else {
            panic!("expected table");
        };
        // No container index — same convention as a container-less `resolve` selector.
        assert_eq!(
            rows,
            vec![[
                "0",
                "pending-0",
                "<none>",
                "Unknown",
                "",
                "<unknown>",
                "<none>"
            ]]
        );
    }

    #[test]
    fn unhealthy_filter_excludes_completed_job_pods() {
        let mut completed = cs("app", true);
        completed.state = Some(ContainerState {
            terminated: Some(k8s_openapi::api::core::v1::ContainerStateTerminated {
                reason: Some("Completed".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut pod_status = pod("job-0", "node-a", Some(vec![completed]));
        pod_status.status.as_mut().unwrap().phase = Some("Succeeded".into());

        let Output::Table { rows, .. } = pods_table(&[pod_status], PodsFilter::Unhealthy) else {
            panic!("expected table");
        };
        assert!(rows.is_empty());
    }
}
