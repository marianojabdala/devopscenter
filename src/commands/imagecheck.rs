//! L4 `imagecheck <deployment>` — the Deployment's desired image per
//! container vs. what's actually running in each of its pods. Catches a
//! stuck rollout from `ImagePullBackOff` (or just a slow one) at a glance:
//! if a pod's running image doesn't match the desired one, that's why.
//!
//! Matches pods via the Deployment's `spec.selector.matchLabels` (simple
//! equality, like a Service selector) — `matchExpressions` aren't
//! evaluated, since the overwhelming majority of Deployments only use
//! `matchLabels`.

use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::Api;

use super::pods::list_pods;
use super::{Command, Output};
use crate::config::ClusterClient;

/// `true` if `selector` (a Deployment's `matchLabels`) is a non-empty subset
/// of `labels` (a pod's labels) — same rule `whois` uses for a Service.
fn selector_matches(
    selector: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
) -> bool {
    !selector.is_empty() && selector.iter().all(|(k, v)| labels.get(k) == Some(v))
}

/// `(container_name, desired_image)` for every container in the
/// Deployment's pod template that has an image set.
fn desired_images(dep: &Deployment) -> Vec<(String, String)> {
    dep.spec
        .as_ref()
        .and_then(|s| s.template.spec.as_ref())
        .map(|s| &s.containers)
        .into_iter()
        .flatten()
        .filter_map(|c| c.image.clone().map(|img| (c.name.clone(), img)))
        .collect()
}

pub struct ImageCheck {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for ImageCheck {
    fn name(&self) -> &'static str {
        "imagecheck"
    }
    fn help(&self) -> &'static str {
        "Compares a deployment's desired image per container against what's actually running"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: imagecheck <deployment>".into()));
        };
        let dep_api: Api<Deployment> = Api::namespaced(ctx.client(), &self.namespace);
        let dep = dep_api
            .get(name)
            .await
            .with_context(|| format!("getting deployment {name}"))?;

        let desired = desired_images(&dep);
        if desired.is_empty() {
            return Ok(Output::Text(format!(
                "Deployment {name:?} has no containers with an image set"
            )));
        }
        let selector = dep
            .spec
            .as_ref()
            .and_then(|s| s.selector.match_labels.clone())
            .unwrap_or_default();

        let pods = list_pods(ctx, &self.namespace).await?;
        let mut rows: Vec<[String; 5]> = Vec::new();
        for pod in &pods {
            let Some(labels) = pod.metadata.labels.as_ref() else {
                continue;
            };
            if !selector_matches(&selector, labels) {
                continue;
            }
            let pod_name = pod.metadata.name.clone().unwrap_or_default();
            let statuses = pod
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref());
            for (container_name, desired_image) in &desired {
                let running = statuses
                    .and_then(|ss| ss.iter().find(|cs| &cs.name == container_name))
                    .map(|cs| cs.image.clone())
                    .unwrap_or_else(|| "<not running>".into());
                let matched = running == *desired_image;
                rows.push([
                    pod_name.clone(),
                    container_name.clone(),
                    desired_image.clone(),
                    running,
                    matched.to_string(),
                ]);
            }
        }
        if rows.is_empty() {
            return Ok(Output::Text(format!(
                "No pods match deployment {name:?}'s selector"
            )));
        }
        Ok(Output::table(
            ["Pod", "Container", "Desired", "Running", "Match"],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::apps::v1::DeploymentSpec;
    use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;

    fn deployment(images: Vec<(&str, &str)>) -> Deployment {
        Deployment {
            metadata: Default::default(),
            spec: Some(DeploymentSpec {
                selector: LabelSelector {
                    match_labels: Some(BTreeMap::from([("app".to_string(), "web".to_string())])),
                    ..Default::default()
                },
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        containers: images
                            .into_iter()
                            .map(|(name, image)| Container {
                                name: name.into(),
                                image: Some(image.into()),
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
            status: None,
        }
    }

    #[test]
    fn selector_matches_requires_every_key_and_rejects_empty_selector() {
        let selector = BTreeMap::from([("app".to_string(), "web".to_string())]);
        let labels = BTreeMap::from([("app".to_string(), "web".to_string())]);
        assert!(selector_matches(&selector, &labels));
        assert!(!selector_matches(&BTreeMap::new(), &labels));
        assert!(!selector_matches(
            &selector,
            &BTreeMap::from([("app".to_string(), "other".to_string())])
        ));
    }

    #[test]
    fn desired_images_pairs_container_name_with_its_image() {
        let dep = deployment(vec![("app", "nginx:1.25"), ("sidecar", "envoy:1.0")]);
        assert_eq!(
            desired_images(&dep),
            vec![
                ("app".to_string(), "nginx:1.25".to_string()),
                ("sidecar".to_string(), "envoy:1.0".to_string()),
            ]
        );
    }
}
