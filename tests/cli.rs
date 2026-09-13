//! Integration coverage for the non-interactive CLI surface (Phase 4).
//! These do not need a cluster — they exercise arg parsing, kubeconfig
//! discovery, and the `--output` renderer.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

const KUBECONFIG: &str = r#"
apiVersion: v1
kind: Config
clusters:
- name: c
  cluster: { server: "https://127.0.0.1:6443", insecure-skip-tls-verify: true }
contexts:
- name: alpha
  context: { cluster: c, user: u }
- name: beta
  context: { cluster: c, user: u }
users:
- name: u
  user: { token: t }
current-context: alpha
"#;

fn kube_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config"), KUBECONFIG).unwrap();
    fs::create_dir_all(dir.path().join("cache")).unwrap();
    fs::write(dir.path().join("cache/skip"), "junk").unwrap();
    dir
}

fn bin() -> Command {
    Command::cargo_bin("devopscenter").unwrap()
}

#[test]
fn help_lists_subcommands() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("contexts"))
        .stdout(predicate::str::contains("pods"))
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn contexts_lists_every_context_from_every_file() {
    let dir = kube_dir();
    bin()
        .args(["--kube-dir", dir.path().to_str().unwrap(), "contexts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("beta"));
}

#[test]
fn contexts_json_output_is_valid_json_array() {
    let dir = kube_dir();
    let out = bin()
        .args([
            "--kube-dir",
            dir.path().to_str().unwrap(),
            "--output",
            "json",
            "contexts",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    let arr = value.as_array().expect("json array");
    let names: Vec<&str> = arr
        .iter()
        .filter_map(|o| o.get("Context").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(names, ["alpha", "beta"]);
}

#[test]
fn empty_kube_dir_exits_nonzero_with_message() {
    let dir = tempfile::tempdir().unwrap();
    bin()
        .args(["--kube-dir", dir.path().to_str().unwrap(), "contexts"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No usable contexts"));
}

#[test]
fn unknown_context_is_reported() {
    let dir = kube_dir();
    bin()
        .args([
            "--kube-dir",
            dir.path().to_str().unwrap(),
            "ns",
            "--context",
            "does-not-exist",
            "list",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
