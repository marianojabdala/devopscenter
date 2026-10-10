//! L4 `istio-sidecar <pod_index>` — checks whether a pod actually has its
//! Istio sidecar injected, and why/why not: the namespace's
//! `istio-injection` label, the pod's own `sidecar.istio.io/inject`
//! override, and whether an `istio-proxy` container is actually present.
//! The #1 "why isn't my traffic going through the mesh" bug — a namespace
//! missing the injection label, or a pod explicitly opted out, with no
//! other symptom than silently bypassing the mesh.

use anyhow::Result;
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::api::Api;

use super::pods::{list_pods, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;

const NAMESPACE_LABEL: &str = "istio-injection";
const POD_OVERRIDE_ANNOTATION: &str = "sidecar.istio.io/inject";

/// `true` if the pod has a container named `istio-proxy` — the actual,
/// ground-truth signal (labels/annotations only express *intent*).
fn has_istio_proxy(pod: &Pod) -> bool {
    pod.spec
        .as_ref()
        .is_some_and(|s| s.containers.iter().any(|c| c.name == "istio-proxy"))
}

fn namespace_injection_label(ns: &Namespace) -> String {
    ns.metadata
        .labels
        .as_ref()
        .and_then(|l| l.get(NAMESPACE_LABEL))
        .cloned()
        .unwrap_or_else(|| "<unset>".into())
}

fn pod_inject_override(pod: &Pod) -> String {
    pod.metadata
        .annotations
        .as_ref()
        .and_then(|a| a.get(POD_OVERRIDE_ANNOTATION))
        .cloned()
        .unwrap_or_else(|| "<none>".into())
}

pub struct IstioSidecarCheck {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for IstioSidecarCheck {
    fn name(&self) -> &'static str {
        "istio-sidecar"
    }
    fn help(&self) -> &'static str {
        "Checks whether a pod has its Istio sidecar injected, and why/why not"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text("usage: istio-sidecar <pod_index>".into()));
        };
        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, _) = resolve(&pods, selector)?;

        let ns_label = Api::<Namespace>::all(ctx.client())
            .get_opt(&self.namespace)
            .await?
            .map(|ns| namespace_injection_label(&ns))
            .unwrap_or_else(|| "<namespace not found>".into());
        let pod_override = pod_inject_override(pod);
        let injected = has_istio_proxy(pod);

        Ok(Output::table(
            ["Check", "Value"],
            vec![
                ["Namespace istio-injection label".to_string(), ns_label],
                [
                    "Pod sidecar.istio.io/inject override".to_string(),
                    pod_override,
                ],
                [
                    "istio-proxy container present".to_string(),
                    injected.to_string(),
                ],
                [
                    "Verdict".to_string(),
                    if injected {
                        "Injected".into()
                    } else {
                        "Not injected".into()
                    },
                ],
            ],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Container, PodSpec};
    use std::collections::BTreeMap;

    fn pod_with(containers: Vec<&str>, annotations: Option<BTreeMap<String, String>>) -> Pod {
        Pod {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                annotations,
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: containers
                    .into_iter()
                    .map(|name| Container {
                        name: name.into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            status: None,
        }
    }

    #[test]
    fn has_istio_proxy_detects_the_sidecar_container() {
        assert!(has_istio_proxy(&pod_with(vec!["app", "istio-proxy"], None)));
        assert!(!has_istio_proxy(&pod_with(vec!["app"], None)));
    }

    #[test]
    fn pod_inject_override_reads_the_annotation_or_defaults_to_none() {
        let overridden = pod_with(
            vec!["app"],
            Some(BTreeMap::from([(
                "sidecar.istio.io/inject".to_string(),
                "false".to_string(),
            )])),
        );
        assert_eq!(pod_inject_override(&overridden), "false");
        assert_eq!(pod_inject_override(&pod_with(vec!["app"], None)), "<none>");
    }

    #[test]
    fn namespace_injection_label_reads_the_label_or_defaults_to_unset() {
        let labeled = Namespace {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                labels: Some(BTreeMap::from([(
                    "istio-injection".to_string(),
                    "enabled".to_string(),
                )])),
                ..Default::default()
            },
            spec: None,
            status: None,
        };
        assert_eq!(namespace_injection_label(&labeled), "enabled");

        let unlabeled = Namespace::default();
        assert_eq!(namespace_injection_label(&unlabeled), "<unset>");
    }
}
