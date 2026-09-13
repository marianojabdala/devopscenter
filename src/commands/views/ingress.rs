//! `ingress` — uses `networking.k8s.io/v1` (migration.md **O5**: the Python
//! version queried the removed `extensions/v1`).

use anyhow::{Context as _, Result};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct IngressView;

#[async_trait::async_trait]
impl Command for IngressView {
    fn name(&self) -> &'static str {
        "ingress"
    }
    fn help(&self) -> &'static str {
        "Ingresses and their ingress.kubernetes.io annotations"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<Ingress> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing ingresses")?;

        let mut rows: Vec<[String; 3]> = Vec::new();
        for ing in list.items {
            let name = ing.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = ing.metadata.namespace.unwrap_or_default();
            let annotations = ing
                .metadata
                .annotations
                .unwrap_or_default()
                .into_iter()
                .filter(|(k, _)| k.contains("ingress.kubernetes.io"))
                .map(|(k, v)| format!("{k}:{v}"))
                .collect::<Vec<_>>();
            if annotations.is_empty() {
                continue;
            }
            rows.push([ns, name, annotations.join("\n")]);
        }
        Ok(Output::table(
            ["Namespace", "Ingress Name", "Annotations"],
            rows,
        ))
    }
}
