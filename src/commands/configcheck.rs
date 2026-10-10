//! L4 `configcheck <pod_index>` — every ConfigMap/Secret a pod references
//! (env, envFrom, volumes, imagePullSecrets), and whether it currently
//! exists. Never shows values, only presence. Catches the "CrashLoopBackOff
//! because someone deleted a ConfigMap" bug, which `describe`'s events
//! section only hints at indirectly, if at all.

use std::collections::BTreeSet;

use anyhow::Result;
use k8s_openapi::api::core::v1::{ConfigMap, Pod, Secret};
use kube::api::Api;

use super::pods::{list_pods, resolve};
use super::{Command, Output};
use crate::config::ClusterClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    ConfigMap,
    Secret,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::ConfigMap => "ConfigMap",
            Kind::Secret => "Secret",
        }
    }
}

/// Every `(kind, name)` referenced anywhere in the pod spec: container
/// `env`/`envFrom` (including init containers), volumes, and
/// `imagePullSecrets`. A `BTreeSet` so the same ConfigMap/Secret referenced
/// several ways (e.g. one key via `env`, the whole map via `envFrom`)
/// produces one row, not several.
fn references(pod: &Pod) -> BTreeSet<(Kind, String)> {
    let mut refs = BTreeSet::new();
    let Some(spec) = pod.spec.as_ref() else {
        return refs;
    };

    for container in spec
        .containers
        .iter()
        .chain(spec.init_containers.iter().flatten())
    {
        for env_from in container.env_from.iter().flatten() {
            if let Some(cm) = &env_from.config_map_ref {
                refs.insert((Kind::ConfigMap, cm.name.clone()));
            }
            if let Some(secret) = &env_from.secret_ref {
                refs.insert((Kind::Secret, secret.name.clone()));
            }
        }
        for env in container.env.iter().flatten() {
            let Some(value_from) = &env.value_from else {
                continue;
            };
            if let Some(cm) = &value_from.config_map_key_ref {
                refs.insert((Kind::ConfigMap, cm.name.clone()));
            }
            if let Some(secret) = &value_from.secret_key_ref {
                refs.insert((Kind::Secret, secret.name.clone()));
            }
        }
    }
    for volume in spec.volumes.iter().flatten() {
        if let Some(cm) = &volume.config_map {
            refs.insert((Kind::ConfigMap, cm.name.clone()));
        }
        if let Some(secret) = &volume.secret {
            if let Some(name) = &secret.secret_name {
                refs.insert((Kind::Secret, name.clone()));
            }
        }
    }
    for pull_secret in spec.image_pull_secrets.iter().flatten() {
        if !pull_secret.name.is_empty() {
            refs.insert((Kind::Secret, pull_secret.name.clone()));
        }
    }
    refs
}

pub struct ConfigCheck {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for ConfigCheck {
    fn name(&self) -> &'static str {
        "configcheck"
    }
    fn help(&self) -> &'static str {
        "Checks that every ConfigMap/Secret a pod references actually exists (no values shown)"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(selector) = args.get(1) else {
            return Ok(Output::Text("usage: configcheck <pod_index>".into()));
        };
        let pods = list_pods(ctx, &self.namespace).await?;
        let (pod, _) = resolve(&pods, selector)?;
        let refs = references(pod);
        if refs.is_empty() {
            return Ok(Output::Text(
                "This pod references no ConfigMaps or Secrets".into(),
            ));
        }

        let cm_api: Api<ConfigMap> = Api::namespaced(ctx.client(), &self.namespace);
        let secret_api: Api<Secret> = Api::namespaced(ctx.client(), &self.namespace);

        let mut rows: Vec<[String; 3]> = Vec::new();
        for (kind, name) in refs {
            let exists = match kind {
                Kind::ConfigMap => cm_api.get_opt(&name).await?.is_some(),
                Kind::Secret => secret_api.get_opt(&name).await?.is_some(),
            };
            rows.push([kind.label().into(), name, exists.to_string()]);
        }
        Ok(Output::table(["Kind", "Name", "Exists"], rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{
        ConfigMapEnvSource, ConfigMapVolumeSource, Container, EnvFromSource, EnvVar, EnvVarSource,
        LocalObjectReference, PodSpec, SecretEnvSource, SecretKeySelector, SecretVolumeSource,
        Volume,
    };

    fn pod_with_spec(spec: PodSpec) -> Pod {
        Pod {
            metadata: Default::default(),
            spec: Some(spec),
            status: None,
        }
    }

    #[test]
    fn references_is_empty_for_a_pod_with_no_config_or_secrets() {
        let pod = pod_with_spec(PodSpec {
            containers: vec![Container {
                name: "app".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(references(&pod).is_empty());
    }

    #[test]
    fn references_collects_env_from_and_volume_and_pull_secrets() {
        let pod = pod_with_spec(PodSpec {
            containers: vec![Container {
                name: "app".into(),
                env_from: Some(vec![EnvFromSource {
                    config_map_ref: Some(ConfigMapEnvSource {
                        name: "app-config".into(),
                        ..Default::default()
                    }),
                    secret_ref: Some(SecretEnvSource {
                        name: "app-secret".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                env: Some(vec![EnvVar {
                    name: "DB_PASSWORD".into(),
                    value_from: Some(EnvVarSource {
                        secret_key_ref: Some(SecretKeySelector {
                            key: "password".into(),
                            name: "db-secret".into(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }]),
                ..Default::default()
            }],
            volumes: Some(vec![Volume {
                name: "cfg".into(),
                config_map: Some(ConfigMapVolumeSource {
                    name: "vol-config".into(),
                    ..Default::default()
                }),
                secret: Some(SecretVolumeSource {
                    secret_name: Some("vol-secret".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            image_pull_secrets: Some(vec![LocalObjectReference {
                name: "pull-secret".into(),
            }]),
            ..Default::default()
        });

        let refs = references(&pod);
        assert!(refs.contains(&(Kind::ConfigMap, "app-config".to_string())));
        assert!(refs.contains(&(Kind::Secret, "app-secret".to_string())));
        assert!(refs.contains(&(Kind::ConfigMap, "vol-config".to_string())));
        assert!(refs.contains(&(Kind::Secret, "vol-secret".to_string())));
        assert!(refs.contains(&(Kind::Secret, "pull-secret".to_string())));
        assert!(refs.contains(&(Kind::Secret, "db-secret".to_string())));
    }
}
