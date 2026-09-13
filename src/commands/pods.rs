//! L4 `pods` and `delete` (`docs/behaviour-catalogue.md` L4).

use anyhow::{anyhow, Context as _, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, DeleteParams, ListParams};

use super::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::container_state;

pub(crate) fn pods_api(ctx: &ClusterClient, namespace: &str) -> Api<Pod> {
    Api::namespaced(ctx.client(), namespace)
}

pub(crate) async fn list_pods(ctx: &ClusterClient, namespace: &str) -> Result<Vec<Pod>> {
    Ok(pods_api(ctx, namespace)
        .list(&ListParams::default())
        .await
        .with_context(|| format!("listing pods in {namespace}"))?
        .items)
}

/// Resolve a `"<pod_index>.<container_index>"` selector (container part
/// optional) against a pod list. Mirrors Python `get_pod` + `get_container_name`.
pub(crate) fn resolve<'a>(pods: &'a [Pod], selector: &str) -> Result<(&'a Pod, Option<String>)> {
    let (pod_part, container_part) = match selector.split_once('.') {
        Some((p, c)) => (p, Some(c)),
        None => (selector, None),
    };
    let pod_idx: usize = pod_part
        .parse()
        .map_err(|_| anyhow!("selector must be <pod>.<container>, e.g. 0.0"))?;
    let pod = pods
        .get(pod_idx)
        .ok_or_else(|| anyhow!("The number is not in the list of pods"))?;

    let container = match container_part {
        None => None,
        Some(c) => {
            let cidx: usize = c
                .parse()
                .map_err(|_| anyhow!("selector must be <pod>.<container>, e.g. 0.0"))?;
            container_names(pod).into_iter().nth(cidx)
        }
    };
    Ok((pod, container))
}

fn container_names(pod: &Pod) -> Vec<String> {
    pod.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|cs| cs.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default()
}

/// Build the `pods` table: one row per container that has a status, keyed
/// `"<pod_index>.<container_index>"`. Pure so it can be unit-tested.
pub(crate) fn pods_table(pods: &[Pod]) -> Output {
    let mut rows: Vec<[String; 5]> = Vec::new();
    for (pi, pod) in pods.iter().enumerate() {
        let name = pod.metadata.name.clone().unwrap_or_default();
        let node = pod
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .unwrap_or_default();
        let statuses = pod
            .status
            .as_ref()
            .and_then(|s| s.container_statuses.as_ref());
        let Some(statuses) = statuses else { continue };
        for (ci, cs) in statuses.iter().enumerate() {
            rows.push([
                format!("{pi}.{ci}"),
                name.clone(),
                cs.name.clone(),
                container_state::derive(cs).label(),
                node.clone(),
            ]);
        }
    }
    Output::table(["N°", "Pod", "Container", "State", "Node"], rows)
}

/// `pods` — list every pod/container in the namespace.
pub struct PodsList {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodsList {
    fn name(&self) -> &'static str {
        "pods"
    }
    fn help(&self) -> &'static str {
        "Shows the pods"
    }
    async fn run(&self, ctx: &ClusterClient, _args: &[String]) -> Result<Output> {
        Ok(pods_table(&list_pods(ctx, &self.namespace).await?))
    }
}

/// `delete <pod>.<container>` — deletes the pod (container part ignored, as in
/// the Python version).
pub struct PodDelete {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodDelete {
    fn name(&self) -> &'static str {
        "delete"
    }
    fn help(&self) -> &'static str {
        "Delete the selected pod"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text("usage: delete <pod>.<container>".into()));
        };
        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, _) = resolve(&pods, selector)?;
        let name = pod
            .metadata
            .name
            .clone()
            .ok_or_else(|| anyhow!("pod has no name"))?;
        pods_api(ctx, &self.namespace)
            .delete(&name, &DeleteParams::default())
            .await
            .with_context(|| format!("deleting pod {name}"))?;
        Ok(Output::Text(format!("pod/{name} deleted")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateRunning, ContainerStatus, PodSpec, PodStatus,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn cs(name: &str, ready: bool) -> ContainerStatus {
        ContainerStatus {
            name: name.into(),
            ready,
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

    fn pod(name: &str, node: &str, statuses: Option<Vec<ContainerStatus>>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.into()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                node_name: Some(node.into()),
                ..Default::default()
            }),
            status: Some(PodStatus {
                container_statuses: statuses,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn table_has_one_row_per_container_with_pc_numbering() {
        let pods = vec![
            pod(
                "web-0",
                "node-a",
                Some(vec![cs("app", true), cs("proxy", false)]),
            ),
            pod("job-1", "node-b", Some(vec![cs("run", true)])),
        ];
        let Output::Table { headers, rows } = pods_table(&pods) else {
            panic!("expected table");
        };
        assert_eq!(headers, ["N°", "Pod", "Container", "State", "Node"]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], ["0.0", "web-0", "app", "Running", "node-a"]);
        assert_eq!(rows[1], ["0.1", "web-0", "proxy", "Not Ready", "node-a"]);
        assert_eq!(rows[2], ["1.0", "job-1", "run", "Running", "node-b"]);
    }

    #[test]
    fn pods_without_statuses_contribute_no_rows() {
        let pods = vec![pod("pending-0", "", None)];
        let Output::Table { rows, .. } = pods_table(&pods) else {
            panic!("expected table");
        };
        assert!(rows.is_empty());
    }

    #[test]
    fn resolve_pod_and_container() {
        let pods = vec![pod(
            "web-0",
            "n",
            Some(vec![cs("app", true), cs("proxy", true)]),
        )];
        let (p, c) = resolve(&pods, "0.1").unwrap();
        assert_eq!(p.metadata.name.as_deref(), Some("web-0"));
        assert_eq!(c.as_deref(), Some("proxy"));

        let (_, c_none) = resolve(&pods, "0").unwrap();
        assert!(c_none.is_none());

        assert!(resolve(&pods, "9").is_err());
    }
}
