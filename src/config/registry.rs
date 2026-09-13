use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Client, Config};
use tracing::{debug, warn};

/// A single named context bound to its own client.
#[derive(Clone)]
pub struct ClusterClient {
    context: String,
    source: PathBuf,
    client: Client,
}

impl ClusterClient {
    /// Context name as it appears in the kubeconfig.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Kubeconfig file this context came from.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// The per-context Kubernetes client. Each `ClusterClient` owns a distinct
    /// one — there is no shared/global configuration (migration.md O1).
    pub fn client(&self) -> Client {
        self.client.clone()
    }
}

/// All contexts discovered at startup, keyed by context name (sorted).
pub struct ClusterRegistry {
    clients: BTreeMap<String, ClusterClient>,
}

impl ClusterRegistry {
    /// Recursively scan `kube_dir` for kubeconfig files and build one
    /// [`ClusterClient`] per context found. Directories whose name contains
    /// `cache` are skipped. Unparseable files and contexts that fail to
    /// initialise are logged and skipped, never fatal.
    pub async fn discover(kube_dir: &Path) -> Result<Self> {
        let mut files = Vec::new();
        collect_files(kube_dir, &mut files);
        debug!(count = files.len(), dir = %kube_dir.display(), "kubeconfig candidates");

        let mut clients = BTreeMap::new();
        for file in files {
            let raw = match Kubeconfig::read_from(&file) {
                Ok(cfg) => cfg,
                Err(err) => {
                    debug!(path = %file.display(), %err, "not a kubeconfig, skipping");
                    continue;
                }
            };
            if raw.contexts.is_empty() {
                continue;
            }
            for named in &raw.contexts {
                let name = named.name.clone();
                if clients.contains_key(&name) {
                    warn!(context = %name, path = %file.display(),
                        "duplicate context name, keeping the first one seen");
                    continue;
                }
                match build_client(&raw, &name).await {
                    Ok(client) => {
                        clients.insert(
                            name.clone(),
                            ClusterClient {
                                context: name,
                                source: file.clone(),
                                client,
                            },
                        );
                    }
                    Err(err) => warn!(context = %name, path = %file.display(), %err,
                        "could not initialise context, skipping"),
                }
            }
        }

        Ok(Self { clients })
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    /// Context names, sorted — used for the picker and its completion.
    pub fn context_names(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    pub fn get(&self, context: &str) -> Option<&ClusterClient> {
        self.clients.get(context)
    }
}

async fn build_client(kubeconfig: &Kubeconfig, context: &str) -> Result<Client> {
    let options = KubeConfigOptions {
        context: Some(context.to_string()),
        cluster: None,
        user: None,
    };
    let config = Config::from_custom_kubeconfig(kubeconfig.clone(), &options)
        .await
        .with_context(|| format!("building config for context {context}"))?;
    Client::try_from(config).with_context(|| format!("building client for context {context}"))
}

/// Depth-first file collection, skipping any directory whose file name contains
/// `cache` (mirrors the Python walk, without its overwrite bug — O3).
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains("cache"))
                .unwrap_or(false);
            if !skip {
                collect_files(&path, out);
            }
        } else if ft.is_file() {
            out.push(path);
        }
    }
}
