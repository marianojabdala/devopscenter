//! L4 `describe <pod_index>` — a `kubectl describe pod` style summary of one
//! pod. Reuses the same `<pod_index>[.<container_index>]` selector as
//! `logs`/`exec`/`delete` (the container part, if given, is accepted but
//! ignored — describing always shows every container).

use anyhow::Result;
use k8s_openapi::api::core::v1::{Container, ContainerStatus, Pod};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::pods::{list_pods, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::container_state;

fn join_map(map: Option<&BTreeMap<String, String>>) -> String {
    match map {
        None => "<none>".to_string(),
        Some(m) if m.is_empty() => "<none>".to_string(),
        Some(m) => m
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn resources_line(resources: Option<&BTreeMap<String, Quantity>>) -> String {
    match resources {
        None => "<none>".to_string(),
        Some(m) if m.is_empty() => "<none>".to_string(),
        Some(m) => m
            .iter()
            .map(|(k, v)| format!("{k}={}", v.0))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn ports_line(container: &Container) -> String {
    match &container.ports {
        None => "<none>".to_string(),
        Some(ports) if ports.is_empty() => "<none>".to_string(),
        Some(ports) => ports
            .iter()
            .map(|p| {
                let proto = p.protocol.as_deref().unwrap_or("TCP");
                match p.name.as_deref() {
                    Some(name) => format!("{name}:{}/{proto}", p.container_port),
                    None => format!("{}/{proto}", p.container_port),
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn status_for<'a>(pod: &'a Pod, name: &str) -> Option<&'a ContainerStatus> {
    pod.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .and_then(|statuses| statuses.iter().find(|cs| cs.name == name))
}

/// Build the description text. Pure so it can be unit-tested against a
/// hand-built `Pod`.
pub(crate) fn describe_pod(pod: &Pod) -> String {
    let mut out = String::new();
    let meta = &pod.metadata;
    let spec = pod.spec.as_ref();
    let status = pod.status.as_ref();

    let _ = writeln!(out, "Name:         {}", meta.name.as_deref().unwrap_or(""));
    let _ = writeln!(
        out,
        "Namespace:    {}",
        meta.namespace.as_deref().unwrap_or("")
    );
    let _ = writeln!(
        out,
        "Node:         {}",
        spec.and_then(|s| s.node_name.as_deref()).unwrap_or("")
    );
    let _ = writeln!(
        out,
        "Start Time:   {}",
        meta.creation_timestamp
            .as_ref()
            .map(|t| t.0.to_string())
            .unwrap_or_default()
    );
    let _ = writeln!(out, "Labels:       {}", join_map(meta.labels.as_ref()));
    let _ = writeln!(out, "Annotations:  {}", join_map(meta.annotations.as_ref()));
    let _ = writeln!(
        out,
        "Status:       {}",
        status.and_then(|s| s.phase.as_deref()).unwrap_or("")
    );
    let _ = writeln!(
        out,
        "IP:           {}",
        status.and_then(|s| s.pod_ip.as_deref()).unwrap_or("")
    );
    if let Some(refs) = meta.owner_references.as_ref().filter(|r| !r.is_empty()) {
        let owners = refs
            .iter()
            .map(|r| format!("{}/{}", r.kind, r.name))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "Controlled By: {owners}");
    }
    let _ = writeln!(
        out,
        "QoS Class:    {}",
        status.and_then(|s| s.qos_class.as_deref()).unwrap_or("")
    );

    let _ = writeln!(out, "\nContainers:");
    if let Some(containers) = spec.map(|s| &s.containers) {
        for container in containers {
            let _ = writeln!(out, "  {}:", container.name);
            let _ = writeln!(
                out,
                "    Image:          {}",
                container.image.as_deref().unwrap_or("")
            );
            let _ = writeln!(out, "    Ports:          {}", ports_line(container));
            match status_for(pod, &container.name) {
                Some(cs) => {
                    let _ = writeln!(
                        out,
                        "    State:          {}",
                        container_state::derive(cs).label()
                    );
                    let _ = writeln!(out, "    Ready:          {}", cs.ready);
                    let _ = writeln!(out, "    Restart Count:  {}", cs.restart_count);
                }
                None => {
                    let _ = writeln!(out, "    State:          <no status>");
                }
            }
            let resources = container.resources.as_ref();
            let _ = writeln!(
                out,
                "    Requests:       {}",
                resources_line(resources.and_then(|r| r.requests.as_ref()))
            );
            let _ = writeln!(
                out,
                "    Limits:         {}",
                resources_line(resources.and_then(|r| r.limits.as_ref()))
            );
        }
    }

    if let Some(conditions) = status
        .and_then(|s| s.conditions.as_ref())
        .filter(|c| !c.is_empty())
    {
        let _ = writeln!(out, "\nConditions:");
        for cond in conditions {
            let _ = writeln!(out, "  {:<20} {}", cond.type_, cond.status);
        }
    }

    out.trim_end().to_string()
}

pub struct PodDescribe {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for PodDescribe {
    fn name(&self) -> &'static str {
        "describe"
    }
    fn help(&self) -> &'static str {
        "Describe the selected pod (like `kubectl describe pod`)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text("usage: describe <pod_index>".into()));
        };
        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, _) = resolve(&pods, selector)?;
        Ok(Output::Text(describe_pod(pod)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{ContainerState, ContainerStateRunning, PodSpec, PodStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn pod() -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some("web-0".into()),
                namespace: Some("default".into()),
                labels: Some(BTreeMap::from([("app".into(), "web".into())])),
                ..Default::default()
            },
            spec: Some(PodSpec {
                node_name: Some("node-a".into()),
                containers: vec![Container {
                    name: "app".into(),
                    image: Some("nginx:1.25".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            status: Some(PodStatus {
                phase: Some("Running".into()),
                pod_ip: Some("10.0.0.5".into()),
                container_statuses: Some(vec![ContainerStatus {
                    name: "app".into(),
                    ready: true,
                    restart_count: 2,
                    image: "nginx:1.25".into(),
                    image_id: String::new(),
                    state: Some(ContainerState {
                        running: Some(ContainerStateRunning::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn describe_includes_metadata_and_container_details() {
        let text = describe_pod(&pod());
        assert!(text.contains("Name:         web-0"));
        assert!(text.contains("Namespace:    default"));
        assert!(text.contains("Node:         node-a"));
        assert!(text.contains("Status:       Running"));
        assert!(text.contains("IP:           10.0.0.5"));
        assert!(text.contains("Labels:       app=web"));
        assert!(text.contains("app:"));
        assert!(text.contains("Image:          nginx:1.25"));
        assert!(text.contains("State:          Running"));
        assert!(text.contains("Ready:          true"));
        assert!(text.contains("Restart Count:  2"));
    }

    #[test]
    fn describe_handles_container_without_status() {
        let mut p = pod();
        p.status.as_mut().unwrap().container_statuses = None;
        let text = describe_pod(&p);
        assert!(text.contains("State:          <no status>"));
    }
}
