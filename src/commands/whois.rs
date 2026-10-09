//! `whois <ip>` — what is this IP: a pod, a service, or both.
//!
//! Cluster-wide, like `search` (`docs/commands/search.rs`): the per-namespace
//! pod/service listing runs concurrently, and the observable contract is the
//! set of matching rows.

use anyhow::{Context as _, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use k8s_openapi::api::core::v1::{Namespace, Pod, Service};
use kube::api::{Api, ListParams};

use super::{Command, Output};
use crate::config::ClusterClient;

const CONCURRENCY: usize = 16;

/// `true` if `pod`'s assigned IP is `ip`.
fn pod_has_ip(pod: &Pod, ip: &str) -> bool {
    pod.status
        .as_ref()
        .and_then(|s| s.pod_ip.as_deref())
        .is_some_and(|pod_ip| pod_ip == ip)
}

/// `true` if any IP this `Service` owns — `clusterIP`, `clusterIPs`,
/// `externalIPs`, or a `status.loadBalancer.ingress[].ip` — is `ip`.
fn service_has_ip(svc: &Service, ip: &str) -> bool {
    let spec_match = svc.spec.as_ref().is_some_and(|spec| {
        spec.cluster_ip.as_deref() == Some(ip)
            || spec
                .cluster_ips
                .as_deref()
                .is_some_and(|ips| ips.iter().any(|i| i == ip))
            || spec
                .external_ips
                .as_deref()
                .is_some_and(|ips| ips.iter().any(|i| i == ip))
    });
    let lb_match = svc
        .status
        .as_ref()
        .and_then(|s| s.load_balancer.as_ref())
        .and_then(|lb| lb.ingress.as_ref())
        .is_some_and(|ingress| ingress.iter().any(|i| i.ip.as_deref() == Some(ip)));
    spec_match || lb_match
}

/// `true` if `selector` (a `Service`'s `spec.selector`) is a non-empty subset
/// of `labels` (a `Pod`'s labels) — i.e. the service routes to that pod.
fn selector_matches_labels(
    selector: &std::collections::BTreeMap<String, String>,
    labels: &std::collections::BTreeMap<String, String>,
) -> bool {
    !selector.is_empty() && selector.iter().all(|(k, v)| labels.get(k) == Some(v))
}

/// Every namespace in the cluster.
async fn all_namespaces(ctx: &ClusterClient) -> Result<Vec<String>> {
    Ok(Api::<Namespace>::all(ctx.client())
        .list(&ListParams::default())
        .await
        .context("listing namespaces")?
        .items
        .into_iter()
        .filter_map(|n| n.metadata.name)
        .collect())
}

pub struct WhoIs;

#[async_trait::async_trait]
impl Command for WhoIs {
    fn name(&self) -> &'static str {
        "whois"
    }
    fn help(&self) -> &'static str {
        "Find the pod and/or service for a given IP, across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(ip) = args.get(1) else {
            return Ok(Output::Text("usage: whois <ip>".into()));
        };

        let namespaces = all_namespaces(ctx).await?;

        let mut pod_tasks = FuturesUnordered::new();
        let mut iter = namespaces.clone().into_iter();
        let spawn_pods = |client: kube::Client, ns: String| async move {
            Api::<Pod>::namespaced(client, &ns)
                .list(&ListParams::default())
                .await
                .map(|l| l.items)
                .unwrap_or_default()
        };
        for _ in 0..CONCURRENCY {
            if let Some(ns) = iter.next() {
                pod_tasks.push(spawn_pods(ctx.client(), ns));
            }
        }
        let mut pod_hit = None;
        while let Some(pods) = pod_tasks.next().await {
            if pod_hit.is_none() {
                pod_hit = pods.into_iter().find(|p| pod_has_ip(p, ip));
            }
            if let Some(ns) = iter.next() {
                pod_tasks.push(spawn_pods(ctx.client(), ns));
            }
        }

        let mut svc_tasks = FuturesUnordered::new();
        let mut iter = namespaces.into_iter();
        let spawn_svcs = |client: kube::Client, ns: String| async move {
            Api::<Service>::namespaced(client, &ns)
                .list(&ListParams::default())
                .await
                .map(|l| l.items)
                .unwrap_or_default()
        };
        for _ in 0..CONCURRENCY {
            if let Some(ns) = iter.next() {
                svc_tasks.push(spawn_svcs(ctx.client(), ns));
            }
        }
        let mut service_hits = Vec::new();
        while let Some(svcs) = svc_tasks.next().await {
            service_hits.extend(svcs.into_iter().filter(|s| service_has_ip(s, ip)));
            if let Some(ns) = iter.next() {
                svc_tasks.push(spawn_svcs(ctx.client(), ns));
            }
        }

        let mut rows: Vec<[String; 4]> = Vec::new();

        if let Some(pod) = &pod_hit {
            let ns = pod.metadata.namespace.clone().unwrap_or_default();
            let name = pod.metadata.name.clone().unwrap_or_default();
            let node = pod
                .spec
                .as_ref()
                .and_then(|s| s.node_name.clone())
                .unwrap_or_default();
            rows.push(["Pod".into(), name, ns.clone(), format!("node={node}")]);

            if let Some(labels) = &pod.metadata.labels {
                let routing: Vec<String> = Api::<Service>::namespaced(ctx.client(), &ns)
                    .list(&ListParams::default())
                    .await
                    .map(|l| {
                        l.items
                            .into_iter()
                            .filter(|svc| {
                                svc.spec
                                    .as_ref()
                                    .and_then(|s| s.selector.as_ref())
                                    .is_some_and(|sel| selector_matches_labels(sel, labels))
                            })
                            .filter_map(|svc| svc.metadata.name)
                            .collect()
                    })
                    .unwrap_or_default();
                for svc_name in routing {
                    rows.push([
                        "Service".into(),
                        svc_name,
                        ns.clone(),
                        format!(
                            "routes to pod {}",
                            pod.metadata.name.clone().unwrap_or_default()
                        ),
                    ]);
                }
            }
        }

        for svc in &service_hits {
            let ns = svc.metadata.namespace.clone().unwrap_or_default();
            let name = svc.metadata.name.clone().unwrap_or_default();
            let svc_type = svc
                .spec
                .as_ref()
                .and_then(|s| s.type_.clone())
                .unwrap_or_else(|| "ClusterIP".into());
            rows.push(["Service".into(), name, ns, format!("type={svc_type}")]);
        }

        if rows.is_empty() {
            return Ok(Output::Text(format!(
                "No pod or service found with IP {ip}"
            )));
        }
        Ok(Output::table(["Kind", "Name", "Namespace", "Detail"], rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{PodSpec, PodStatus, ServiceSpec, ServiceStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn pod_with_ip(ip: &str) -> Pod {
        Pod {
            metadata: ObjectMeta::default(),
            spec: Some(PodSpec::default()),
            status: Some(PodStatus {
                pod_ip: Some(ip.into()),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn pod_has_ip_matches_pod_ip() {
        assert!(pod_has_ip(&pod_with_ip("10.1.2.3"), "10.1.2.3"));
        assert!(!pod_has_ip(&pod_with_ip("10.1.2.3"), "10.1.2.4"));
        let no_status = Pod {
            metadata: ObjectMeta::default(),
            spec: None,
            status: None,
        };
        assert!(!pod_has_ip(&no_status, "10.1.2.3"));
    }

    fn service(
        cluster_ip: Option<&str>,
        external_ips: Option<Vec<&str>>,
        lb_ip: Option<&str>,
    ) -> Service {
        Service {
            metadata: ObjectMeta::default(),
            spec: Some(ServiceSpec {
                cluster_ip: cluster_ip.map(String::from),
                external_ips: external_ips.map(|ips| ips.into_iter().map(String::from).collect()),
                ..Default::default()
            }),
            status: lb_ip.map(|ip| ServiceStatus {
                load_balancer: Some(k8s_openapi::api::core::v1::LoadBalancerStatus {
                    ingress: Some(vec![k8s_openapi::api::core::v1::LoadBalancerIngress {
                        ip: Some(ip.into()),
                        ..Default::default()
                    }]),
                }),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn service_has_ip_matches_cluster_ip() {
        let svc = service(Some("10.96.0.1"), None, None);
        assert!(service_has_ip(&svc, "10.96.0.1"));
        assert!(!service_has_ip(&svc, "10.96.0.2"));
    }

    #[test]
    fn service_has_ip_matches_external_ip() {
        let svc = service(None, Some(vec!["203.0.113.5"]), None);
        assert!(service_has_ip(&svc, "203.0.113.5"));
    }

    #[test]
    fn service_has_ip_matches_load_balancer_ingress() {
        let svc = service(None, None, Some("198.51.100.9"));
        assert!(service_has_ip(&svc, "198.51.100.9"));
    }

    #[test]
    fn selector_matches_labels_requires_every_selector_key() {
        let selector = BTreeMap::from([("app".to_string(), "web".to_string())]);
        let labels = BTreeMap::from([
            ("app".to_string(), "web".to_string()),
            ("version".to_string(), "v2".to_string()),
        ]);
        assert!(selector_matches_labels(&selector, &labels));

        let mismatched = BTreeMap::from([("app".to_string(), "other".to_string())]);
        assert!(!selector_matches_labels(&selector, &mismatched));

        let empty = BTreeMap::new();
        assert!(!selector_matches_labels(&empty, &labels));
    }
}
