//! `daemonsets` — DaemonSet rollout status across all namespaces. Same shape
//! as `deploy`/`stateful`, but for the workload type neither covered.

use anyhow::{Context as _, Result};
use k8s_openapi::api::apps::v1::DaemonSet;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct DaemonSetView;

#[async_trait::async_trait]
impl Command for DaemonSetView {
    fn name(&self) -> &'static str {
        "daemonsets"
    }
    fn help(&self) -> &'static str {
        "DaemonSet rollout status across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<DaemonSet> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing daemonsets")?;

        let mut rows: Vec<[String; 6]> = Vec::new();
        for ds in list.items {
            let name = ds.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = ds.metadata.namespace.unwrap_or_default();
            let status = ds.status.unwrap_or_default();
            rows.push([
                ctx.context().to_string(),
                ns,
                name,
                status.desired_number_scheduled.to_string(),
                status.current_number_scheduled.to_string(),
                status.number_ready.to_string(),
            ]);
        }
        Ok(Output::table(
            [
                "Cluster",
                "Namespace",
                "Name",
                "Desired",
                "Current",
                "Ready",
            ],
            rows,
        ))
    }
}
