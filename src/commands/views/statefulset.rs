use anyhow::{Context as _, Result};
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct StatefulsetView;

#[async_trait::async_trait]
impl Command for StatefulsetView {
    fn name(&self) -> &'static str {
        "stateful"
    }
    fn help(&self) -> &'static str {
        "StatefulSets across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<StatefulSet> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing statefulsets")?;

        let mut rows: Vec<[String; 3]> = Vec::new();
        for s in list.items {
            let name = s.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = s.metadata.namespace.unwrap_or_default();
            let idx = rows.len();
            rows.push([ctx.context().to_string(), ns, format!("{name} ({idx})")]);
        }
        Ok(Output::table(["Cluster", "Namespace", "Statefulset"], rows))
    }
}
