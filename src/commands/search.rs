//! `search <substring>` — namespaces containing a pod whose name matches
//! (`docs/behaviour-catalogue.md` L3-search).
//!
//! The per-namespace pod listing runs concurrently (migration.md O9); the
//! observable contract is unchanged: the set of matching namespace names.

use std::collections::BTreeSet;

use anyhow::{Context as _, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::api::{Api, ListParams};

use super::{Command, Output};
use crate::config::ClusterClient;

const CONCURRENCY: usize = 16;

fn namespace_matches<I>(pod_names: I, needle: &str) -> bool
where
    I: IntoIterator<Item = String>,
{
    pod_names.into_iter().any(|n| n.contains(needle))
}

/// Namespaces (sorted) that contain at least one pod whose name contains
/// `needle` as a substring.
pub async fn find_namespaces(ctx: &ClusterClient, needle: &str) -> Result<BTreeSet<String>> {
    let ns_api: Api<Namespace> = Api::all(ctx.client());
    let namespaces: Vec<String> = ns_api
        .list(&ListParams::default())
        .await
        .context("listing namespaces")?
        .items
        .into_iter()
        .filter_map(|n| n.metadata.name)
        .collect();

    let mut hits = BTreeSet::new();
    let mut tasks = FuturesUnordered::new();
    let mut iter = namespaces.into_iter();

    let spawn_one = |name: String| {
        let client = ctx.client();
        async move {
            let api: Api<Pod> = Api::namespaced(client, &name);
            let names = api
                .list(&ListParams::default())
                .await
                .map(|l| {
                    l.items
                        .into_iter()
                        .filter_map(|p| p.metadata.name)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (name, names)
        }
    };

    for _ in 0..CONCURRENCY {
        if let Some(n) = iter.next() {
            tasks.push(spawn_one(n));
        }
    }
    while let Some((name, pod_names)) = tasks.next().await {
        if namespace_matches(pod_names, needle) {
            hits.insert(name);
        }
        if let Some(n) = iter.next() {
            tasks.push(spawn_one(n));
        }
    }
    Ok(hits)
}

/// `search <substring>`.
pub struct PodSearch;

#[async_trait::async_trait]
impl Command for PodSearch {
    fn name(&self) -> &'static str {
        "search"
    }
    fn help(&self) -> &'static str {
        "Look for a microservice across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(needle) = args.get(1) else {
            return Ok(Output::Text(
                "You have to add the microservice to search".into(),
            ));
        };
        let hits = find_namespaces(ctx, needle).await?;
        if hits.is_empty() {
            return Ok(Output::Text("Not found".into()));
        }
        let rows: Vec<[String; 1]> = hits.into_iter().map(|n| [n]).collect();
        Ok(Output::table(["Namespace"], rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_is_substring_not_exact() {
        let names = || vec!["payments-api-abc".to_string(), "redis-0".to_string()];
        assert!(namespace_matches(names(), "payments"));
        assert!(namespace_matches(names(), "edis"));
        assert!(!namespace_matches(names(), "worker"));
        assert!(!namespace_matches(Vec::<String>::new(), "x"));
    }
}
