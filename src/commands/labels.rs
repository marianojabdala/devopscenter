//! L4 `labels <pod>.<container>|<name-substring>`.
//!
//! The `pods` table crams every pod's labels into one comma-joined cell,
//! which is hard to scan once a pod has more than a couple. `labels` instead
//! renders one row per key/value pair, and — reusing the same selector
//! resolution as `logs` — can show them for a single pod or for every pod
//! matching a name substring at once, so mismatched labels across a
//! deployment's replicas are easy to spot.

use anyhow::Result;

use super::pods::{filter_by_name, is_index_selector, list_pods, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;
use k8s_openapi::api::core::v1::Pod;

/// `(key, value)` pairs for one pod, sorted by key (`BTreeMap` iteration
/// order); `[("<none>", "")]` if the pod has no labels.
fn label_rows(pod: &Pod) -> Vec<[String; 2]> {
    match &pod.metadata.labels {
        Some(labels) if !labels.is_empty() => {
            labels.iter().map(|(k, v)| [k.clone(), v.clone()]).collect()
        }
        _ => vec![["<none>".into(), String::new()]],
    }
}

pub struct PodLabels {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodLabels {
    fn name(&self) -> &'static str {
        "labels"
    }
    fn help(&self) -> &'static str {
        "Shows pod labels as a Key/Value table: labels <pod>.<container>, or labels <name-substring> for every matching pod"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text(
                "Error you should select the number of the pod to show labels for. Eg labels 0"
                    .into(),
            ));
        };
        let pods = list_pods(ctx, &self.namespace).await?;

        if is_index_selector(selector) {
            let (pod, _) = resolve(&pods, selector)?;
            return Ok(Output::table(["Key", "Value"], label_rows(pod)));
        }

        let matches = filter_by_name(&pods, selector);
        if matches.is_empty() {
            return Ok(Output::Text(format!(
                "No pods match {selector:?} in {}",
                self.namespace
            )));
        }
        let rows: Vec<[String; 3]> = matches
            .into_iter()
            .flat_map(|pod| {
                let name = pod.metadata.name.clone().unwrap_or_default();
                label_rows(pod)
                    .into_iter()
                    .map(move |[k, v]| [name.clone(), k, v])
            })
            .collect();
        Ok(Output::table(["Pod", "Key", "Value"], rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn pod(name: &str, labels: Option<BTreeMap<String, String>>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.into()),
                labels,
                ..Default::default()
            },
            spec: None,
            status: None,
        }
    }

    #[test]
    fn label_rows_returns_sorted_key_value_pairs() {
        let labels = BTreeMap::from([
            ("version".to_string(), "v2".to_string()),
            ("app".to_string(), "web".to_string()),
        ]);
        let p = pod("web-0", Some(labels));
        assert_eq!(
            label_rows(&p),
            vec![
                ["app".to_string(), "web".to_string()],
                ["version".to_string(), "v2".to_string()],
            ]
        );
    }

    #[test]
    fn label_rows_reports_none_when_pod_has_no_labels() {
        let p = pod("web-0", None);
        assert_eq!(label_rows(&p), vec![["<none>".to_string(), String::new()]]);
    }
}
