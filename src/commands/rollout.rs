//! L4 `rollout <deployment>` — poll a Deployment until its rollout settles
//! or a bounded timeout elapses (`kubectl rollout status` parity, without a
//! Python equivalent — the original tool had no deployment-level view at
//! all inside a namespace).

use std::time::Duration;

use anyhow::{Context as _, Result};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::Api;

use super::{Command, Output};
use crate::config::ClusterClient;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_POLLS: u32 = 30; // ~60s total

/// A rollout is done once the controller has observed the latest spec
/// (`observedGeneration` caught up) and every desired replica is updated,
/// present, and available.
pub(crate) fn rollout_complete(dep: &Deployment) -> bool {
    let Some(spec) = dep.spec.as_ref() else {
        return false;
    };
    let Some(status) = dep.status.as_ref() else {
        return false;
    };
    let desired = spec.replicas.unwrap_or(1);
    let generation_caught_up =
        status.observed_generation.unwrap_or(0) >= dep.metadata.generation.unwrap_or(0);
    generation_caught_up
        && status.updated_replicas.unwrap_or(0) == desired
        && status.replicas.unwrap_or(0) == desired
        && status.available_replicas.unwrap_or(0) == desired
}

pub(crate) fn status_line(dep: &Deployment) -> String {
    let desired = dep.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
    let status = dep.status.as_ref();
    format!(
        "updated={}/{desired} available={}/{desired} replicas={}/{desired}",
        status.and_then(|s| s.updated_replicas).unwrap_or(0),
        status.and_then(|s| s.available_replicas).unwrap_or(0),
        status.and_then(|s| s.replicas).unwrap_or(0),
    )
}

pub struct RolloutStatus {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for RolloutStatus {
    fn name(&self) -> &'static str {
        "rollout"
    }
    fn help(&self) -> &'static str {
        "Poll a deployment until its rollout settles (or ~60s pass)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: rollout <deployment-name>".into()));
        };
        let api: Api<Deployment> = Api::namespaced(ctx.client(), &self.namespace);
        for attempt in 0..MAX_POLLS {
            let dep = api
                .get(name)
                .await
                .with_context(|| format!("getting deployment {name}"))?;
            if rollout_complete(&dep) {
                return Ok(Output::Text(format!(
                    "deployment {name} rolled out successfully ({})",
                    status_line(&dep)
                )));
            }
            if attempt + 1 == MAX_POLLS {
                return Ok(Output::Text(format!(
                    "timed out after ~60s waiting for deployment {name} to roll out ({})",
                    status_line(&dep)
                )));
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        unreachable!("loop always returns before exhausting MAX_POLLS iterations")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::apps::v1::{DeploymentSpec, DeploymentStatus};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn deployment(
        generation: i64,
        replicas: i32,
        observed_generation: i64,
        updated: i32,
        available: i32,
        current: i32,
    ) -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                generation: Some(generation),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(replicas),
                ..Default::default()
            }),
            status: Some(DeploymentStatus {
                observed_generation: Some(observed_generation),
                updated_replicas: Some(updated),
                available_replicas: Some(available),
                replicas: Some(current),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn complete_when_generation_caught_up_and_all_counts_match() {
        let dep = deployment(2, 3, 2, 3, 3, 3);
        assert!(rollout_complete(&dep));
    }

    #[test]
    fn not_complete_while_generation_is_stale() {
        // Controller hasn't seen the latest spec yet — old counts could
        // still be "correct" by coincidence, so this must not pass.
        let dep = deployment(2, 3, 1, 3, 3, 3);
        assert!(!rollout_complete(&dep));
    }

    #[test]
    fn not_complete_while_replicas_are_still_rolling() {
        let dep = deployment(1, 3, 1, 2, 2, 3);
        assert!(!rollout_complete(&dep));
    }

    #[test]
    fn status_line_reports_actual_over_desired() {
        let dep = deployment(1, 3, 1, 1, 2, 3);
        assert_eq!(status_line(&dep), "updated=1/3 available=2/3 replicas=3/3");
    }
}
