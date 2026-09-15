//! Istio (`networking.istio.io/v1beta1`, `security.istio.io/v1beta1`) debug
//! views. No typed crate exists for these CRDs, so — like the `metrics.k8s.io`
//! `NodeMetrics` lookup in `nodes.rs` — they're read via `kube`'s dynamic API
//! and their fields pulled out of the raw JSON.

use anyhow::{Context as _, Result};
use kube::api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams};
use serde_json::Value;

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

async fn list(
    ctx: &ClusterClient,
    group: &str,
    version: &str,
    kind: &str,
) -> Result<Vec<DynamicObject>> {
    let gvk = GroupVersionKind::gvk(group, version, kind);
    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(ctx.client(), &ar);
    Ok(api
        .list(&ListParams::default())
        .await
        .with_context(|| format!("listing {kind} ({group}/{version})"))?
        .items)
}

fn str_array_joined(data: &Value, path: &[&str], sep: &str) -> String {
    let mut v = data;
    for key in path {
        let Some(next) = v.get(key) else {
            return String::new();
        };
        v = next;
    }
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(sep)
        })
        .unwrap_or_default()
}

pub struct IstioVirtualServicesView;

fn virtual_service_row(obj: &DynamicObject) -> [String; 4] {
    let ns = obj.metadata.namespace.clone().unwrap_or_default();
    let name = obj.metadata.name.clone().unwrap_or_default();
    let gateways = str_array_joined(&obj.data, &["spec", "gateways"], ",");
    let hosts = str_array_joined(&obj.data, &["spec", "hosts"], ",");
    [ns, name, gateways, hosts]
}

#[async_trait::async_trait]
impl Command for IstioVirtualServicesView {
    fn name(&self) -> &'static str {
        "istio-vs"
    }
    fn help(&self) -> &'static str {
        "Istio VirtualServices: gateways and hosts"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let objs = list(ctx, "networking.istio.io", "v1beta1", "VirtualService").await?;
        let rows: Vec<[String; 4]> = objs
            .iter()
            .filter(|o| passes(filter, o.metadata.name.as_deref().unwrap_or_default()))
            .map(virtual_service_row)
            .collect();
        Ok(Output::table(
            ["Namespace", "Name", "Gateways", "Hosts"],
            rows,
        ))
    }
}

pub struct IstioDestinationRulesView;

fn destination_rule_row(obj: &DynamicObject) -> [String; 4] {
    let ns = obj.metadata.namespace.clone().unwrap_or_default();
    let name = obj.metadata.name.clone().unwrap_or_default();
    let host = obj
        .data
        .get("spec")
        .and_then(|s| s.get("host"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let subsets = obj
        .data
        .get("spec")
        .and_then(|s| s.get("subsets"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    [ns, name, host, subsets]
}

#[async_trait::async_trait]
impl Command for IstioDestinationRulesView {
    fn name(&self) -> &'static str {
        "istio-dr"
    }
    fn help(&self) -> &'static str {
        "Istio DestinationRules: host and subsets"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let objs = list(ctx, "networking.istio.io", "v1beta1", "DestinationRule").await?;
        let rows: Vec<[String; 4]> = objs
            .iter()
            .filter(|o| passes(filter, o.metadata.name.as_deref().unwrap_or_default()))
            .map(destination_rule_row)
            .collect();
        Ok(Output::table(
            ["Namespace", "Name", "Host", "Subsets"],
            rows,
        ))
    }
}

pub struct IstioGatewaysView;

fn gateway_selector(data: &Value) -> String {
    data.get("spec")
        .and_then(|s| s.get("selector"))
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| format!("{k}={v}")))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default()
}

fn gateway_servers(data: &Value) -> String {
    let Some(servers) = data
        .get("spec")
        .and_then(|s| s.get("servers"))
        .and_then(Value::as_array)
    else {
        return String::new();
    };
    servers
        .iter()
        .map(|server| {
            let port = server.get("port");
            let number = port
                .and_then(|p| p.get("number"))
                .and_then(Value::as_u64)
                .map(|n| n.to_string())
                .unwrap_or_default();
            let protocol = port
                .and_then(|p| p.get("protocol"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let hosts = server
                .get("hosts")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            format!("{number}/{protocol}:{hosts}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn gateway_row(obj: &DynamicObject) -> [String; 4] {
    let ns = obj.metadata.namespace.clone().unwrap_or_default();
    let name = obj.metadata.name.clone().unwrap_or_default();
    [
        ns,
        name,
        gateway_selector(&obj.data),
        gateway_servers(&obj.data),
    ]
}

#[async_trait::async_trait]
impl Command for IstioGatewaysView {
    fn name(&self) -> &'static str {
        "istio-gw"
    }
    fn help(&self) -> &'static str {
        "Istio Gateways: workload selector and exposed servers"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let objs = list(ctx, "networking.istio.io", "v1beta1", "Gateway").await?;
        let rows: Vec<[String; 4]> = objs
            .iter()
            .filter(|o| passes(filter, o.metadata.name.as_deref().unwrap_or_default()))
            .map(gateway_row)
            .collect();
        Ok(Output::table(
            ["Namespace", "Name", "Selector", "Servers"],
            rows,
        ))
    }
}

pub struct IstioPeerAuthView;

/// `spec.selector.matchLabels`, `k=v` pairs comma-joined; `<mesh-wide>` if
/// absent (no selector means the policy applies to every workload in scope).
fn peer_auth_selector(data: &Value) -> String {
    data.get("spec")
        .and_then(|s| s.get("selector"))
        .and_then(|s| s.get("matchLabels"))
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| format!("{k}={v}")))
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "<mesh-wide>".into())
}

fn peer_auth_row(obj: &DynamicObject) -> [String; 4] {
    let ns = obj.metadata.namespace.clone().unwrap_or_default();
    let name = obj.metadata.name.clone().unwrap_or_default();
    let mode = obj
        .data
        .get("spec")
        .and_then(|s| s.get("mtls"))
        .and_then(|m| m.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("<unset>")
        .to_string();
    let selector = peer_auth_selector(&obj.data);
    [ns, name, mode, selector]
}

#[async_trait::async_trait]
impl Command for IstioPeerAuthView {
    fn name(&self) -> &'static str {
        "istio-pa"
    }
    fn help(&self) -> &'static str {
        "Istio PeerAuthentications: mTLS mode per selector"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let objs = list(ctx, "security.istio.io", "v1beta1", "PeerAuthentication").await?;
        let rows: Vec<[String; 4]> = objs
            .iter()
            .filter(|o| passes(filter, o.metadata.name.as_deref().unwrap_or_default()))
            .map(peer_auth_row)
            .collect();
        Ok(Output::table(
            ["Namespace", "Name", "mTLS Mode", "Selector"],
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
    fn virtual_service_row_reads_gateways_and_hosts() {
        let o = obj(
            "web",
            "vs-1",
            json!({"spec": {"gateways": ["mesh", "ingress-gw"], "hosts": ["a.example.com"]}}),
        );
        assert_eq!(
            virtual_service_row(&o),
            ["web", "vs-1", "mesh,ingress-gw", "a.example.com"]
        );
    }

    #[test]
    fn virtual_service_row_handles_missing_fields() {
        let o = obj("web", "vs-1", json!({"spec": {}}));
        assert_eq!(virtual_service_row(&o), ["web", "vs-1", "", ""]);
    }

    #[test]
    fn destination_rule_row_reads_host_and_subsets() {
        let o = obj(
            "web",
            "dr-1",
            json!({"spec": {"host": "web.web.svc.cluster.local", "subsets": [{"name": "canary"}, {"name": "primary"}]}}),
        );
        assert_eq!(
            destination_rule_row(&o),
            ["web", "dr-1", "web.web.svc.cluster.local", "canary,primary"]
        );
    }

    #[test]
    fn destination_rule_row_handles_no_subsets() {
        let o = obj("web", "dr-1", json!({"spec": {"host": "web"}}));
        assert_eq!(destination_rule_row(&o), ["web", "dr-1", "web", ""]);
    }

    #[test]
    fn gateway_row_renders_selector_and_servers() {
        let o = obj(
            "istio-system",
            "gw-1",
            json!({
                "spec": {
                    "selector": {"istio": "ingressgateway"},
                    "servers": [
                        {"port": {"number": 80, "protocol": "HTTP"}, "hosts": ["*"]},
                        {"port": {"number": 443, "protocol": "HTTPS"}, "hosts": ["a.example.com", "b.example.com"]}
                    ]
                }
            }),
        );
        let row = gateway_row(&o);
        assert_eq!(row[0], "istio-system");
        assert_eq!(row[1], "gw-1");
        assert_eq!(row[2], "istio=ingressgateway");
        assert_eq!(row[3], "80/HTTP:*\n443/HTTPS:a.example.com,b.example.com");
    }

    #[test]
    fn peer_auth_row_reports_mode_and_falls_back_to_unset() {
        let strict = obj(
            "web",
            "default",
            json!({"spec": {"mtls": {"mode": "STRICT"}, "selector": {"matchLabels": {"app": "web"}}}}),
        );
        let row = peer_auth_row(&strict);
        assert_eq!(row[2], "STRICT");
        assert_eq!(row[3], "app=web");

        let unset = obj("web", "default", json!({"spec": {}}));
        let row = peer_auth_row(&unset);
        assert_eq!(row[2], "<unset>");
        assert_eq!(row[3], "<mesh-wide>");
    }
}
