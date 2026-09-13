//! `nodes` — cluster nodes: readiness, roles, age, kubelet version, and
//! CPU/Mem utilization from `metrics.k8s.io/v1beta1` NodeMetrics (needs
//! metrics-server). There is no Python equivalent — the original tool had no
//! node-level view at all.

use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::Node;
use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::{age, quantity};

pub struct NodesView;

/// `node-role.kubernetes.io/<role>` label keys, comma-joined; `<none>` for a
/// plain worker node with no role labels.
pub(super) fn roles(node: &Node) -> String {
    let Some(labels) = node.metadata.labels.as_ref() else {
        return "<none>".into();
    };
    let roles: Vec<&str> = labels
        .keys()
        .filter_map(|k| k.strip_prefix("node-role.kubernetes.io/"))
        .collect();
    if roles.is_empty() {
        "<none>".into()
    } else {
        roles.join(",")
    }
}

/// `Ready`/`NotReady`/`Unknown` from the `Ready` condition, plus any other
/// condition currently `True` (e.g. `MemoryPressure`) appended.
pub(super) fn status(node: &Node) -> String {
    let Some(conditions) = node.status.as_ref().and_then(|s| s.conditions.as_ref()) else {
        return "Unknown".into();
    };
    let ready = conditions.iter().find(|c| c.type_ == "Ready");
    let mut parts = vec![match ready.map(|c| c.status.as_str()) {
        Some("True") => "Ready",
        Some("False") => "NotReady",
        _ => "Unknown",
    }
    .to_string()];
    for c in conditions {
        if c.type_ != "Ready" && c.status == "True" {
            parts.push(c.type_.clone());
        }
    }
    parts.join(",")
}

fn allocatable_millicores(node: &Node) -> Option<i64> {
    node.status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .and_then(|a| a.get("cpu"))
        .and_then(|q| quantity::cpu_millicores(&q.0).ok())
}

fn allocatable_mib(node: &Node) -> Option<f64> {
    node.status
        .as_ref()
        .and_then(|s| s.allocatable.as_ref())
        .and_then(|a| a.get("memory"))
        .and_then(|q| quantity::memory_mib(&q.0).ok())
}

fn percent(used: f64, allocatable: Option<f64>) -> String {
    match allocatable.filter(|a| *a > 0.0) {
        Some(a) => format!("{}%", (used / a * 100.0).round() as i64),
        None => "-".into(),
    }
}

async fn node_usage(ctx: &ClusterClient) -> BTreeMap<String, (i64, f64)> {
    let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "NodeMetrics");
    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(ctx.client(), &ar);
    // metrics-server may not be installed — treat that as "no usage data"
    // rather than failing the whole view.
    let Ok(list) = api.list(&ListParams::default()).await else {
        return BTreeMap::new();
    };
    list.items
        .into_iter()
        .filter_map(|obj| {
            let name = obj.metadata.name?;
            let cpu_raw = obj
                .data
                .get("usage")
                .and_then(|u| u.get("cpu"))
                .and_then(|v| v.as_str())
                .unwrap_or("0");
            let mem_raw = obj
                .data
                .get("usage")
                .and_then(|u| u.get("memory"))
                .and_then(|v| v.as_str())
                .unwrap_or("0");
            Some((
                name,
                (
                    quantity::cpu_millicores(cpu_raw).unwrap_or(0),
                    quantity::memory_mib(mem_raw).unwrap_or(0.0),
                ),
            ))
        })
        .collect()
}

#[async_trait::async_trait]
impl Command for NodesView {
    fn name(&self) -> &'static str {
        "nodes"
    }
    fn help(&self) -> &'static str {
        "Cluster nodes: readiness, roles, age, version, CPU/Mem % (needs metrics-server)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let nodes = Api::<Node>::all(ctx.client())
            .list(&ListParams::default())
            .await
            .context("listing nodes")?
            .items;
        let usage = node_usage(ctx).await;
        let now = age::now_secs();

        let mut rows: Vec<[String; 7]> = Vec::new();
        for node in &nodes {
            let name = node.metadata.name.clone().unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let node_age = node
                .metadata
                .creation_timestamp
                .as_ref()
                .map(|t| age::format_secs(age::secs_since(now, t)))
                .unwrap_or_default();
            let version = node
                .status
                .as_ref()
                .and_then(|s| s.node_info.as_ref())
                .map(|i| i.kubelet_version.clone())
                .unwrap_or_default();
            let (cpu_pct, mem_pct) = match usage.get(&name) {
                Some((cpu_m, mem_m)) => (
                    percent(
                        *cpu_m as f64,
                        allocatable_millicores(node).map(|a| a as f64),
                    ),
                    percent(*mem_m, allocatable_mib(node)),
                ),
                None => ("-".into(), "-".into()),
            };
            rows.push([
                name,
                status(node),
                roles(node),
                node_age,
                version,
                cpu_pct,
                mem_pct,
            ]);
        }
        Ok(Output::table(
            ["Name", "Status", "Roles", "Age", "Version", "Cpu%", "Mem%"],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::NodeCondition;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn node(labels: &[(&str, &str)], conditions: Vec<(&str, &str)>) -> Node {
        Node {
            metadata: ObjectMeta {
                labels: Some(
                    labels
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                ),
                ..Default::default()
            },
            spec: None,
            status: Some(k8s_openapi::api::core::v1::NodeStatus {
                conditions: Some(
                    conditions
                        .into_iter()
                        .map(|(type_, status)| NodeCondition {
                            type_: type_.into(),
                            status: status.into(),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn roles_reads_node_role_labels() {
        let n = node(
            &[("node-role.kubernetes.io/control-plane", "")],
            vec![("Ready", "True")],
        );
        assert_eq!(roles(&n), "control-plane");
        assert_eq!(roles(&node(&[], vec![])), "<none>");
    }

    #[test]
    fn status_reports_ready_and_appends_true_pressure_conditions() {
        let ready = node(&[], vec![("Ready", "True")]);
        assert_eq!(status(&ready), "Ready");

        let pressured = node(&[], vec![("MemoryPressure", "True"), ("Ready", "False")]);
        assert_eq!(status(&pressured), "NotReady,MemoryPressure");

        let unknown = node(&[], vec![]);
        assert_eq!(status(&unknown), "Unknown");
    }

    #[test]
    fn percent_formats_or_dashes_when_no_allocatable() {
        assert_eq!(percent(500.0, Some(1000.0)), "50%");
        assert_eq!(percent(500.0, None), "-");
        assert_eq!(percent(500.0, Some(0.0)), "-");
    }
}
