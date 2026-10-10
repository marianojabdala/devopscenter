//! `cronjobs` — schedule, suspend state, last-run time, and currently-active
//! Job count across all namespaces. Answers "did my cron actually fire" at a
//! glance, which previously meant cross-referencing `jobs` by hand.

use anyhow::{Context as _, Result};
use k8s_openapi::api::batch::v1::CronJob;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;
use crate::domain::age;

/// Time since `status.lastScheduleTime`, or `<never>` if the CronJob has
/// never fired (e.g. just created, or permanently suspended).
fn last_schedule(cron: &CronJob, now: i64) -> String {
    cron.status
        .as_ref()
        .and_then(|s| s.last_schedule_time.as_ref())
        .map(|t| age::format_secs(age::secs_since(now, t)))
        .unwrap_or_else(|| "<never>".into())
}

pub struct CronJobView;

#[async_trait::async_trait]
impl Command for CronJobView {
    fn name(&self) -> &'static str {
        "cronjobs"
    }
    fn help(&self) -> &'static str {
        "CronJob schedule/suspend/last-run across all namespaces"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let now = age::now_secs();
        let api: Api<CronJob> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing cronjobs")?;

        let mut rows: Vec<[String; 7]> = Vec::new();
        for cron in list.items {
            let name = cron.metadata.name.clone().unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = cron.metadata.namespace.clone().unwrap_or_default();
            let schedule = cron
                .spec
                .as_ref()
                .map(|s| s.schedule.clone())
                .unwrap_or_default();
            let suspended = cron
                .spec
                .as_ref()
                .and_then(|s| s.suspend)
                .unwrap_or(false)
                .to_string();
            let active = cron
                .status
                .as_ref()
                .and_then(|s| s.active.as_ref())
                .map(|a| a.len())
                .unwrap_or(0)
                .to_string();
            let scheduled = last_schedule(&cron, now);
            rows.push([
                ctx.context().to_string(),
                ns,
                name,
                schedule,
                suspended,
                scheduled,
                active,
            ]);
        }
        Ok(Output::table(
            [
                "Cluster",
                "Namespace",
                "Name",
                "Schedule",
                "Suspended",
                "Last Schedule",
                "Active",
            ],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::batch::v1::{CronJobSpec, CronJobStatus, JobTemplateSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use k8s_openapi::jiff::ToSpan;

    fn cronjob(last_schedule_secs_ago: Option<i64>) -> CronJob {
        CronJob {
            metadata: Default::default(),
            spec: Some(CronJobSpec {
                schedule: "* * * * *".into(),
                job_template: JobTemplateSpec::default(),
                ..Default::default()
            }),
            status: Some(CronJobStatus {
                last_schedule_time: last_schedule_secs_ago
                    .map(|secs| Time(k8s_openapi::jiff::Timestamp::now() - secs.seconds())),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn last_schedule_reports_never_when_it_has_not_fired() {
        let now = age::now_secs();
        assert_eq!(last_schedule(&cronjob(None), now), "<never>");
    }

    #[test]
    fn last_schedule_formats_elapsed_time() {
        let now = age::now_secs();
        assert_eq!(last_schedule(&cronjob(Some(90)), now), "1m");
    }
}
