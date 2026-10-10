//! `warnings` — every `Warning`-type event across all namespaces, newest
//! first.
//!
//! `events` (L4) is `kubectl get events` parity: one namespace, every
//! severity, oldest first. `warnings` is the fast "what's broken right now"
//! signal: only `Warning` events, across the whole cluster, **newest**
//! first — the opposite sort order from `events`, deliberately, since here
//! you want today's problem on top, not the oldest backlog entry.
//!
//! Cluster-wide, like `search`/`whois`: the per-namespace event listing runs
//! concurrently.

use anyhow::{Context as _, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use k8s_openapi::api::core::v1::{Event, Namespace};
use kube::api::{Api, ListParams};

use super::events::{event_age, last_seen_secs};
use super::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::age;

const CONCURRENCY: usize = 16;

async fn all_namespaces(ctx: &ClusterClient) -> Result<Vec<String>> {
    Ok(Api::<Namespace>::all(ctx.client())
        .list(&ListParams::default())
        .await
        .context("listing namespaces")?
        .items
        .into_iter()
        .filter_map(|n| n.metadata.name)
        .collect())
}

/// `(namespace, event)` for every `Warning`-type event in the cluster.
async fn find_warnings(ctx: &ClusterClient, namespaces: Vec<String>) -> Vec<(String, Event)> {
    let mut warnings = Vec::new();
    let mut tasks = FuturesUnordered::new();
    let mut iter = namespaces.into_iter();

    let spawn_one = |client: kube::Client, ns: String| async move {
        let events = Api::<Event>::namespaced(client, &ns)
            .list(&ListParams::default())
            .await
            .map(|l| l.items)
            .unwrap_or_default();
        (ns, events)
    };

    for _ in 0..CONCURRENCY {
        if let Some(ns) = iter.next() {
            tasks.push(spawn_one(ctx.client(), ns));
        }
    }
    while let Some((ns, events)) = tasks.next().await {
        warnings.extend(
            events
                .into_iter()
                .filter(|e| e.type_.as_deref() == Some("Warning"))
                .map(|e| (ns.clone(), e)),
        );
        if let Some(ns) = iter.next() {
            tasks.push(spawn_one(ctx.client(), ns));
        }
    }
    warnings
}

pub(crate) fn warnings_table(mut warnings: Vec<(String, Event)>) -> Output {
    warnings.sort_by_key(|(_, e)| std::cmp::Reverse(last_seen_secs(e)));
    let now = age::now_secs();
    let rows = warnings.iter().map(|(ns, event)| {
        let object = match &event.involved_object.kind {
            Some(kind) => format!(
                "{kind}/{}",
                event.involved_object.name.as_deref().unwrap_or("?")
            ),
            None => event.involved_object.name.clone().unwrap_or_default(),
        };
        [
            ns.clone(),
            event.reason.clone().unwrap_or_default(),
            object,
            event_age(event, now),
            event.message.clone().unwrap_or_default(),
        ]
    });
    Output::table(["Namespace", "Reason", "Object", "Age", "Message"], rows)
}

pub struct Warnings;

#[async_trait::async_trait]
impl Command for Warnings {
    fn name(&self) -> &'static str {
        "warnings"
    }
    fn help(&self) -> &'static str {
        "Every Warning event across all namespaces, newest first"
    }
    async fn run(&self, ctx: &ClusterClient, _args: &[String]) -> Result<Output> {
        let namespaces = all_namespaces(ctx).await?;
        Ok(warnings_table(find_warnings(ctx, namespaces).await))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::ObjectReference;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
    use k8s_openapi::jiff::ToSpan;

    fn event(kind: &str, name: &str, reason: &str, type_: &str, secs_ago: i64) -> Event {
        let ts = k8s_openapi::jiff::Timestamp::now() - secs_ago.seconds();
        Event {
            metadata: ObjectMeta::default(),
            involved_object: ObjectReference {
                kind: Some(kind.into()),
                name: Some(name.into()),
                ..Default::default()
            },
            reason: Some(reason.into()),
            type_: Some(type_.into()),
            message: Some("something happened".into()),
            last_timestamp: Some(Time(ts)),
            ..Default::default()
        }
    }

    #[test]
    fn warnings_table_sorts_newest_first_across_namespaces() {
        let warnings = vec![
            (
                "ns-a".to_string(),
                event("Pod", "web-0", "BackOff", "Warning", 3600),
            ),
            (
                "ns-b".to_string(),
                event("Pod", "worker-0", "FailedMount", "Warning", 10),
            ),
        ];
        let Output::Table { headers, rows } = warnings_table(warnings) else {
            panic!("expected table");
        };
        assert_eq!(headers, ["Namespace", "Reason", "Object", "Age", "Message"]);
        assert_eq!(rows.len(), 2);
        // newest (ns-b, ~10s ago) first
        assert_eq!(rows[0][0], "ns-b");
        assert_eq!(rows[0][1], "FailedMount");
        assert_eq!(rows[0][2], "Pod/worker-0");
        assert_eq!(rows[1][0], "ns-a");
        assert_eq!(rows[1][1], "BackOff");
    }
}
