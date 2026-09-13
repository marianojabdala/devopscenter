//! `ns` verbs: `list`, `create <name>`, `delete <name>`. Entering a namespace
//! (`ns <name>`) is handled by the REPL, which switches to the L4 command set
//! (`docs/behaviour-catalogue.md` L3/L4).

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, ListParams, PostParams};

use super::{Command, Output};
use crate::config::ClusterClient;

fn api(ctx: &ClusterClient) -> Api<Namespace> {
    Api::all(ctx.client())
}

pub(crate) async fn namespace_names(ctx: &ClusterClient) -> Result<Vec<String>> {
    let list = api(ctx)
        .list(&ListParams::default())
        .await
        .context("listing namespaces")?;
    Ok(list
        .items
        .into_iter()
        .filter_map(|n| n.metadata.name)
        .collect())
}

/// `ns list` — every namespace, API order.
pub struct NamespacesList;

#[async_trait::async_trait]
impl Command for NamespacesList {
    fn name(&self) -> &'static str {
        "list"
    }
    fn help(&self) -> &'static str {
        "Shows all the namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, _args: &[String]) -> Result<Output> {
        let rows: Vec<[String; 1]> = namespace_names(ctx)
            .await?
            .into_iter()
            .map(|n| [n])
            .collect();
        Ok(Output::table(["Namespace Name"], rows))
    }
}

/// `ns create <name>`.
pub struct NamespaceCreate;

#[async_trait::async_trait]
impl Command for NamespaceCreate {
    fn name(&self) -> &'static str {
        "create"
    }
    fn help(&self) -> &'static str {
        "Creates the given namespace"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: create <namespace>".into()));
        };
        if namespace_names(ctx).await?.iter().any(|n| n == name) {
            return Ok(Output::Text("Namespace already exists!".into()));
        }
        let ns = Namespace {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        api(ctx)
            .create(&PostParams::default(), &ns)
            .await
            .with_context(|| format!("creating namespace {name}"))?;
        Ok(Output::Text(format!("namespace/{name} created")))
    }
}

/// `ns delete <name>` — issues the delete and returns immediately. Unlike the
/// Python version it does **not** block the REPL polling for termination
/// (migration.md O7).
pub struct NamespaceDelete;

#[async_trait::async_trait]
impl Command for NamespaceDelete {
    fn name(&self) -> &'static str {
        "delete"
    }
    fn help(&self) -> &'static str {
        "Delete the given namespace"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: delete <namespace>".into()));
        };
        if !namespace_names(ctx).await?.iter().any(|n| n == name) {
            return Ok(Output::Text("Namespace doesn't exists!".into()));
        }
        api(ctx)
            .delete(name, &DeleteParams::default())
            .await
            .with_context(|| format!("deleting namespace {name}"))?;
        Ok(Output::Text(format!(
            "namespace/{name} deletion requested (terminating in the background)"
        )))
    }
}
