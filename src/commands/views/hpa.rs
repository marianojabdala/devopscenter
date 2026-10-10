//! `hpa` — HorizontalPodAutoscalers across all namespaces, including live
//! current-vs-target metrics (not just replica counts). Read via
//! `autoscaling/v2` rather than `v1` so multi-metric HPAs (CPU **and**
//! memory, the common case) show every metric, not just CPU.

use anyhow::{Context as _, Result};
use k8s_openapi::api::autoscaling::v2::{HorizontalPodAutoscaler, MetricSpec, MetricStatus};
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

/// `<value>%` for a utilization target/current, or the raw `Quantity` text
/// for an absolute average-value target — whichever the metric actually set.
fn metric_target_value(target: &k8s_openapi::api::autoscaling::v2::MetricTarget) -> String {
    target
        .average_utilization
        .map(|u| format!("{u}%"))
        .or_else(|| target.average_value.as_ref().map(|q| q.0.clone()))
        .or_else(|| target.value.as_ref().map(|q| q.0.clone()))
        .unwrap_or_else(|| "?".into())
}

fn metric_current_value(current: &k8s_openapi::api::autoscaling::v2::MetricValueStatus) -> String {
    current
        .average_utilization
        .map(|u| format!("{u}%"))
        .or_else(|| current.average_value.as_ref().map(|q| q.0.clone()))
        .or_else(|| current.value.as_ref().map(|q| q.0.clone()))
        .unwrap_or_else(|| "?".into())
}

/// The `(type, resource-name)` identity used to pair a `spec.metrics` entry
/// with its `status.currentMetrics` counterpart — Kubernetes doesn't
/// guarantee the two arrays are in the same order, only that each entry
/// carries enough identity (type + resource name, for `Resource` metrics)
/// to be matched up.
fn metric_identity(type_: &str, resource_name: Option<&str>) -> (String, Option<String>) {
    (type_.to_string(), resource_name.map(String::from))
}

/// One `name: current/target` summary per metric, comma-joined. Only
/// `Resource` metrics (CPU/memory utilization — by far the common case) are
/// resolved to an actual value; any other metric type (`Pods`, `Object`,
/// `External`, `ContainerResource`) is shown by its type name only, since
/// those don't reduce to one simple scalar the same way.
pub(crate) fn metrics_summary(specs: &[MetricSpec], statuses: &[MetricStatus]) -> String {
    if specs.is_empty() {
        return "<none>".into();
    }
    specs
        .iter()
        .map(|spec| match &spec.resource {
            Some(res) => {
                let identity = metric_identity("Resource", Some(&res.name));
                let current = statuses
                    .iter()
                    .find(|s| {
                        s.resource
                            .as_ref()
                            .map(|r| metric_identity("Resource", Some(&r.name)))
                            .as_ref()
                            == Some(&identity)
                    })
                    .and_then(|s| s.resource.as_ref())
                    .map(|r| metric_current_value(&r.current))
                    .unwrap_or_else(|| "?".into());
                format!(
                    "{}: {current}/{}",
                    res.name,
                    metric_target_value(&res.target)
                )
            }
            None => spec.type_.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub struct HpaView;

#[async_trait::async_trait]
impl Command for HpaView {
    fn name(&self) -> &'static str {
        "hpa"
    }
    fn help(&self) -> &'static str {
        "Horizontal Pod Autoscalers across all namespaces, with live current/target metrics"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<HorizontalPodAutoscaler> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing horizontalpodautoscalers")?;

        let mut rows: Vec<[String; 7]> = Vec::new();
        for h in list.items {
            let name = h.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = h.metadata.namespace.unwrap_or_default();
            let spec = h.spec.unwrap_or_default();
            let status = h.status.unwrap_or_default();
            let min = spec.min_replicas.map(|v| v.to_string()).unwrap_or_default();
            let max = spec.max_replicas.to_string();
            let current_replicas = status.current_replicas.unwrap_or(0).to_string();
            let desired_replicas = status.desired_replicas.to_string();
            let metrics = metrics_summary(
                spec.metrics.as_deref().unwrap_or_default(),
                status.current_metrics.as_deref().unwrap_or_default(),
            );
            let idx = rows.len();
            rows.push([
                ctx.context().to_string(),
                ns,
                format!("{name} ({idx})"),
                min,
                max,
                format!("{current_replicas}/{desired_replicas}"),
                metrics,
            ]);
        }
        Ok(Output::table(
            [
                "Cluster",
                "Namespace",
                "Hpa name",
                "Min",
                "Max",
                "Current/Desired",
                "Metrics",
            ],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::autoscaling::v2::{
        MetricTarget, MetricValueStatus, ResourceMetricSource, ResourceMetricStatus,
    };
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    fn resource_spec(name: &str, target_utilization: i32) -> MetricSpec {
        MetricSpec {
            type_: "Resource".into(),
            resource: Some(ResourceMetricSource {
                name: name.into(),
                target: MetricTarget {
                    type_: "Utilization".into(),
                    average_utilization: Some(target_utilization),
                    ..Default::default()
                },
            }),
            ..Default::default()
        }
    }

    fn resource_status_utilization(name: &str, current_utilization: i32) -> MetricStatus {
        MetricStatus {
            type_: "Resource".into(),
            resource: Some(ResourceMetricStatus {
                name: name.into(),
                current: MetricValueStatus {
                    average_utilization: Some(current_utilization),
                    ..Default::default()
                },
            }),
            ..Default::default()
        }
    }

    #[test]
    fn metrics_summary_pairs_spec_and_status_by_resource_name() {
        let specs = vec![resource_spec("cpu", 80), resource_spec("memory", 75)];
        let statuses = vec![
            resource_status_utilization("memory", 60),
            resource_status_utilization("cpu", 45),
        ];
        assert_eq!(
            metrics_summary(&specs, &statuses),
            "cpu: 45%/80%, memory: 60%/75%"
        );
    }

    #[test]
    fn metrics_summary_reports_unknown_when_status_has_no_matching_entry() {
        let specs = vec![resource_spec("cpu", 80)];
        assert_eq!(metrics_summary(&specs, &[]), "cpu: ?/80%");
    }

    #[test]
    fn metrics_summary_reports_none_for_an_hpa_with_no_metrics() {
        assert_eq!(metrics_summary(&[], &[]), "<none>");
    }

    #[test]
    fn metrics_summary_falls_back_to_type_name_for_non_resource_metrics() {
        let specs = vec![MetricSpec {
            type_: "Object".into(),
            ..Default::default()
        }];
        assert_eq!(metrics_summary(&specs, &[]), "Object");
    }

    #[test]
    fn metric_target_value_prefers_utilization_then_average_value_then_value() {
        assert_eq!(
            metric_target_value(&MetricTarget {
                type_: "Utilization".into(),
                average_utilization: Some(50),
                ..Default::default()
            }),
            "50%"
        );
        assert_eq!(
            metric_target_value(&MetricTarget {
                type_: "AverageValue".into(),
                average_value: Some(Quantity("100Mi".into())),
                ..Default::default()
            }),
            "100Mi"
        );
    }
}
