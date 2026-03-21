/// zkperf-parse: Parse perf stat output → witness JSON with commitment
///
/// Usage:
///   zkperf-parse <perf.txt> <source-file>     → single witness JSON
///   zkperf-parse --batch <dir>                 → all .perf files → JSONL + global commitment
///   zkperf-parse --combine                     → read witness JSONL from stdin → global commitment

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct Witness {
    file: String,
    counters: BTreeMap<String, u64>,
    commitment: String,
}

#[derive(Serialize)]
struct GlobalCommitment {
    global_commitment: String,
    n_witnesses: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage:");
        eprintln!("  zkperf-parse <perf.txt> <source-file>");
        eprintln!("  zkperf-parse --batch <dir>");
        eprintln!("  zkperf-parse --combine < witnesses.jsonl");
        std::process::exit(1);
    }

    match args[0].as_str() {
        "--batch" => cmd_batch(&args[1]),
        "--combine" => cmd_combine(),
        _ => {
            let source = args.get(1).map(|s| s.as_str()).unwrap_or("unknown");
            let w = parse_perf_file(&args[0], source);
            println!("{}", serde_json::to_string(&w).unwrap());
        }
    }
}

fn cmd_batch(dir: &str) {
    let mut witnesses = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "txt" || x == "perf").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.path());

    let stdout = io::stdout();
    let mut out = stdout.lock();

    for entry in &entries {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy();
        let source = format!("{}.agda", stem.trim_end_matches(".perf"));
        let w = parse_perf_file(&path.to_string_lossy(), &source);
        writeln!(out, "{}", serde_json::to_string(&w).unwrap()).unwrap();
        witnesses.push(w.commitment);
    }

    // Global commitment to stderr
    let global = make_global(&witnesses);
    eprintln!("{}", serde_json::to_string(&global).unwrap());
}

fn cmd_combine() {
    let stdin = io::stdin();
    let mut commits = Vec::new();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        if line.trim().is_empty() { continue; }
        if let Ok(w) = serde_json::from_str::<Witness>(&line) {
            commits.push(w.commitment);
        }
    }
    let global = make_global(&commits);
    println!("{}", serde_json::to_string(&global).unwrap());
}

fn parse_perf_file(path: &str, source: &str) -> Witness {
    let mut counters = BTreeMap::new();
    let text = fs::read_to_string(path).unwrap_or_default();

    for line in text.lines() {
        let line = line.trim();
        // Match: "123,456 cpu_core/cycles/" or "123,456 cycles"
        if let Some(c) = parse_counter_line(line) {
            counters.insert(c.0, c.1);
        }
    }

    let commitment = sha256_hex(&serde_json::to_string(&counters).unwrap());
    Witness {
        file: source.to_string(),
        counters,
        commitment,
    }
}

fn parse_counter_line(line: &str) -> Option<(String, u64)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 { return None; }

    let num_str = parts[0].replace(',', "");
    let val: u64 = num_str.parse().ok()?;

    let name = parts[1];
    // "cpu_core/cycles/" → "cycles"
    let clean = if name.contains('/') {
        name.split('/').nth(1).unwrap_or(name)
    } else {
        name
    };

    let known = ["cycles", "instructions", "cache-misses", "cache-references",
                  "branch-misses", "branches"];
    if known.iter().any(|&k| clean == k) {
        Some((clean.to_string(), val))
    } else {
        None
    }
}

fn make_global(commits: &[String]) -> GlobalCommitment {
    let mut sorted = commits.to_vec();
    sorted.sort();
    let combined = sorted.join("|");
    GlobalCommitment {
        global_commitment: sha256_hex(&combined),
        n_witnesses: commits.len(),
    }
}

fn sha256_hex(data: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // Minimal hash — replace with real SHA256 if available
    // Using two rounds of DefaultHasher to get 256 bits
    let mut h1 = DefaultHasher::new();
    data.hash(&mut h1);
    let a = h1.finish();
    let mut h2 = DefaultHasher::new();
    (data, a).hash(&mut h2);
    let b = h2.finish();
    let mut h3 = DefaultHasher::new();
    (data, b).hash(&mut h3);
    let c = h3.finish();
    let mut h4 = DefaultHasher::new();
    (data, c).hash(&mut h4);
    let d = h4.finish();
    format!("{:016x}{:016x}{:016x}{:016x}", a, b, c, d)
}
