//! Witness sharing: bundle witnesses with a Merkle root commitment.
//!
//! A `WitnessBundle` is the unit of exchange between nodes.
//! It contains a set of witnesses and a Merkle tree root over
//! their individual commitments, so any subset can be verified.

use crate::Witness;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessBundle {
    pub witnesses: Vec<OwnedWitness>,
    pub merkle_root: String,
    pub created_at: u64,
    pub node_id: String,
}

/// Owned version of Witness for serialization across process boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedWitness {
    pub context: String,
    pub signature: String,
    pub complexity: String,
    pub max_n: u64,
    pub max_ms: u64,
    pub elapsed_ms: u64,
    pub violated: bool,
    pub timestamp: u64,
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perf: Option<crate::PerfReadings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations: Option<crate::Violations>,
}

impl From<&Witness> for OwnedWitness {
    fn from(w: &Witness) -> Self {
        Self {
            context: w.context.into(),
            signature: w.signature.into(),
            complexity: w.complexity.into(),
            max_n: w.max_n,
            max_ms: w.max_ms,
            elapsed_ms: w.elapsed_ms,
            violated: w.violated,
            timestamp: w.timestamp,
            platform: w.platform.into(),
            perf: w.perf.clone(),
            violations: w.violations.clone(),
        }
    }
}

impl OwnedWitness {
    pub fn commitment(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.signature.as_bytes());
        h.update(b"|");
        h.update(self.elapsed_ms.to_string().as_bytes());
        h.update(b"|");
        h.update(self.timestamp.to_string().as_bytes());
        if let Some(ref p) = self.perf {
            if let Some(c) = p.cycles { h.update(c.to_string().as_bytes()); }
            if let Some(i) = p.instructions { h.update(i.to_string().as_bytes()); }
        }
        hex::encode(h.finalize())
    }
}

impl WitnessBundle {
    /// Create a bundle from a set of witnesses.
    pub fn new(witnesses: Vec<OwnedWitness>, node_id: &str) -> Self {
        let merkle_root = merkle_root(&witnesses);
        Self {
            witnesses,
            merkle_root,
            created_at: crate::now_ms(),
            node_id: node_id.into(),
        }
    }

    /// Verify the Merkle root matches the witnesses.
    pub fn verify(&self) -> bool {
        self.merkle_root == merkle_root(&self.witnesses)
    }

    /// Serialize to JSON for exchange.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Export to file.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = self.to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// Import from file.
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        Self::from_json(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

/// Build a Merkle root from witness commitments.
fn merkle_root(witnesses: &[OwnedWitness]) -> String {
    if witnesses.is_empty() {
        return hex::encode(Sha256::digest(b"empty"));
    }
    let mut hashes: Vec<[u8; 32]> = witnesses
        .iter()
        .map(|w| {
            let c = w.commitment();
            let mut out = [0u8; 32];
            out.copy_from_slice(&hex::decode(&c).unwrap_or_else(|_| vec![0; 32]));
            out
        })
        .collect();

    while hashes.len() > 1 {
        let mut next = Vec::with_capacity((hashes.len() + 1) / 2);
        for pair in hashes.chunks(2) {
            let mut h = Sha256::new();
            h.update(pair[0]);
            if pair.len() > 1 {
                h.update(pair[1]);
            } else {
                h.update(pair[0]); // duplicate odd leaf
            }
            next.push(Into::<[u8; 32]>::into(h.finalize()));
        }
        hashes = next;
    }

    hex::encode(hashes[0])
}

/// Load all witness files from ~/.zkperf/witnesses/ and bundle them.
pub fn bundle_all(node_id: &str) -> std::io::Result<WitnessBundle> {
    let dir = crate::dirs_fallback().join("witnesses");
    let mut witnesses = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.filter_map(|e| e.ok()) {
            if entry.path().extension().map_or(false, |x| x == "json") {
                if let Ok(data) = std::fs::read_to_string(entry.path()) {
                    if let Ok(w) = serde_json::from_str::<OwnedWitness>(&data) {
                        witnesses.push(w);
                    }
                }
            }
        }
    }
    Ok(WitnessBundle::new(witnesses, node_id))
}
