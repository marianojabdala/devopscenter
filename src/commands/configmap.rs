//! L4 `configmap <name>` — a ConfigMap's full key/value content.
//! ConfigMaps hold non-sensitive configuration, so (unlike `secret`) the
//! actual values are shown — complements `configcheck`, which only checks
//! existence and never shows content.

use anyhow::Result;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::Api;

use super::{Command, Output};
use crate::config::ClusterClient;

/// One row per key: `data` entries as-is, `binaryData` entries as
/// `<binary, N bytes>` (decoding arbitrary bytes as text isn't safe to
/// print). `[("<none>", "")]` if the ConfigMap has neither.
fn content_rows(cm: &ConfigMap) -> Vec<[String; 2]> {
    let mut rows: Vec<[String; 2]> = cm
        .data
        .iter()
        .flatten()
        .map(|(k, v)| [k.clone(), v.clone()])
        .collect();
    rows.extend(
        cm.binary_data
            .iter()
            .flatten()
            .map(|(k, v)| [k.clone(), format!("<binary, {} bytes>", v.0.len())]),
    );
    if rows.is_empty() {
        rows.push(["<none>".into(), String::new()]);
    }
    rows.sort();
    rows
}

pub struct ConfigMapShow {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for ConfigMapShow {
    fn name(&self) -> &'static str {
        "configmap"
    }
    fn help(&self) -> &'static str {
        "Shows a ConfigMap's full key/value content"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: configmap <name>".into()));
        };
        let api: Api<ConfigMap> = Api::namespaced(ctx.client(), &self.namespace);
        let Some(cm) = api.get_opt(name).await? else {
            return Ok(Output::Text(format!(
                "ConfigMap {name:?} not found in {}",
                self.namespace
            )));
        };
        Ok(Output::table(["Key", "Value"], content_rows(&cm)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::ByteString;
    use std::collections::BTreeMap;

    fn configmap(
        data: Option<BTreeMap<String, String>>,
        binary: Option<BTreeMap<String, ByteString>>,
    ) -> ConfigMap {
        ConfigMap {
            metadata: Default::default(),
            data,
            binary_data: binary,
            immutable: None,
        }
    }

    #[test]
    fn content_rows_includes_data_entries_as_is() {
        let cm = configmap(
            Some(BTreeMap::from([("key".to_string(), "value".to_string())])),
            None,
        );
        assert_eq!(
            content_rows(&cm),
            vec![["key".to_string(), "value".to_string()]]
        );
    }

    #[test]
    fn content_rows_shows_binary_entries_as_byte_count_not_raw_bytes() {
        let cm = configmap(
            None,
            Some(BTreeMap::from([(
                "blob".to_string(),
                ByteString(vec![1, 2, 3, 4]),
            )])),
        );
        assert_eq!(
            content_rows(&cm),
            vec![["blob".to_string(), "<binary, 4 bytes>".to_string()]]
        );
    }

    #[test]
    fn content_rows_reports_none_when_empty() {
        let cm = configmap(None, None);
        assert_eq!(
            content_rows(&cm),
            vec![["<none>".to_string(), String::new()]]
        );
    }
}
