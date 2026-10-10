//! `taints` — every node's taints, across the whole cluster at once.
//! `describe-node` already shows one node's taints in its text dump; this is
//! the "why won't this pod schedule" view — compare taints across every
//! node in one table instead of describing them one at a time.

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::Node;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct TaintsView;

#[async_trait::async_trait]
impl Command for TaintsView {
    fn name(&self) -> &'static str {
        "taints"
    }
    fn help(&self) -> &'static str {
        "Every node's taints across the cluster"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<Node> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing nodes")?;

        let mut rows: Vec<[String; 5]> = Vec::new();
        for node in list.items {
            let name = node.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let taints = node.spec.and_then(|s| s.taints).filter(|t| !t.is_empty());
            match taints {
                Some(taints) => {
                    for t in taints {
                        rows.push([
                            ctx.context().to_string(),
                            name.clone(),
                            t.key,
                            t.value.unwrap_or_default(),
                            t.effect,
                        ]);
                    }
                }
                None => rows.push([
                    ctx.context().to_string(),
                    name,
                    "<none>".into(),
                    "".into(),
                    "".into(),
                ]),
            }
        }
        Ok(Output::table(
            ["Cluster", "Node", "Key", "Value", "Effect"],
            rows,
        ))
    }
}
