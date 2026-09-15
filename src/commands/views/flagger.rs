//! Flagger (`flagger.app/v1beta1` `Canary`) debug view. Same dynamic-API
//! approach as `istio.rs` — no typed crate exists for this CRD.

use anyhow::{Context as _, Result};
use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams};
use serde_json::Value;

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

pub struct CanaryView;

fn target(data: &Value) -> String {
    let target = data.get("spec").and_then(|s| s.get("targetRef"));
    let kind = target
        .and_then(|t| t.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let name = target
        .and_then(|t| t.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind.is_empty() && name.is_empty() {
        String::new()
    } else {
        format!("{kind}/{name}")
    }
}

fn status_str(data: &Value, field: &str) -> String {
    data.get("status")
        .and_then(|s| s.get(field))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn status_num(data: &Value, field: &str) -> String {
    data.get("status")
        .and_then(|s| s.get(field))
        .and_then(Value::as_i64)
        .map(|n| n.to_string())
        .unwrap_or_default()
}

fn canary_row(obj: &DynamicObject) -> [String; 8] {
    let ns = obj.metadata.namespace.clone().unwrap_or_default();
    let name = obj.metadata.name.clone().unwrap_or_default();
    [
        ns,
        name,
        target(&obj.data),
        status_str(&obj.data, "phase"),
        status_num(&obj.data, "canaryWeight"),
        status_num(&obj.data, "iterations"),
        status_num(&obj.data, "failedChecks"),
        status_str(&obj.data, "lastTransitionTime"),
    ]
}

#[async_trait::async_trait]
impl Command for CanaryView {
    fn name(&self) -> &'static str {
        "canary"
    }
    fn help(&self) -> &'static str {
        "Flagger Canaries: rollout phase, weight and failed checks"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let gvk = GroupVersionKind::gvk("flagger.app", "v1beta1", "Canary");
        let ar = ApiResource::from_gvk(&gvk);
        let api: Api<DynamicObject> = Api::all_with(ctx.client(), &ar);
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing canaries (flagger.app/v1beta1)")?;

        let rows: Vec<[String; 8]> = list
            .items
            .iter()
            .filter(|o| passes(filter, o.metadata.name.as_deref().unwrap_or_default()))
            .map(canary_row)
            .collect();
        Ok(Output::table(
            [
                "Namespace",
                "Name",
                "Target",
                "Phase",
                "Weight",
                "Iterations",
                "Failed Checks",
                "Last Transition",
            ],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use serde_json::json;

    fn obj(namespace: &str, name: &str, data: Value) -> DynamicObject {
        DynamicObject {
            types: None,
            metadata: ObjectMeta {
                namespace: Some(namespace.into()),
                name: Some(name.into()),
                ..Default::default()
            },
            data,
        }
    }

    #[test]
    fn canary_row_reads_target_and_status() {
        let o = obj(
            "web",
            "web-canary",
            json!({
                "spec": {"targetRef": {"kind": "Deployment", "name": "web"}},
                "status": {
                    "phase": "Progressing", "canaryWeight": 30, "iterations": 3,
                    "failedChecks": 0, "lastTransitionTime": "2026-09-15T10:00:00Z"
                }
            }),
        );
        assert_eq!(
            canary_row(&o),
            [
                "web",
                "web-canary",
                "Deployment/web",
                "Progressing",
                "30",
                "3",
                "0",
                "2026-09-15T10:00:00Z"
            ]
        );
    }

    #[test]
    fn canary_row_handles_missing_status() {
        let o = obj(
            "web",
            "web-canary",
            json!({"spec": {"targetRef": {"kind": "Deployment", "name": "web"}}}),
        );
        assert_eq!(
            canary_row(&o),
            ["web", "web-canary", "Deployment/web", "", "", "", "", ""]
        );
    }

    #[test]
    fn canary_row_handles_missing_target() {
        let o = obj(
            "web",
            "web-canary",
            json!({"status": {"phase": "Initialized"}}),
        );
        assert_eq!(
            canary_row(&o),
            ["web", "web-canary", "", "Initialized", "", "", "", ""]
        );
    }
}
