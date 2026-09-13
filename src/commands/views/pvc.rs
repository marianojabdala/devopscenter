use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct PvcView;

struct Row {
    namespace: String,
    storage: String,
    pod: String,
}

#[async_trait::async_trait]
impl Command for PvcView {
    fn name(&self) -> &'static str {
        "pvc"
    }
    fn help(&self) -> &'static str {
        "Persistent Volume Claims and the pods that mount them"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);

        let pvc_api: Api<PersistentVolumeClaim> = Api::all(ctx.client());
        let pvcs = pvc_api
            .list(&ListParams::default())
            .await
            .context("listing persistentvolumeclaims")?;

        let mut by_claim: BTreeMap<String, Row> = BTreeMap::new();
        for p in pvcs.items {
            let name = p.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let storage = p
                .spec
                .and_then(|s| s.resources)
                .and_then(|r| r.requests)
                .and_then(|req| req.get("storage").map(|q| q.0.clone()))
                .unwrap_or_else(|| "no storage".into());
            by_claim.insert(
                name,
                Row {
                    namespace: p
                        .metadata
                        .namespace
                        .unwrap_or_else(|| "no namespace".into()),
                    storage,
                    pod: "no pod".into(),
                },
            );
        }

        let pod_api: Api<Pod> = Api::all(ctx.client());
        let pods = pod_api
            .list(&ListParams::default())
            .await
            .context("listing pods")?;
        for pod in pods.items {
            let pod_name = pod.metadata.name.clone().unwrap_or_default();
            let Some(spec) = pod.spec else { continue };
            for vol in spec.volumes.unwrap_or_default() {
                if let Some(claim) = vol.persistent_volume_claim {
                    if let Some(row) = by_claim.get_mut(&claim.claim_name) {
                        row.pod = pod_name.clone();
                    }
                }
            }
        }

        let rows: Vec<[String; 5]> = by_claim
            .into_iter()
            .map(|(claim, r)| {
                [
                    ctx.context().to_string(),
                    r.namespace,
                    r.pod,
                    claim,
                    r.storage,
                ]
            })
            .collect();
        Ok(Output::table(
            ["Cluster", "Namespace", "Pod Name", "Pvc", "Capacity"],
            rows,
        ))
    }
}
