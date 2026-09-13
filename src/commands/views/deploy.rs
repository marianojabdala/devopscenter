use anyhow::{Context as _, Result};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct DeployView;

#[async_trait::async_trait]
impl Command for DeployView {
    fn name(&self) -> &'static str {
        "deploy"
    }
    fn help(&self) -> &'static str {
        "Deployments across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<Deployment> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing deployments")?;

        let mut rows: Vec<[String; 4]> = Vec::new();
        for d in list.items {
            let name = d.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = d.metadata.namespace.unwrap_or_default();
            let replicas = d
                .spec
                .and_then(|s| s.replicas)
                .map(|r| r.to_string())
                .unwrap_or_else(|| "?".into());
            let idx = rows.len();
            rows.push([
                ctx.context().to_string(),
                ns,
                replicas,
                format!("{name} ({idx})"),
            ]);
        }
        Ok(Output::table(
            ["Cluster", "Namespace", "Replicas", "Name"],
            rows,
        ))
    }
}
