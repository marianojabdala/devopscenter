//! Cluster discovery and connection registry.
//!
//! Fixes migration.md **O1** and **O2/O3** structurally:
//!   * every context in every kubeconfig is enumerated (not just each file's
//!     active context), and each gets its **own** [`kube::Client`];
//!   * discovery runs exactly once, at startup, and the registry is passed by
//!     reference into commands — constructors never touch the filesystem.

mod registry;

pub use registry::{ClusterClient, ClusterRegistry};

use std::path::PathBuf;

/// `~/.kube` — the conventional kubeconfig location.
pub fn default_kube_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".kube"))
        .unwrap_or_else(|| PathBuf::from(".kube"))
}

/// Per-user state directory (`~/.local/share/devopscenter` on Linux) — matches
/// the Python `Base.base_path`. Holds the REPL history file. Created on demand.
pub fn default_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join("devopscenter"))
        .unwrap_or_else(|| PathBuf::from(".devopscenter"))
}
