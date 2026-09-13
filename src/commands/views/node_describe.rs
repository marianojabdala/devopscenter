//! `describe-node <name>` — a `kubectl describe node` style summary.
//! Companion to the pod-level `describe`; no Python equivalent existed.

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::Node;
use kube::api::Api;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::nodes::{roles, status};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

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

pub(crate) fn describe_node(node: &Node) -> String {
    let mut out = String::new();
    let meta = &node.metadata;
    let spec = node.spec.as_ref();
    let node_status = node.status.as_ref();

    let _ = writeln!(
        out,
        "Name:               {}",
        meta.name.as_deref().unwrap_or("")
    );
    let _ = writeln!(out, "Roles:              {}", roles(node));
    let _ = writeln!(out, "Status:             {}", status(node));
    let _ = writeln!(
        out,
        "Labels:             {}",
        join_map(meta.labels.as_ref())
    );
    let _ = writeln!(
        out,
        "Annotations:        {}",
        join_map(meta.annotations.as_ref())
    );

    if let Some(addrs) = node_status
        .and_then(|s| s.addresses.as_ref())
        .filter(|a| !a.is_empty())
    {
        let joined = addrs
            .iter()
            .map(|a| format!("{}={}", a.type_, a.address))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "Addresses:          {joined}");
    }

    if let Some(info) = node_status.and_then(|s| s.node_info.as_ref()) {
        let _ = writeln!(out, "OS Image:           {}", info.os_image);
        let _ = writeln!(out, "Kernel Version:     {}", info.kernel_version);
        let _ = writeln!(
            out,
            "Container Runtime:  {}",
            info.container_runtime_version
        );
        let _ = writeln!(out, "Kubelet Version:    {}", info.kubelet_version);
        let _ = writeln!(
            out,
            "Architecture:       {}/{}",
            info.operating_system, info.architecture
        );
    }

    if let Some(taints) = spec
        .and_then(|s| s.taints.as_ref())
        .filter(|t| !t.is_empty())
    {
        let _ = writeln!(out, "\nTaints:");
        for t in taints {
            let value = t.value.as_deref().unwrap_or("");
            let _ = writeln!(out, "  {}={}:{}", t.key, value, t.effect);
        }
    } else {
        let _ = writeln!(out, "Taints:             <none>");
    }

    let _ = writeln!(out, "\nCapacity / Allocatable:");
    let capacity = node_status.and_then(|s| s.capacity.as_ref());
    let allocatable = node_status.and_then(|s| s.allocatable.as_ref());
    for key in ["cpu", "memory", "pods", "ephemeral-storage"] {
        let cap = capacity
            .and_then(|m| m.get(key))
            .map(|q| q.0.as_str())
            .unwrap_or("-");
        let alloc = allocatable
            .and_then(|m| m.get(key))
            .map(|q| q.0.as_str())
            .unwrap_or("-");
        let _ = writeln!(out, "  {key:<18} capacity={cap:<12} allocatable={alloc}");
    }

    if let Some(conditions) = node_status
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

/// `describe-node <name>` — full detail on one node (not filtered/indexed
/// like pods; node names are unique and stable, so we take the name directly).
pub struct NodeDescribe;

#[async_trait::async_trait]
impl Command for NodeDescribe {
    fn name(&self) -> &'static str {
        "describe-node"
    }
    fn help(&self) -> &'static str {
        "Describe one node by name (like `kubectl describe node`)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: describe-node <node-name>".into()));
        };
        let node = Api::<Node>::all(ctx.client())
            .get(name)
            .await
            .with_context(|| format!("getting node {name}"))?;
        Ok(Output::Text(describe_node(&node)))
    }
}
