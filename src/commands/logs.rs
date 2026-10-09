//! L4 `logs <pod>.<container>|<name-substring> [--filter TERM] [-p|--previous]`.
//!
//! A numeric selector (`0.0`, `0`) behaves as before: one pod/container. Any
//! other selector is a case-insensitive substring match against pod names
//! (same rule as `PodsFilter::Name`), pulling logs from every matching pod's
//! every container so you can tail a whole deployment at once.
//!
//! The Python version streams a non-follow log line by line; fetching the whole
//! (finite) log and printing it is observably equivalent.

use anyhow::{anyhow, Result};
use futures::future::join_all;
use k8s_openapi::api::core::v1::Pod;
use kube::api::LogParams;

use super::pods::{
    container_names, filter_by_name, is_index_selector, list_pods, pods_api, resolve,
};
use super::{Command, Output};
use crate::config::ClusterClient;

/// `-p`/`--previous`: show the log of the *previous* (already terminated)
/// instance of the container. This is the single most useful thing to check
/// on a crash-looping pod — the current instance's log is often empty.
fn wants_previous(args: &[String]) -> bool {
    args.iter().any(|a| a == "-p" || a == "--previous")
}

/// `--filter TERM`: keep only output lines containing `TERM` (case-insensitive).
fn filter_term(args: &[String]) -> Option<&str> {
    args.iter()
        .position(|a| a == "--filter")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// Everything before an optional `--filter TERM` pair — i.e. the selector
/// plus flags like `-p`/`--previous`.
fn selector_args(args: &[String]) -> Vec<String> {
    match args.iter().position(|a| a == "--filter") {
        Some(i) => {
            let mut v = args[..i].to_vec();
            v.extend(args[i + 2..].iter().cloned());
            v
        }
        None => args.to_vec(),
    }
}

/// Every `(pod_name, container_name)` pair for pods whose name contains
/// `name_filter` (case-insensitive). A pod with no reported containers is
/// skipped — there's nothing to fetch logs for yet.
fn matching_pod_containers(pods: &[Pod], name_filter: &str) -> Vec<(String, String)> {
    filter_by_name(pods, name_filter)
        .into_iter()
        .flat_map(|p| {
            let name = p.metadata.name.clone().unwrap_or_default();
            container_names(p)
                .into_iter()
                .map(move |c| (name.clone(), c))
        })
        .collect()
}

/// Apply an optional case-insensitive substring `filter` to `text`, one line
/// at a time.
fn apply_filter(text: &str, filter: Option<&str>) -> String {
    let Some(term) = filter else {
        return text.to_string();
    };
    let needle = term.to_lowercase();
    text.lines()
        .filter(|line| line.to_lowercase().contains(&needle))
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct PodLogs {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodLogs {
    fn name(&self) -> &'static str {
        "logs"
    }
    fn help(&self) -> &'static str {
        "Shows pod logs: logs <pod>.<container>, or logs <name-substring> for every matching pod \
         (-p/--previous for the last terminated instance, --filter TERM to keep matching lines)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let sel_args = selector_args(args);
        let Some(selector) = sel_args.get(1) else {
            return Ok(Output::Text(
                "Error you should select the number of the pod to show the log. Eg logs 0.0".into(),
            ));
        };
        let previous = wants_previous(args);
        let filter = filter_term(args);
        let pods = list_pods(ctx, &self.namespace).await?;

        if is_index_selector(selector) {
            let (pod, container) = resolve(&pods, selector)?;
            let pod_name = pod
                .metadata
                .name
                .clone()
                .ok_or_else(|| anyhow!("pod has no name"))?;
            let text = fetch_log(ctx, &self.namespace, &pod_name, container, previous).await;
            return Ok(Output::Text(apply_filter(&text, filter)));
        }

        let targets = matching_pod_containers(&pods, selector);
        if targets.is_empty() {
            return Ok(Output::Text(format!(
                "No pods match {selector:?} in {}",
                self.namespace
            )));
        }

        let fetches = targets.into_iter().map(|(pod_name, container)| {
            let ctx = ctx.clone();
            let namespace = self.namespace.clone();
            async move {
                let text = fetch_log(
                    &ctx,
                    &namespace,
                    &pod_name,
                    Some(container.clone()),
                    previous,
                )
                .await;
                text.lines()
                    .map(|line| format!("{pod_name}/{container}: {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        });
        let combined = join_all(fetches)
            .await
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Output::Text(apply_filter(&combined, filter)))
    }
}

/// Fetch one pod/container's log text, turning a fetch error into an inline
/// message rather than failing the whole (possibly multi-pod) command.
async fn fetch_log(
    ctx: &ClusterClient,
    namespace: &str,
    pod_name: &str,
    container: Option<String>,
    previous: bool,
) -> String {
    let params = LogParams {
        container,
        follow: false,
        timestamps: false,
        previous,
        ..Default::default()
    };
    match pods_api(ctx, namespace).logs(pod_name, &params).await {
        Ok(text) => text,
        Err(err) => format!(
            "<error {} logs for {pod_name}: {err}>",
            if previous {
                "reading previous"
            } else {
                "reading"
            }
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wants_previous_recognises_short_and_long_flags() {
        assert!(!wants_previous(&["logs".into(), "0.0".into()]));
        assert!(wants_previous(&["logs".into(), "0.0".into(), "-p".into()]));
        assert!(wants_previous(&[
            "logs".into(),
            "0.0".into(),
            "--previous".into()
        ]));
    }

    #[test]
    fn filter_term_extracts_the_value_after_the_flag() {
        assert_eq!(filter_term(&["logs".into(), "0.0".into()]), None);
        assert_eq!(
            filter_term(&[
                "logs".into(),
                "0.0".into(),
                "--filter".into(),
                "error".into()
            ]),
            Some("error")
        );
    }

    #[test]
    fn selector_args_strips_the_filter_pair() {
        assert_eq!(
            selector_args(&[
                "logs".into(),
                "web".into(),
                "--filter".into(),
                "error".into(),
                "-p".into()
            ]),
            vec!["logs".to_string(), "web".to_string(), "-p".to_string()]
        );
    }

    #[test]
    fn is_index_selector_distinguishes_numeric_from_name_selectors() {
        assert!(is_index_selector("0"));
        assert!(is_index_selector("0.1"));
        assert!(!is_index_selector("web"));
        assert!(!is_index_selector("api-users"));
    }

    #[test]
    fn apply_filter_keeps_only_matching_lines_case_insensitively() {
        let text = "hello\nERROR: boom\nok";
        assert_eq!(apply_filter(text, Some("error")), "ERROR: boom");
        assert_eq!(apply_filter(text, None), text);
    }

    #[test]
    fn matching_pod_containers_matches_by_substring_across_all_containers() {
        use k8s_openapi::api::core::v1::{
            ContainerState, ContainerStateRunning, ContainerStatus, PodStatus,
        };
        use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

        fn cs(name: &str) -> ContainerStatus {
            ContainerStatus {
                name: name.into(),
                ready: true,
                image: "i".into(),
                image_id: String::new(),
                restart_count: 0,
                state: Some(ContainerState {
                    running: Some(ContainerStateRunning::default()),
                    ..Default::default()
                }),
                ..Default::default()
            }
        }
        fn pod(name: &str, containers: Vec<ContainerStatus>) -> Pod {
            Pod {
                metadata: ObjectMeta {
                    name: Some(name.into()),
                    ..Default::default()
                },
                spec: None,
                status: Some(PodStatus {
                    container_statuses: Some(containers),
                    ..Default::default()
                }),
            }
        }

        let pods = vec![
            pod("api-users-0", vec![cs("app"), cs("sidecar")]),
            pod("api-orders-0", vec![cs("app")]),
        ];
        let matches = matching_pod_containers(&pods, "api-users");
        assert_eq!(
            matches,
            vec![
                ("api-users-0".to_string(), "app".to_string()),
                ("api-users-0".to_string(), "sidecar".to_string()),
            ]
        );
    }
}
