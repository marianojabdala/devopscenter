//! `pdb` — PodDisruptionBudgets across all namespaces, with the live
//! `disruptionsAllowed` count. A PDB that's too strict (`disruptionsAllowed`
//! stuck at `0`) is a frequent, non-obvious reason a rollout or `kubectl
//! drain` stalls — nothing in this tool surfaced it before.

use anyhow::{Context as _, Result};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{Api, ListParams};

use super::{name_filter, passes};
use crate::commands::{Command, Output};
use crate::config::ClusterClient;

fn fmt_int_or_string(v: Option<&IntOrString>) -> String {
    match v {
        Some(IntOrString::Int(i)) => i.to_string(),
        Some(IntOrString::String(s)) => s.clone(),
        None => "<none>".into(),
    }
}

pub struct PdbView;

#[async_trait::async_trait]
impl Command for PdbView {
    fn name(&self) -> &'static str {
        "pdb"
    }
    fn help(&self) -> &'static str {
        "PodDisruptionBudgets and their live disruptionsAllowed count"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let filter = name_filter(args);
        let api: Api<PodDisruptionBudget> = Api::all(ctx.client());
        let list = api
            .list(&ListParams::default())
            .await
            .context("listing poddisruptionbudgets")?;

        let mut rows: Vec<[String; 7]> = Vec::new();
        for pdb in list.items {
            let name = pdb.metadata.name.unwrap_or_default();
            if !passes(filter, &name) {
                continue;
            }
            let ns = pdb.metadata.namespace.unwrap_or_default();
            let spec = pdb.spec.unwrap_or_default();
            let status = pdb.status.unwrap_or_default();
            rows.push([
                ctx.context().to_string(),
                ns,
                name,
                fmt_int_or_string(spec.min_available.as_ref()),
                fmt_int_or_string(spec.max_unavailable.as_ref()),
                status.disruptions_allowed.to_string(),
                format!("{}/{}", status.current_healthy, status.desired_healthy),
            ]);
        }
        Ok(Output::table(
            [
                "Cluster",
                "Namespace",
                "Name",
                "Min Available",
                "Max Unavailable",
                "Disruptions Allowed",
                "Healthy",
            ],
            rows,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_int_or_string_handles_both_variants_and_none() {
        assert_eq!(fmt_int_or_string(Some(&IntOrString::Int(2))), "2");
        assert_eq!(
            fmt_int_or_string(Some(&IntOrString::String("50%".into()))),
            "50%"
        );
        assert_eq!(fmt_int_or_string(None), "<none>");
    }
}
