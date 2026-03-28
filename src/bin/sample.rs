//! zkperf-sample: read first N samples from a perf.data file.
//!
//! Handles large/corrupt files gracefully by stopping after --limit samples.
//! Outputs event counts and Monster address as JSON.
//!
//! Usage:
//!   zkperf-sample <perf.data>                  — first 1000 samples
//!   zkperf-sample <perf.data> --limit 100      — first 100 samples
//!   zkperf-sample <perf.data> --limit 0        — header only

use anyhow::Result;
use linux_perf_data::{PerfFileReader, PerfFileRecord};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

const SSP: [u64; 15] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 41, 47, 59, 71];

#[derive(Serialize)]
struct SampleReport {
    file: String,
    arch: String,
    n_event_types: usize,
    samples_read: usize,
    event_counts: BTreeMap<String, u64>,
    monster_addr: [u64; 3],
    commitment: String,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: zkperf-sample <perf.data> [--limit N]");
        std::process::exit(1);
    }

    let path = &args[0];
    let limit: usize = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);

    let file = File::open(path)?;
    let reader = BufReader::with_capacity(64 * 1024 * 1024, file);
    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)?;

    let arch = perf_file
        .arch()
        .ok()
        .flatten()
        .unwrap_or_default()
        .to_string();

    let event_names: Vec<String> = perf_file
        .event_attributes()
        .iter()
        .filter_map(|a| a.name().map(|s| s.to_string()))
        .collect();

    let mut event_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut count = 0usize;

    while count < limit || limit == 0 {
        match record_iter.next_record(&mut perf_file) {
            Ok(Some(record)) => {
                if let PerfFileRecord::EventRecord { attr_index, .. } = &record {
                    let name = event_names
                        .get(*attr_index as usize)
                        .cloned()
                        .unwrap_or_else(|| format!("event_{}", attr_index));
                    *event_counts.entry(name).or_insert(0) += 1;
                }
                count += 1;
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("stopped at record {}: {}", count, e);
                break;
            }
        }
    }

    // Monster address from event counts
    let total: u64 = event_counts.values().sum();
    let cycles = event_counts
        .iter()
        .find(|(k, _)| k.contains("cycles"))
        .map(|(_, v)| *v)
        .unwrap_or(0);
    let instructions = event_counts
        .iter()
        .find(|(k, _)| k.contains("instructions"))
        .map(|(_, v)| *v)
        .unwrap_or(0);
    let cache_misses = event_counts
        .iter()
        .find(|(k, _)| k.contains("cache-misses"))
        .map(|(_, v)| *v)
        .unwrap_or(0);

    // Commitment
    let commit_str = serde_json::to_string(&event_counts)?;
    let commitment = format!("{:x}", md5_simple(commit_str.as_bytes()));

    let report = SampleReport {
        file: path.clone(),
        arch,
        n_event_types: event_names.len(),
        samples_read: count,
        event_counts,
        monster_addr: [cycles % 71, instructions % 59, cache_misses % 47],
        commitment,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn md5_simple(data: &[u8]) -> u128 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    data.hash(&mut h1);
    let a = h1.finish();
    let mut h2 = DefaultHasher::new();
    (data, a).hash(&mut h2);
    let b = h2.finish();
    ((a as u128) << 64) | (b as u128)
}
