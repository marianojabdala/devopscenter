//! L4 `endpoints <service>` — a Service's backing pods and their
//! ready/not-ready state.
//!
//! Complements `whois` (IP → pod/service) from the other direction: service
//! name → which pods are actually behind it right now. Catches the "Service
//! exists but routes nowhere" bug (selector typo, every backing pod failing
//! readiness) that `whois` alone won't surface.

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::{EndpointAddress, Endpoints};
use kube::api::Api;

use super::{Command, Output};
use crate::config::ClusterClient;

/// One row per address: `Ready`, `IP`, `Pod` (from `target_ref` when it's a
/// Pod, `<none>` otherwise — e.g. a manually-managed `Endpoints` object), `Node`.
fn address_rows(addresses: &[EndpointAddress], ready: bool) -> Vec<[String; 4]> {
    addresses
        .iter()
        .map(|a| {
            let pod = a
                .target_ref
                .as_ref()
                .filter(|r| r.kind.as_deref() == Some("Pod"))
                .and_then(|r| r.name.clone())
                .unwrap_or_else(|| "<none>".into());
            [
                ready.to_string(),
                a.ip.clone(),
                pod,
                a.node_name.clone().unwrap_or_default(),
            ]
        })
        .collect()
}

pub struct ServiceEndpoints {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for ServiceEndpoints {
    fn name(&self) -> &'static str {
        "endpoints"
    }
    fn help(&self) -> &'static str {
        "Shows a service's backing pods and their ready/not-ready state"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: endpoints <service>".into()));
        };
        let api: Api<Endpoints> = Api::namespaced(ctx.client(), &self.namespace);
        let endpoints = match api.get(name).await {
            Ok(e) => e,
            Err(kube::Error::Api(status)) if status.code == 404 => {
                return Ok(no_endpoints(name, &self.namespace));
            }
            Err(err) => {
                return Err(err).with_context(|| format!("getting endpoints for {name}"));
            }
        };

        let mut rows: Vec<[String; 4]> = Vec::new();
        for subset in endpoints.subsets.unwrap_or_default() {
            rows.extend(address_rows(&subset.addresses.unwrap_or_default(), true));
            rows.extend(address_rows(
                &subset.not_ready_addresses.unwrap_or_default(),
                false,
            ));
        }
        if rows.is_empty() {
            return Ok(no_endpoints(name, &self.namespace));
        }
        Ok(Output::table(["Ready", "IP", "Pod", "Node"], rows))
    }
}

fn no_endpoints(name: &str, namespace: &str) -> Output {
    Output::Text(format!(
        "No endpoints found for service {name:?} in {namespace}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::ObjectReference;

    fn address(ip: &str, pod: Option<&str>, node: Option<&str>) -> EndpointAddress {
        EndpointAddress {
            ip: ip.into(),
            node_name: node.map(String::from),
            target_ref: pod.map(|p| ObjectReference {
                kind: Some("Pod".into()),
                name: Some(p.into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn address_rows_marks_ready_and_extracts_pod_and_node() {
        let addrs = vec![address("10.0.0.1", Some("web-0"), Some("node-a"))];
        assert_eq!(
            address_rows(&addrs, true),
            vec![[
                "true".to_string(),
                "10.0.0.1".to_string(),
                "web-0".to_string(),
                "node-a".to_string()
            ]]
        );
        assert_eq!(
            address_rows(&addrs, false),
            vec![[
                "false".to_string(),
                "10.0.0.1".to_string(),
                "web-0".to_string(),
                "node-a".to_string()
            ]]
        );
    }

    #[test]
    fn address_rows_falls_back_when_target_is_not_a_pod() {
        let addrs = vec![address("10.0.0.2", None, None)];
        let rows = address_rows(&addrs, true);
        assert_eq!(rows[0][2], "<none>");
        assert_eq!(rows[0][3], "");
    }
}
