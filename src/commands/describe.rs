//! L4 `describe <pod_index>` — a `kubectl describe pod` style summary of one
//! pod. Reuses the same `<pod_index>[.<container_index>]` selector as
//! `logs`/`exec`/`delete` (the container part, if given, is accepted but
//! ignored — describing always shows every container).

use anyhow::Result;
use k8s_openapi::api::core::v1::{Container, ContainerState, ContainerStatus, Pod};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::events::{event_age, last_seen_secs, list_events};
use super::pods::{list_pods, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::age;

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

/// Expand a `ContainerState` into `kubectl describe`-style lines: the kind on
/// the `label` line, then indented detail — critically, the **message** for
/// `Waiting` (e.g. the actual "back-off restarting failed container..." text,
/// not just the `CrashLoopBackOff` reason word) and the **last termination**
/// (exit code, reason, timestamps), which is where "why is this crashing"
/// actually lives and the old single-line `State: Waiting-X` label lost.
fn describe_state(label: &str, state: &ContainerState) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(t) = &state.terminated {
        lines.push(format!("{label}Terminated"));
        lines.push(format!(
            "      Reason:       {}",
            t.reason.as_deref().unwrap_or("")
        ));
        lines.push(format!("      Exit Code:    {}", t.exit_code));
        if let Some(s) = &t.started_at {
            lines.push(format!("      Started:      {}", s.0));
        }
        if let Some(f) = &t.finished_at {
            lines.push(format!("      Finished:     {}", f.0));
        }
        if let Some(m) = t.message.as_deref().filter(|m| !m.is_empty()) {
            lines.push(format!("      Message:      {m}"));
        }
    } else if let Some(w) = &state.waiting {
        lines.push(format!("{label}Waiting"));
        lines.push(format!(
            "      Reason:       {}",
            w.reason.as_deref().unwrap_or("")
        ));
        if let Some(m) = w.message.as_deref().filter(|m| !m.is_empty()) {
            lines.push(format!("      Message:      {m}"));
        }
    } else if let Some(r) = &state.running {
        lines.push(format!("{label}Running"));
        if let Some(s) = &r.started_at {
            lines.push(format!("      Started:      {}", s.0));
        }
    } else {
        lines.push(format!("{label}Unknown"));
    }
    lines
}

/// Does this `ContainerState` actually carry anything (as opposed to a
/// default/empty `ContainerState` with all three variants `None`)?
fn state_is_present(state: &ContainerState) -> bool {
    state.terminated.is_some() || state.waiting.is_some() || state.running.is_some()
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
                    match cs.state.as_ref().filter(|s| state_is_present(s)) {
                        Some(state) => {
                            for line in describe_state("    State:          ", state) {
                                let _ = writeln!(out, "{line}");
                            }
                        }
                        None => {
                            let _ = writeln!(out, "    State:          Unknown");
                        }
                    }
                    if let Some(last) = cs.last_state.as_ref().filter(|s| state_is_present(s)) {
                        for line in describe_state("    Last State:     ", last) {
                            let _ = writeln!(out, "{line}");
                        }
                    }
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
        let mut text = describe_pod(pod);

        // Best-effort: a pod described without RBAC on events still gets
        // everything above, just no Events section.
        if let (Some(pod_name), Ok(events)) = (
            pod.metadata.name.as_deref(),
            list_events(ctx, &self.namespace).await,
        ) {
            let mut relevant: Vec<_> = events
                .iter()
                .filter(|e| e.involved_object.name.as_deref() == Some(pod_name))
                .collect();
            if !relevant.is_empty() {
                relevant.sort_by_key(|e| last_seen_secs(e));
                let now = age::now_secs();
                let _ = write!(text, "\n\nEvents:\n");
                for e in relevant {
                    let _ = writeln!(
                        text,
                        "  {:<8} {:<20} {:<6} {}",
                        e.type_.as_deref().unwrap_or(""),
                        e.reason.as_deref().unwrap_or(""),
                        event_age(e, now),
                        e.message.as_deref().unwrap_or(""),
                    );
                }
                text = text.trim_end().to_string();
            }
        }

        Ok(Output::Text(text))
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

    #[test]
    fn describe_surfaces_crashloop_reason_message_and_last_termination() {
        use k8s_openapi::api::core::v1::{ContainerStateTerminated, ContainerStateWaiting};

        let mut p = pod();
        let cs = &mut p
            .status
            .as_mut()
            .unwrap()
            .container_statuses
            .as_mut()
            .unwrap()[0];
        cs.ready = false;
        cs.restart_count = 7;
        cs.state = Some(ContainerState {
            waiting: Some(ContainerStateWaiting {
                reason: Some("CrashLoopBackOff".into()),
                message: Some(
                    "back-off 5m0s restarting failed container=app pod=web-0_default".into(),
                ),
            }),
            ..Default::default()
        });
        cs.last_state = Some(ContainerState {
            terminated: Some(ContainerStateTerminated {
                reason: Some("Error".into()),
                exit_code: 1,
                ..Default::default()
            }),
            ..Default::default()
        });

        let text = describe_pod(&p);
        // This is the exact information a "why is it crash-looping" question
        // needs, and the pre-fix version never printed any of it — only the
        // single line `State: Waiting-CrashLoopBackOff`.
        assert!(text.contains("State:          Waiting"));
        assert!(text.contains("Reason:       CrashLoopBackOff"));
        assert!(text.contains(
            "Message:      back-off 5m0s restarting failed container=app pod=web-0_default"
        ));
        assert!(text.contains("Last State:     Terminated"));
        assert!(text.contains("Reason:       Error"));
        assert!(text.contains("Exit Code:    1"));
    }

    #[test]
    fn describe_omits_last_state_when_pod_never_restarted() {
        // A default/empty ContainerState (no running/waiting/terminated set)
        // must not print a bogus "Last State: Unknown" line.
        let mut p = pod();
        p.status
            .as_mut()
            .unwrap()
            .container_statuses
            .as_mut()
            .unwrap()[0]
            .last_state = Some(ContainerState::default());
        let text = describe_pod(&p);
        assert!(!text.contains("Last State"));
    }
}
