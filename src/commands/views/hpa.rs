use anyhow::{Context as _, Result};
use k8s_openapi::api::autoscaling::v1::HorizontalPodAutoscaler;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct HpaView;

#[async_trait::async_trait]
impl Command for HpaView {
    fn name(&self) -> &'static str {
        "hpa"
    }
    fn help(&self) -> &'static str {
        "Horizontal Pod Autoscalers across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<HorizontalPodAutoscaler> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing horizontalpodautoscalers")?;

        let mut rows: Vec<[String; 6]> = Vec::new();
        for h in list.items {
            let name = h.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = h.metadata.namespace.unwrap_or_default();
            let spec = h.spec.unwrap_or_default();
            let min = spec.min_replicas.map(|v| v.to_string()).unwrap_or_default();
            let max = spec.max_replicas.to_string();
            let current = h
                .status
                .map(|s| s.current_replicas.to_string())
                .unwrap_or_default();
            let idx = rows.len();
            rows.push([
                ctx.context().to_string(),
                ns,
                format!("{name} ({idx})"),
                min,
                max,
                current,
            ]);
        }
        Ok(Output::table(
            [
                "Cluster",
                "Namespace",
                "Hpa name",
                "min replicas",
                "max replicas",
                "current replicas",
            ],
            rows,
        ))
    }
}
