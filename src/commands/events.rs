//! L4 `events` — every event in the namespace, oldest first (`kubectl get
//! events` parity). Unlike `pods`/`logs`/`exec`/`delete`, this is not
//! selector-based: it shows all objects' events, since a single pod's events
//! are just a filtered view of the same list.

use anyhow::{Context as _, Result};
use k8s_openapi::api::core::v1::Event;
use kube::api::{Api, ListParams};

use super::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::age;

pub(crate) async fn list_events(ctx: &ClusterClient, namespace: &str) -> Result<Vec<Event>> {
    Ok(Api::<Event>::namespaced(ctx.client(), namespace)
        .list(&ListParams::default())
        .await
        .with_context(|| format!("listing events in {namespace}"))?
        .items)
}

/// Sort key: last-seen time (falls back to first-seen, then event_time),
/// oldest first — matches `kubectl get events`.
pub(crate) fn last_seen_secs(event: &Event) -> i64 {
    event
        .last_timestamp
        .as_ref()
        .map(|t| t.0.as_second())
        .or_else(|| event.first_timestamp.as_ref().map(|t| t.0.as_second()))
        .or_else(|| event.event_time.as_ref().map(|t| t.0.as_second()))
        .unwrap_or(0)
}

/// Age string for one event, matching the `events` table's rule: last-seen
/// time, falling back to `event_time`, else `<unknown>`.
pub(crate) fn event_age(event: &Event, now: i64) -> String {
    event
        .last_timestamp
        .as_ref()
        .map(|t| age::format_secs(age::secs_since(now, t)))
        .or_else(|| {
            event
                .event_time
                .as_ref()
                .map(|t| age::format_secs(age::secs_since_micro(now, t)))
        })
        .unwrap_or_else(|| "<unknown>".into())
}

pub(crate) fn events_table(mut events: Vec<Event>) -> Output {
    events.sort_by_key(last_seen_secs);
    let now = age::now_secs();
    let rows = events.iter().map(|event| {
        let age = event_age(event, now);
        let object = match &event.involved_object.kind {
            Some(kind) => format!(
                "{kind}/{}",
                event.involved_object.name.as_deref().unwrap_or("?")
            ),
            None => event.involved_object.name.clone().unwrap_or_default(),
        };
        [
            event.type_.clone().unwrap_or_default(),
            event.reason.clone().unwrap_or_default(),
            object,
            age,
            event.message.clone().unwrap_or_default(),
        ]
    });
    Output::table(["Type", "Reason", "Object", "Age", "Message"], rows)
}

/// `events` — every event in the namespace.
pub struct NamespaceEvents {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for NamespaceEvents {
    fn name(&self) -> &'static str {
        "events"
    }
    fn help(&self) -> &'static str {
        "Shows every event in the namespace, oldest first"
    }
    async fn run(&self, ctx: &ClusterClient, _args: &[String]) -> Result<Output> {
        Ok(events_table(list_events(ctx, &self.namespace).await?))
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
    fn sorted_oldest_first_with_expected_columns() {
        let events = vec![
            event("Pod", "web-0", "Started", "Normal", 10),
            event("Node", "node-a", "NodeNotReady", "Warning", 3600),
        ];
        let Output::Table { headers, rows } = events_table(events) else {
            panic!("expected table");
        };
        assert_eq!(headers, ["Type", "Reason", "Object", "Age", "Message"]);
        assert_eq!(rows.len(), 2);
        // oldest (node, ~1h ago) first
        assert_eq!(rows[0][1], "NodeNotReady");
        assert_eq!(rows[0][2], "Node/node-a");
        assert_eq!(rows[1][1], "Started");
        assert_eq!(rows[1][2], "Pod/web-0");
    }
}
