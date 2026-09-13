//! Read-only cluster reports (`docs/behaviour-catalogue.md` L3-views).
//!
//! Every view takes an optional `args[1]` substring filter on the resource
//! name. `ingress` uses `networking.k8s.io/v1` (migration.md O5 fix); `usage`
//! parses quantities via [`crate::domain::quantity`] (O4 fix).

mod deploy;
mod hpa;
mod ingress;
mod pod_resources;
mod pvc;
mod statefulset;
mod usage;

pub use deploy::DeployView;
pub use hpa::HpaView;
pub use ingress::IngressView;
pub use pod_resources::PodResourcesView;
pub use pvc::PvcView;
pub use statefulset::StatefulsetView;
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
