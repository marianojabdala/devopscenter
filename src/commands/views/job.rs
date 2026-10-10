//! `jobs` — Job status across all namespaces. Batch workload failures (a
//! stuck Job, one that never completes) were previously invisible — nothing
//! in the `views` menu covered this resource type.

use anyhow::{Context as _, Result};
use k8s_openapi::api::batch::v1::Job;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::age;

/// `succeeded/desired`, where `desired` is `spec.completions` (default `1`,
/// same as the API server's own default for a non-parallel Job).
fn completions(job: &Job) -> String {
    let desired = job.spec.as_ref().and_then(|s| s.completions).unwrap_or(1);
    let succeeded = job.status.as_ref().and_then(|s| s.succeeded).unwrap_or(0);
    format!("{succeeded}/{desired}")
}

pub struct JobView;

#[async_trait::async_trait]
impl Command for JobView {
    fn name(&self) -> &'static str {
        "jobs"
    }
    fn help(&self) -> &'static str {
        "Job status across all namespaces (completions/active/failed)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let now = age::now_secs();
        let api: Api<Job> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing jobs")?;

        let mut rows: Vec<[String; 7]> = Vec::new();
        for job in list.items {
            let name = job.metadata.name.clone().unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = job.metadata.namespace.clone().unwrap_or_default();
            let active = job
                .status
                .as_ref()
                .and_then(|s| s.active)
                .unwrap_or(0)
                .to_string();
            let failed = job
                .status
                .as_ref()
                .and_then(|s| s.failed)
                .unwrap_or(0)
                .to_string();
            let created_age = job
                .metadata
                .creation_timestamp
                .as_ref()
                .map(|t| age::format_secs(age::secs_since(now, t)))
                .unwrap_or_else(|| "<unknown>".into());
            rows.push([
                ctx.context().to_string(),
                ns,
                name,
                completions(&job),
                active,
                failed,
                created_age,
            ]);
        }
        Ok(Output::table(
            [
                "Cluster",
                "Namespace",
                "Name",
                "Completions",
                "Active",
                "Failed",
                "Age",
            ],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::batch::v1::{JobSpec, JobStatus};

    fn job(completions_desired: Option<i32>, succeeded: Option<i32>) -> Job {
        Job {
            metadata: Default::default(),
            spec: Some(JobSpec {
                completions: completions_desired,
                ..Default::default()
            }),
            status: Some(JobStatus {
                succeeded,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn completions_defaults_desired_to_one() {
        assert_eq!(completions(&job(None, Some(1))), "1/1");
    }

    #[test]
    fn completions_uses_explicit_desired_count() {
        assert_eq!(completions(&job(Some(5), Some(3))), "3/5");
    }

    #[test]
    fn completions_defaults_succeeded_to_zero() {
        assert_eq!(completions(&job(Some(3), None)), "0/3");
    }
}
