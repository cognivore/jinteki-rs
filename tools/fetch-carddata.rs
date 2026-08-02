#!/usr/bin/env rust-script
//! fetch-carddata.rs — pinned, verifiable procurement of tools/raw_data.edn.
//!
//! Upstream chain (source of truth):
//!   Null-Signal-Games/netrunner-cards-json  (NSG official card JSON)
//!     │  compiled by NoahTheDuke/netrunner-data (jinteki.net's data repo)
//!     ▼
//!   https://raw.githubusercontent.com/NoahTheDuke/netrunner-data/<commit>/edn/raw_data.edn
//!     │  this script (pin + sha256 verify)
//!     ▼
//!   tools/raw_data.edn          (vendored, byte-identical to upstream)
//!     │  tools/gen-carddata.py
//!     ▼
//!   crates/jinteki-core/carddata/{cards.json,coverage.json}, docs/CARD-COVERAGE.md
//!
//! The pin lives in tools/raw_data.edn.lock (JSON: commit, sha256, fetched_at).
//!
//! Usage (from anywhere inside the repo, `rust-script` on PATH via the devshell):
//!   rust-script tools/fetch-carddata.rs            actualise: move the pin to the
//!                                                  latest upstream commit, fetch,
//!                                                  verify, rewrite the lock, regen
//!   rust-script tools/fetch-carddata.rs verify     check the vendored file against
//!                                                  the lock; touches nothing
//!   rust-script tools/fetch-carddata.rs pinned     re-fetch the PINNED commit,
//!                                                  verify sha256 against the lock,
//!                                                  install, regen
//!
//! ```cargo
//! [dependencies]
//! ureq = { version = "2", features = ["json"] }
//! serde_json = "1"
//! sha2 = "0.10"
//! chrono = "0.4"
//! ```

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const UPSTREAM_REPO: &str = "NoahTheDuke/netrunner-data";
const UPSTREAM_PATH: &str = "edn/raw_data.edn";

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}

fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("cwd");
    loop {
        if dir.join("tools").join("gen-carddata.py").is_file() {
            return dir;
        }
        if !dir.pop() {
            die("not inside the jinteki-rs repo (tools/gen-carddata.py not found in any parent)");
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn http_get(url: &str) -> Vec<u8> {
    let resp = ureq::get(url)
        .set("User-Agent", "jinteki-rs fetch-carddata")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .unwrap_or_else(|e| die(&format!("GET {url}: {e}")));
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .unwrap_or_else(|e| die(&format!("reading {url}: {e}")));
    buf
}

fn latest_commit() -> String {
    let url = format!(
        "https://api.github.com/repos/{UPSTREAM_REPO}/commits?path={UPSTREAM_PATH}&per_page=1"
    );
    let body = http_get(&url);
    let commits: Value = serde_json::from_slice(&body)
        .unwrap_or_else(|e| die(&format!("GitHub API returned non-JSON: {e}")));
    commits[0]["sha"]
        .as_str()
        .unwrap_or_else(|| die(&format!("no upstream commits found for {UPSTREAM_PATH}")))
        .to_string()
}

fn fetch_at(commit: &str) -> Vec<u8> {
    let url =
        format!("https://raw.githubusercontent.com/{UPSTREAM_REPO}/{commit}/{UPSTREAM_PATH}");
    eprintln!("fetching {url} ...");
    http_get(&url)
}

fn read_lock(lock_path: &Path) -> Value {
    let src = std::fs::read_to_string(lock_path)
        .unwrap_or_else(|e| die(&format!("reading {}: {e}", lock_path.display())));
    serde_json::from_str(&src)
        .unwrap_or_else(|e| die(&format!("parsing {}: {e}", lock_path.display())))
}

fn write_lock(lock_path: &Path, commit: &str, sha: &str) {
    let lock = json!({
        "upstream_repo": UPSTREAM_REPO,
        "upstream_path": UPSTREAM_PATH,
        "commit": commit,
        "sha256": sha,
        "fetched_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
    });
    let mut out = serde_json::to_string_pretty(&lock).unwrap();
    out.push('\n');
    std::fs::write(lock_path, out)
        .unwrap_or_else(|e| die(&format!("writing {}: {e}", lock_path.display())));
}

fn install(raw_edn: &Path, blob: &[u8], commit: &str, expect_sha: Option<&str>) -> String {
    let sha = sha256_hex(blob);
    if let Some(expected) = expect_sha {
        if sha != expected {
            die(&format!(
                "sha256 mismatch for {UPSTREAM_PATH} at {commit}\n  \
                 expected {expected}\n  got      {sha}\n\
                 Upstream rewrote history or the download was corrupted; not installing."
            ));
        }
    }
    std::fs::write(raw_edn, blob)
        .unwrap_or_else(|e| die(&format!("writing {}: {e}", raw_edn.display())));
    eprintln!(
        "installed tools/raw_data.edn ({} bytes, sha256 {sha})",
        blob.len()
    );
    sha
}

fn regen(root: &Path) {
    eprintln!("regenerating downstream card data ...");
    let status = Command::new("python3")
        .arg(root.join("tools").join("gen-carddata.py"))
        .current_dir(root)
        .status()
        .unwrap_or_else(|e| die(&format!("spawning gen-carddata.py: {e}")));
    if !status.success() {
        die("gen-carddata.py failed");
    }
}

fn cmd_verify(root: &Path) {
    let lock = read_lock(&root.join("tools").join("raw_data.edn.lock"));
    let raw_edn = root.join("tools").join("raw_data.edn");
    let blob = std::fs::read(&raw_edn)
        .unwrap_or_else(|_| die("tools/raw_data.edn is missing; run fetch-carddata.rs pinned"));
    let actual = sha256_hex(&blob);
    let locked = lock["sha256"].as_str().unwrap_or_else(|| die("lock has no sha256"));
    let commit = lock["commit"].as_str().unwrap_or("?");
    if actual != locked {
        die(&format!(
            "FAIL: tools/raw_data.edn does not match the lock\n  \
             lock   {locked} (commit {commit})\n  actual {actual}\n\
             Run `rust-script tools/fetch-carddata.rs pinned` to restore the pinned file."
        ));
    }
    println!("OK: tools/raw_data.edn matches lock (commit {commit}, sha256 {actual})");
}

fn cmd_pinned(root: &Path) {
    let lock = read_lock(&root.join("tools").join("raw_data.edn.lock"));
    let commit = lock["commit"].as_str().unwrap_or_else(|| die("lock has no commit"));
    let expected = lock["sha256"].as_str().unwrap_or_else(|| die("lock has no sha256"));
    let blob = fetch_at(commit);
    install(&root.join("tools").join("raw_data.edn"), &blob, commit, Some(expected));
    regen(root);
}

fn cmd_update(root: &Path) {
    let lock_path = root.join("tools").join("raw_data.edn.lock");
    let old_commit = lock_path
        .is_file()
        .then(|| read_lock(&lock_path)["commit"].as_str().map(String::from))
        .flatten();
    let commit = latest_commit();
    if old_commit.as_deref() == Some(commit.as_str()) {
        eprintln!("already at latest upstream commit {commit}; re-verifying pinned fetch");
        cmd_pinned(root);
        return;
    }
    let blob = fetch_at(&commit);
    let sha = install(&root.join("tools").join("raw_data.edn"), &blob, &commit, None);
    write_lock(&lock_path, &commit, &sha);
    eprintln!(
        "lock updated: {} -> {commit}",
        old_commit.as_deref().unwrap_or("(none)")
    );
    regen(root);
}

fn main() -> ExitCode {
    let root = repo_root();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => cmd_update(&root),
        ["verify"] => cmd_verify(&root),
        ["pinned"] => cmd_pinned(&root),
        _ => {
            eprintln!("usage: rust-script tools/fetch-carddata.rs [verify|pinned]");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
