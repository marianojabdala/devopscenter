//! `usage` — live pod/container CPU + memory from `metrics.k8s.io/v1beta1`.
//!
//! Fixes vs. Python: quantities are parsed numerically (O4) and the table is
//! sorted by memory as a **number**, not a string.

use anyhow::{Context as _, Result};
use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::quantity;

pub struct UsageView;

struct Usage {
    namespace: String,
    pod: String,
    container: String,
    cpu_milli: i64,
    mem_mib: f64,
}

#[async_trait::async_trait]
impl Command for UsageView {
    fn name(&self) -> &'static str {
        "usage"
    }
    fn help(&self) -> &'static str {
        "Live CPU / memory usage (needs metrics-server)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let gvk = GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "PodMetrics");
        let ar = ApiResource::from_gvk(&gvk);
        let api: Api<DynamicObject> = Api::all_with(ctx.client(), &ar);
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing pod metrics (metrics.k8s.io/v1beta1)")?;

        let mut items: Vec<Usage> = Vec::new();
        for obj in list.items {
            let pod = obj.metadata.name.clone().unwrap_or_default();
            if !passes(filter, &pod) {
                continue;
            }
            let namespace = obj.metadata.namespace.clone().unwrap_or_default();
            let Some(containers) = obj.data.get("containers").and_then(|c| c.as_array()) else {
                continue;
            };
            for c in containers {
                let container = c
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let cpu_raw = c
                    .get("usage")
                    .and_then(|u| u.get("cpu"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");
                let mem_raw = c
                    .get("usage")
                    .and_then(|u| u.get("memory"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");
                items.push(Usage {
                    namespace: namespace.clone(),
                    pod: pod.clone(),
                    container,
                    cpu_milli: quantity::cpu_millicores(cpu_raw).unwrap_or(0),
                    mem_mib: quantity::memory_mib(mem_raw).unwrap_or(0.0),
                });
            }
        }

        items.sort_by(|a, b| b.mem_mib.total_cmp(&a.mem_mib));

        let rows: Vec<[String; 5]> = items
            .into_iter()
            .map(|u| {
                [
                    u.namespace,
                    u.pod,
                    u.container,
                    quantity::format_millicores(u.cpu_milli),
                    quantity::format_mib(u.mem_mib),
                ]
            })
            .collect();
        Ok(Output::table(
            ["Namespace", "Pod Name", "Container Name", "Cpu", "Memory"],
            rows,
        ))
    }
}
