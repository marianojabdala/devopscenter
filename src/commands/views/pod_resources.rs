use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct PodResourcesView;

#[async_trait::async_trait]
impl Command for PodResourcesView {
    fn name(&self) -> &'static str {
        "resources"
    }
    fn help(&self) -> &'static str {
        "Requests / limits per container"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<Pod> = Api::all(ctx.client());
        let pods = api
            .list(&ListParams::default())
            .await
            .context("listing pods")?;

        let mut rows: Vec<[String; 4]> = Vec::new();
        for pod in pods.items {
            let name = pod.metadata.name.clone().unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = pod.metadata.namespace.clone().unwrap_or_default();
            let Some(spec) = pod.spec else { continue };
            for c in spec.containers {
                let (req, lim) = c
                    .resources
                    .map(|r| (fmt_map(r.requests), fmt_map(r.limits)))
                    .unwrap_or_else(|| ("{}".into(), "{}".into()));
                rows.push([ns.clone(), c.name, req, lim]);
            }
        }
        Ok(Output::table(
            ["Namespace", "Container", "Requests", "Limits"],
            rows,
        ))
    }
}

fn fmt_map(
    m: Option<
        std::collections::BTreeMap<String, k8s_openapi::apimachinery::pkg::api::resource::Quantity>,
    >,
) -> String {
    match m {
        None => "{}".into(),
        Some(map) => {
            let inner = map
                .into_iter()
                .map(|(k, v)| format!("{k}: {}", v.0))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
    }
}
