//! L4 `summary` — a one-shot "is this namespace healthy" rollup: pod counts
//! by health, deployment/statefulset rollout counts, and totals for a couple
//! of other common resources. Meant as the glance before drilling in with
//! `pods`/`describe`/`events`. No Python equivalent — the original tool had
//! nothing like `kubectl get all`.

use anyhow::{Context as _, Result};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod, Service};
use kube::api::{Api, ListParams};

use super::pods::pod_is_unhealthy;
use super::rollout::rollout_complete;
use super::{Command, Output};
use crate::config::ClusterClient;

fn statefulset_ready(sts: &StatefulSet) -> bool {
    let desired = sts.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1);
    sts.status
        .as_ref()
        .map(|s| s.ready_replicas.unwrap_or(0) == desired)
        .unwrap_or(false)
}

fn pvc_is_bound(pvc: &PersistentVolumeClaim) -> bool {
    pvc.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Bound")
}

pub(crate) fn summarize(
    namespace: &str,
    pods: &[Pod],
    deployments: &[Deployment],
    statefulsets: &[StatefulSet],
    services_count: usize,
    pvcs: &[PersistentVolumeClaim],
) -> String {
    let pods_unhealthy = pods.iter().filter(|p| pod_is_unhealthy(p)).count();
    let deployments_rolled_out = deployments.iter().filter(|d| rollout_complete(d)).count();
    let statefulsets_ready = statefulsets.iter().filter(|s| statefulset_ready(s)).count();
    let pvcs_bound = pvcs.iter().filter(|p| pvc_is_bound(p)).count();

    format!(
        "Namespace:    {namespace}\n\
         Pods:         {} total ({pods_unhealthy} unhealthy)\n\
         Deployments:  {} total ({deployments_rolled_out} rolled out)\n\
         StatefulSets: {} total ({statefulsets_ready} ready)\n\
         Services:     {services_count}\n\
         PVCs:         {} total ({pvcs_bound} bound)",
        pods.len(),
        deployments.len(),
        statefulsets.len(),
        pvcs.len(),
    )
}

pub struct NamespaceSummary {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for NamespaceSummary {
    fn name(&self) -> &'static str {
        "summary"
    }
    fn help(&self) -> &'static str {
        "One-shot namespace health rollup: pods/deployments/statefulsets/services/pvcs"
    }
    async fn run(&self, ctx: &ClusterClient, _args: &[String]) -> Result<Output> {
        let ns = &self.namespace;
        let client = ctx.client();

        let pods = Api::<Pod>::namespaced(client.clone(), ns)
            .list(&ListParams::default())
            .await
            .with_context(|| format!("listing pods in {ns}"))?
            .items;
        let deployments = Api::<Deployment>::namespaced(client.clone(), ns)
            .list(&ListParams::default())
            .await
            .with_context(|| format!("listing deployments in {ns}"))?
            .items;
        let statefulsets = Api::<StatefulSet>::namespaced(client.clone(), ns)
            .list(&ListParams::default())
            .await
            .with_context(|| format!("listing statefulsets in {ns}"))?
            .items;
        let services = Api::<Service>::namespaced(client.clone(), ns)
            .list(&ListParams::default())
            .await
            .with_context(|| format!("listing services in {ns}"))?
            .items;
        let pvcs = Api::<PersistentVolumeClaim>::namespaced(client, ns)
            .list(&ListParams::default())
            .await
            .with_context(|| format!("listing pvcs in {ns}"))?
            .items;

        Ok(Output::Text(summarize(
            ns,
            &pods,
            &deployments,
            &statefulsets,
            services.len(),
            &pvcs,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_reports_totals_and_health_counts() {
        let text = summarize("default", &[], &[], &[], 4, &[]);
        assert!(text.contains("Namespace:    default"));
        assert!(text.contains("Pods:         0 total (0 unhealthy)"));
        assert!(text.contains("Deployments:  0 total (0 rolled out)"));
        assert!(text.contains("StatefulSets: 0 total (0 ready)"));
        assert!(text.contains("Services:     4"));
        assert!(text.contains("PVCs:         0 total (0 bound)"));
    }
}
