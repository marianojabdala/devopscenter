//! L4 `secret <name>` — a Secret's keys, masked. Shows every key and its
//! decoded byte length, **never** the decoded value — unlike `configmap`,
//! which shows real content, since a Secret's whole purpose is to hold
//! values that shouldn't land on a screen or in a saved `--output json`
//! file.

use anyhow::Result;
use k8s_openapi::api::core::v1::Secret;
use kube::api::Api;

use super::{Command, Output};
use crate::config::ClusterClient;

/// One row per key: `<N bytes>`, never the decoded value. A leading
/// `<type>` row surfaces the Secret's type (`Opaque`, `kubernetes.io/tls`,
/// ...) — not sensitive, and useful context for what the keys mean.
/// `[("<none>", "")]` after that if the Secret has no data.
fn masked_rows(secret: &Secret) -> Vec<[String; 2]> {
    let mut rows = vec![[
        "<type>".to_string(),
        secret.type_.clone().unwrap_or_else(|| "Opaque".to_string()),
    ]];
    let mut data_rows: Vec<[String; 2]> = secret
        .data
        .iter()
        .flatten()
        .map(|(k, v)| [k.clone(), format!("<{} bytes>", v.0.len())])
        .collect();
    data_rows.sort();
    if data_rows.is_empty() {
        data_rows.push(["<none>".into(), String::new()]);
    }
    rows.extend(data_rows);
    rows
}

pub struct SecretShow {
    pub namespace: String,
}

#[async_trait::async_trait]
impl Command for SecretShow {
    fn name(&self) -> &'static str {
        "secret"
    }
    fn help(&self) -> &'static str {
        "Shows a Secret's keys and byte lengths — never the decoded values"
    }
    async fn run(&self, ctx: &ClusterClient, args: &[String]) -> Result<Output> {
        let Some(name) = args.get(1) else {
            return Ok(Output::Text("usage: secret <name>".into()));
        };
        let api: Api<Secret> = Api::namespaced(ctx.client(), &self.namespace);
        let Some(secret) = api.get_opt(name).await? else {
            return Ok(Output::Text(format!(
                "Secret {name:?} not found in {}",
                self.namespace
            )));
        };
        Ok(Output::table(["Key", "Value"], masked_rows(&secret)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::ByteString;
    use std::collections::BTreeMap;

    fn secret(type_: Option<&str>, data: Option<BTreeMap<String, ByteString>>) -> Secret {
        Secret {
            metadata: Default::default(),
            data,
            immutable: None,
            string_data: None,
            type_: type_.map(String::from),
        }
    }

    #[test]
    fn masked_rows_never_includes_the_decoded_value() {
        let s = secret(
            Some("Opaque"),
            Some(BTreeMap::from([(
                "password".to_string(),
                ByteString(b"super-secret".to_vec()),
            )])),
        );
        let rows = masked_rows(&s);
        assert_eq!(rows[0], ["<type>".to_string(), "Opaque".to_string()]);
        assert_eq!(rows[1], ["password".to_string(), "<12 bytes>".to_string()]);
        for row in &rows {
            assert!(!row[1].contains("super-secret"));
        }
    }

    #[test]
    fn masked_rows_defaults_type_to_opaque_and_reports_none_for_no_data() {
        let s = secret(None, None);
        let rows = masked_rows(&s);
        assert_eq!(rows[0], ["<type>".to_string(), "Opaque".to_string()]);
        assert_eq!(rows[1], ["<none>".to_string(), String::new()]);
    }
}
