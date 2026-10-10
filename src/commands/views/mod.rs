//! Read-only cluster reports.
//!
//! Every view takes an optional `args[1]` substring filter on the resource
//! name. `ingress` uses `networking.k8s.io/v1` (migration.md O5 fix); `usage`
//! parses quantities via [`crate::domain::quantity`] (O4 fix).

mod cronjob;
mod daemonset;
mod deploy;
mod flagger;
mod hpa;
mod ingress;
mod istio;
mod job;
mod node_describe;
mod nodes;
mod pdb;
mod pod_resources;
mod pvc;
mod statefulset;
mod taints;
mod usage;

pub use cronjob::CronJobView;
pub use daemonset::DaemonSetView;
pub use deploy::DeployView;
pub use flagger::CanaryView;
pub use hpa::HpaView;
pub use ingress::IngressView;
pub use istio::{
    IstioAuthzView, IstioDestinationRulesView, IstioGatewaysView, IstioPeerAuthView,
    IstioVirtualServicesView,
};
pub use job::JobView;
pub use node_describe::NodeDescribe;
pub use nodes::NodesView;
pub use pdb::PdbView;
pub use pod_resources::PodResourcesView;
pub use pvc::PvcView;
pub use statefulset::StatefulsetView;
pub use taints::TaintsView;
pub use usage::UsageView;

/// The `args[1]` name filter, if present.
pub(crate) fn name_filter(args: &[String]) -> Option<&str> {
    args.get(1).map(String::as_str)
}

pub(crate) fn passes(filter: Option<&str>, name: &str) -> bool {
    match filter {
        Some(f) => name.contains(f),
        None => true,
    }
}
