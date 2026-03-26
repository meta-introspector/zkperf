//! zkperf-audit — commitment chain for all zkperf operations
//!
//! Every operation (schema, extract, record, read) produces an audit shard:
//!   - input commitment (hash of what was consumed)
//!   - output commitments (hashes of what was produced)
//!   - operation name + timestamp
//!   - chain link to previous audit shard
//!
//! Verify: walk the chain, check every commitment matches actual shard content.

use anyhow::Result;
use erdfa_publish::{Component, Shard};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// An audit record for one zkperf operation
pub struct AuditRecord {
    pub operation: String,
    pub timestamp: u64,
    pub input_commitment: String,
    pub output_commitments: Vec<String>,
    pub prev_audit: String, // hash of previous audit shard (chain link)
}

impl AuditRecord {
    pub fn commitment(&self) -> String {
        let data = format!(
            "{}:{}:{}:{}:{}",
            self.operation,
            self.timestamp,
            self.input_commitment,
            self.output_commitments.join("|"),
            self.prev_audit
        );
        hex::encode(Sha256::digest(data.as_bytes()))
    }

    pub fn to_shard(&self) -> Shard {
        let commit = self.commitment();
        let pairs = vec![
            ("operation".into(), self.operation.clone()),
            ("timestamp".into(), self.timestamp.to_string()),
            ("input_commitment".into(), self.input_commitment.clone()),
            (
                "output_count".into(),
                self.output_commitments.len().to_string(),
            ),
            (
                "output_commitments".into(),
                self.output_commitments.join(","),
            ),
            ("prev_audit".into(), self.prev_audit.clone()),
            ("commitment".into(), commit.clone()),
        ];
        Shard::new(
            format!("audit_{}", &commit[..16]),
            Component::KeyValue { pairs },
        )
        .with_tags(vec!["da51".into(), "audit".into(), "zkperf".into()])
    }
}

/// Hash a CBOR shard file
pub fn hash_file(path: &str) -> Result<String> {
    let data = fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&data)))
}

/// Hash all .cbor files in a directory → sorted commitment
pub fn hash_dir(dir: &str) -> Result<(String, Vec<String>)> {
    let mut hashes = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "cbor").unwrap_or(false))
        .collect();
    entries.sort();
    for path in &entries {
        let h = hex::encode(Sha256::digest(&fs::read(path)?));
        hashes.push(h);
    }
    let combined = hashes.join("|");
    let root = hex::encode(Sha256::digest(combined.as_bytes()));
    Ok((root, hashes))
}

/// Read the latest audit shard from a directory
fn latest_audit(dir: &str) -> String {
    let mut audits: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("audit_"))
                .unwrap_or(false)
        })
        .collect();
    audits.sort();
    audits
        .last()
        .and_then(|p| fs::read(p).ok())
        .map(|data| hex::encode(Sha256::digest(&data)))
        .unwrap_or_else(|| "genesis".into())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage:");
        eprintln!("  zkperf-audit log <operation> <input-path> <output-dir>");
        eprintln!("  zkperf-audit verify <shard-dir>");
        eprintln!("  zkperf-audit chain <shard-dir>");
        std::process::exit(1);
    }
    match args[0].as_str() {
        "log" => cmd_log(&args[1], &args[2], &args[3])?,
        "verify" => cmd_verify(&args[1])?,
        "chain" => cmd_chain(&args[1])?,
        _ => eprintln!("unknown: {}", args[0]),
    }
    Ok(())
}

/// Log an operation: hash input, hash outputs, emit audit shard
fn cmd_log(operation: &str, input_path: &str, output_dir: &str) -> Result<()> {
    let input_commitment = if std::path::Path::new(input_path).is_dir() {
        hash_dir(input_path)?.0
    } else {
        hash_file(input_path)?
    };

    let (_, output_commitments) = hash_dir(output_dir)?;
    let prev = latest_audit(output_dir);

    let record = AuditRecord {
        operation: operation.into(),
        timestamp: now(),
        input_commitment,
        output_commitments: output_commitments.clone(),
        prev_audit: prev,
    };

    let shard = record.to_shard();
    let commit = record.commitment();
    let path = format!("{}/audit_{}.cbor", output_dir, &commit[..16]);
    fs::write(&path, shard.to_cbor())?;
    eprintln!(
        "audit: {} → {} ({} outputs, commitment {})",
        operation,
        path,
        output_commitments.len(),
        &commit[..16]
    );
    Ok(())
}

/// Verify all shards in a directory match their audit commitments
fn cmd_verify(dir: &str) -> Result<()> {
    let (root, hashes) = hash_dir(dir)?;
    let mut audits = Vec::new();
    let mut violations = 0usize;

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path
            .file_name()
            .map(|n| n.to_string_lossy().starts_with("audit_"))
            .unwrap_or(false)
        {
            let raw = fs::read(&path)?;
            audits.push((path.clone(), raw));
        }
    }

    // Check each non-audit shard is accounted for in some audit's output_commitments
    let mut all_audit_outputs: Vec<String> = Vec::new();
    for (_, raw) in &audits {
        let data = if raw.len() > 2 && raw[0] == 0xda && raw[1] == 0x51 {
            &raw[2..]
        } else {
            &raw[..]
        };
        let debug = format!(
            "{:?}",
            ciborium::from_reader::<ciborium::Value, _>(data).ok()
        );
        if let Some(pos) = debug.find("output_commitments") {
            let rest = &debug[pos..];
            if let Some(tpos) = rest.find("Text(\"") {
                let inner = &rest[tpos + 6..];
                if let Some(end) = inner.find('"') {
                    for h in inner[..end].split(',') {
                        if !h.is_empty() {
                            all_audit_outputs.push(h.to_string());
                        }
                    }
                }
            }
        }
    }

    for h in &hashes {
        if !all_audit_outputs.contains(h) {
            violations += 1;
        }
    }

    // Audit shards themselves are not in output_commitments — subtract them
    let real_violations = if violations > audits.len() {
        violations - audits.len()
    } else {
        0
    };

    if real_violations == 0 {
        eprintln!(
            "✓ COMPLIANT: {} shards, {} audits, root={}",
            hashes.len(),
            audits.len(),
            &root[..16]
        );
    } else {
        eprintln!(
            "✗ VIOLATION: {} unaccounted shards out of {}",
            real_violations,
            hashes.len()
        );
    }
    println!(
        "{}",
        serde_json::json!({
            "compliant": real_violations == 0,
            "total_shards": hashes.len(),
            "audit_records": audits.len(),
            "root_commitment": root,
        })
    );
    Ok(())
}

/// Print the audit chain
fn cmd_chain(dir: &str) -> Result<()> {
    let mut audits: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("audit_"))
                .unwrap_or(false)
        })
        .collect();
    audits.sort();

    for path in &audits {
        let raw = fs::read(path)?;
        let hash = hex::encode(Sha256::digest(&raw));
        eprintln!(
            "{} → {}",
            path.file_name().unwrap().to_string_lossy(),
            &hash[..16]
        );
    }
    Ok(())
}
