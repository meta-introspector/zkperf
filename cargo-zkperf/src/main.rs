//! cargo-zkperf — lint + auto-annotate Rust code with zkperf instrumentation
//!
//! Usage:
//!   cargo-zkperf audit  [path]   — scan, report unannotated functions + risk
//!   cargo-zkperf annotate [path] — auto-add #[zkperf] to public functions
//!   cargo-zkperf report [path]   — JSON risk assessment rollup

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use syn::visit::Visit;
use syn::{Expr, ItemFn, Visibility};

#[derive(Debug, Clone, Serialize)]
struct FnRisk {
    file: String,
    name: String,
    line: usize,
    loc: usize,
    is_pub: bool,
    is_async: bool,
    is_unsafe: bool,
    has_zkperf: bool,
    has_witness_boundary: bool,
    unsafe_blocks: usize,
    loop_count: usize,
    loop_depth: usize,
    call_count: usize,
    risk_score: u32,
    risk_level: &'static str,
    signature: String,
    // Inferred constraints (promises)
    inferred_complexity: &'static str,
    inferred_max_ms: u64,
}

#[derive(Default)]
struct Scanner {
    file: String,
    source: String,
    functions: Vec<FnRisk>,
}

impl<'ast> Visit<'ast> for Scanner {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let name = node.sig.ident.to_string();
        let is_pub = matches!(node.vis, Visibility::Public(_));
        let is_async = node.sig.asyncness.is_some();
        let is_unsafe = node.sig.unsafety.is_some();

        let has_zkperf = node.attrs.iter().any(|a| a.path().is_ident("zkperf"));
        let has_wb = node
            .attrs
            .iter()
            .any(|a| a.path().is_ident("witness_boundary"));

        let body_str = quote::quote!(#node).to_string();
        let loc = body_str.lines().count();

        let mut counter = BlockCounter::default();
        counter.visit_block(&node.block);

        let mut hasher = Sha256::new();
        hasher.update(format!("{}::{}", self.file, name).as_bytes());
        let sig = hex::encode(&hasher.finalize()[..8]);

        // Infer complexity from loop nesting
        let inferred_complexity = match counter.max_loop_depth {
            0 => "O(1)",
            1 => "O(n)",
            2 => "O(n^2)",
            3 => "O(n^3)",
            _ => "O(n^k)",
        };

        // Infer max_ms from LOC + complexity + calls
        let inferred_max_ms = match counter.max_loop_depth {
            0 => 100 + (counter.call_count as u64 * 10),
            1 => 1000 + (loc as u64 * 10),
            2 => 5000 + (loc as u64 * 50),
            _ => 30000,
        };

        // Risk scoring
        let mut risk: u32 = 0;
        if is_unsafe {
            risk += 30;
        }
        risk += counter.unsafe_blocks as u32 * 20;
        risk += counter.loop_count as u32 * 5;
        risk += (counter.max_loop_depth as u32).saturating_sub(1) * 15; // nested loops
        if loc > 50 {
            risk += 10;
        }
        if loc > 100 {
            risk += 15;
        }
        if !has_zkperf && !has_wb {
            risk += 10;
        }
        if is_pub {
            risk += 5;
        }

        let risk_level = match risk {
            0..=10 => "low",
            11..=30 => "medium",
            31..=60 => "high",
            _ => "critical",
        };

        let line = self.source[..self.source.find(&format!("fn {}", name)).unwrap_or(0)]
            .lines()
            .count()
            + 1;

        self.functions.push(FnRisk {
            file: self.file.clone(),
            name,
            line,
            loc,
            is_pub,
            is_async,
            is_unsafe,
            has_zkperf,
            has_witness_boundary: has_wb,
            unsafe_blocks: counter.unsafe_blocks,
            loop_count: counter.loop_count,
            loop_depth: counter.max_loop_depth,
            call_count: counter.call_count,
            risk_score: risk,
            risk_level,
            signature: sig,
            inferred_complexity,
            inferred_max_ms,
        });

        syn::visit::visit_item_fn(self, node);
    }
}

#[derive(Default)]
struct BlockCounter {
    unsafe_blocks: usize,
    loop_count: usize,
    loop_depth: usize,
    max_loop_depth: usize,
    call_count: usize,
}

impl<'ast> Visit<'ast> for BlockCounter {
    fn visit_expr(&mut self, node: &'ast Expr) {
        match node {
            Expr::Unsafe(_) => self.unsafe_blocks += 1,
            Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_) => {
                self.loop_count += 1;
                self.loop_depth += 1;
                if self.loop_depth > self.max_loop_depth {
                    self.max_loop_depth = self.loop_depth;
                }
                syn::visit::visit_expr(self, node);
                self.loop_depth -= 1;
                return;
            }
            Expr::Call(_) | Expr::MethodCall(_) => self.call_count += 1,
            _ => {}
        }
        syn::visit::visit_expr(self, node);
    }
}

fn scan_file(path: &Path) -> Vec<FnRisk> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let ast = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut scanner = Scanner {
        file: path.display().to_string(),
        source,
        ..Default::default()
    };
    scanner.visit_file(&ast);
    scanner.functions
}

fn scan_dir(dir: &str) -> Vec<FnRisk> {
    let pattern = format!("{}/**/*.rs", dir);
    glob::glob(&pattern)
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|p| scan_file(&p))
        .collect()
}

fn cmd_audit(dir: &str) {
    let fns = scan_dir(dir);
    let total = fns.len();
    let instrumented = fns
        .iter()
        .filter(|f| f.has_zkperf || f.has_witness_boundary)
        .count();
    let uninstrumented = total - instrumented;
    let critical = fns.iter().filter(|f| f.risk_level == "critical").count();
    let high = fns.iter().filter(|f| f.risk_level == "high").count();

    eprintln!("zkperf audit: {} functions in {}", total, dir);
    eprintln!(
        "  instrumented: {} ({:.0}%)",
        instrumented,
        instrumented as f64 / total.max(1) as f64 * 100.0
    );
    eprintln!("  uninstrumented: {}", uninstrumented);
    eprintln!("  risk: {} critical, {} high", critical, high);
    eprintln!();

    // Show uninstrumented functions sorted by risk
    let mut risky: Vec<_> = fns
        .iter()
        .filter(|f| !f.has_zkperf && !f.has_witness_boundary)
        .collect();
    risky.sort_by(|a, b| b.risk_score.cmp(&a.risk_score));

    for f in risky.iter().take(30) {
        let flags = format!(
            "{}{}{}",
            if f.is_pub { "pub " } else { "" },
            if f.is_async { "async " } else { "" },
            if f.is_unsafe { "unsafe " } else { "" }
        );
        eprintln!(
            "  {:>3} {:8} {}:{} {}fn {} [{}] max_ms={} ({}loc, d{}loops, {}unsafe, {}calls)",
            f.risk_score,
            f.risk_level,
            f.file.rsplit('/').next().unwrap_or(&f.file),
            f.line,
            flags,
            f.name,
            f.inferred_complexity,
            f.inferred_max_ms,
            f.loc,
            f.loop_depth,
            f.unsafe_blocks,
            f.call_count
        );
    }
    if risky.len() > 30 {
        eprintln!("  ... and {} more", risky.len() - 30);
    }
}

fn cmd_annotate(dir: &str) {
    let fns = scan_dir(dir);
    let targets: Vec<_> = fns
        .iter()
        .filter(|f| f.is_pub && !f.has_zkperf && !f.has_witness_boundary)
        .collect();

    eprintln!(
        "annotating {} public functions with inferred constraints...",
        targets.len()
    );

    let mut by_file: std::collections::HashMap<&str, Vec<&&FnRisk>> =
        std::collections::HashMap::new();
    for f in &targets {
        by_file.entry(&f.file).or_default().push(f);
    }

    for (file, fns) in &by_file {
        let source = std::fs::read_to_string(file).unwrap();
        let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
        let mut insertions: Vec<(usize, String)> = fns.iter()
            .map(|f| {
                let annotation = format!(
                    "#[zkperf_macros::witness_boundary(complexity = \"{}\", max_n = {}, max_ms = {})]",
                    f.inferred_complexity,
                    match f.inferred_complexity {
                        "O(1)" => 1,
                        "O(n)" => 10000,
                        "O(n^2)" => 1000,
                        "O(n^3)" => 100,
                        _ => 50,
                    },
                    f.inferred_max_ms,
                );
                (f.line - 1, annotation)
            })
            .collect();
        insertions.sort_by(|a, b| b.0.cmp(&a.0));
        for (line, annotation) in &insertions {
            if *line < lines.len()
                && !lines[*line].contains("#[zkperf")
                && !lines[*line].contains("witness_boundary")
            {
                lines.insert(*line, annotation.clone());
            }
        }
        std::fs::write(file, lines.join("\n")).unwrap();
        eprintln!("  {} — {} constraints added", file, fns.len());
        for f in fns {
            eprintln!(
                "    {} → {} max_ms={}",
                f.name, f.inferred_complexity, f.inferred_max_ms
            );
        }
    }
}

fn cmd_report(dir: &str) {
    let fns = scan_dir(dir);
    let total = fns.len();
    let instrumented = fns
        .iter()
        .filter(|f| f.has_zkperf || f.has_witness_boundary)
        .count();
    let total_risk: u32 = fns.iter().map(|f| f.risk_score).sum();

    #[derive(Serialize)]
    struct Report {
        total_functions: usize,
        instrumented: usize,
        coverage_pct: f64,
        total_risk: u32,
        avg_risk: f64,
        functions: Vec<FnRisk>,
    }

    let report = Report {
        total_functions: total,
        instrumented,
        coverage_pct: instrumented as f64 / total.max(1) as f64 * 100.0,
        total_risk,
        avg_risk: total_risk as f64 / total.max(1) as f64,
        functions: fns,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Support both `cargo zkperf` and `cargo-zkperf` invocation
    let (cmd, dir) = if args.len() > 1 && args[1] == "zkperf" {
        (
            args.get(2).map(|s| s.as_str()).unwrap_or("audit"),
            args.get(3).map(|s| s.as_str()).unwrap_or("src"),
        )
    } else {
        (
            args.get(1).map(|s| s.as_str()).unwrap_or("audit"),
            args.get(2).map(|s| s.as_str()).unwrap_or("src"),
        )
    };

    match cmd {
        "audit" => cmd_audit(dir),
        "annotate" => cmd_annotate(dir),
        "report" => cmd_report(dir),
        "shard" => cmd_shard(dir),
        "prompt" => cmd_prompt(dir),
        "verify" => cmd_verify_shard(dir),
        _ => eprintln!("usage: cargo zkperf <audit|annotate|report|shard|prompt|verify> [path]"),
    }
}

/// DA51 CBOR tag
const DASL_TAG: u64 = 55889;

/// Monster prime domain mapping
fn monster_domain(f: &FnRisk) -> (u64, &'static str) {
    let n = &f.name;
    if n.contains("crypt") || n.contains("hash") || n.contains("sign") { return (5, "crypto"); }
    if n.contains("net") || n.contains("connect") || n.contains("socket") || n.contains("wg") { return (7, "network"); }
    if n.contains("parse") || n.contains("lex") || n.contains("token") { return (13, "parse"); }
    if n.contains("prove") || n.contains("verify") || n.contains("witness") || n.contains("zkp") { return (17, "prove"); }
    if n.contains("store") || n.contains("save") || n.contains("write") || n.contains("cbor") { return (19, "store"); }
    if n.contains("graph") || n.contains("lattice") || n.contains("tree") { return (23, "graph"); }
    if n.contains("perf") || n.contains("monitor") || n.contains("trace") { return (29, "monitor"); }
    if n.contains("build") || n.contains("compile") || n.contains("cargo") { return (31, "compile"); }
    if n.contains("deploy") || n.contains("service") || n.contains("systemd") { return (37, "deploy"); }
    if n.contains("test") || n.contains("audit") || n.contains("check") || n.contains("scan") { return (41, "test"); }
    if n.contains("render") || n.contains("view") || n.contains("ui") || n.contains("html") { return (43, "ui"); }
    if n.contains("agent") || n.contains("mcp") || n.contains("tool") { return (47, "agent"); }
    if n.contains("stego") || n.contains("encode") || n.contains("decode") { return (53, "stego"); }
    if n.contains("tunnel") || n.contains("vpn") || n.contains("relay") { return (59, "vpn"); }
    if n.contains("record") || n.contains("rec") || n.contains("tmux") { return (61, "record"); }
    if n.contains("shard") || n.contains("distribute") { return (67, "shard"); }
    if n.contains("bootstrap") || n.contains("main") || n.contains("init") { return (71, "meta"); }
    if f.is_unsafe { return (2, "binary"); }
    (3, "general")
}

fn cmd_shard(dir: &str) {
    let fns = scan_dir(dir);
    let project = dir.rsplit('/').nth(1).unwrap_or("unknown");

    // Group functions by Monster prime
    let mut shards: std::collections::HashMap<u64, Vec<&FnRisk>> = std::collections::HashMap::new();
    for f in &fns {
        let (prime, _) = monster_domain(f);
        shards.entry(prime).or_default().push(f);
    }

    let out_dir = format!("{}/.zkperf/shards/{}", std::env::var("HOME").unwrap_or_default(), project);
    std::fs::create_dir_all(&out_dir).ok();

    let mut manifest = Vec::new();

    for (prime, funcs) in &shards {
        let (_, domain) = monster_domain(funcs[0]);
        let rows: Vec<Vec<String>> = funcs.iter().map(|f| vec![
            f.name.clone(), f.inferred_complexity.to_string(),
            f.inferred_max_ms.to_string(), f.risk_score.to_string(), f.signature.clone(),
        ]).collect();

        // Build DA51 CBOR shard
        let shard_data = serde_json::json!({
            "id": format!("{}-shard-{}", project, prime),
            "prime": prime,
            "domain": domain,
            "functions": funcs.len(),
            "total_risk": funcs.iter().map(|f| f.risk_score).sum::<u32>(),
            "table": {
                "headers": ["function", "complexity", "max_ms", "risk", "signature"],
                "rows": rows,
            },
            "tags": ["zkperf", format!("shard-{}", prime), domain],
        });

        // Encode as DA51 CBOR
        let val = ciborium::Value::serialized(&shard_data).unwrap();
        let tagged = ciborium::Value::Tag(DASL_TAG, Box::new(val));
        let mut cbor_buf = Vec::new();
        ciborium::into_writer(&tagged, &mut cbor_buf).unwrap();

        // Content hash
        let mut hasher = sha2::Sha256::new();
        sha2::Digest::update(&mut hasher, &cbor_buf);
        let cid = format!("bafk{}", &hex::encode(hasher.finalize())[..32]);

        let path = format!("{}/{}-{}.cbor", out_dir, domain, prime);
        std::fs::write(&path, &cbor_buf).ok();

        manifest.push(serde_json::json!({
            "prime": prime, "domain": domain, "cid": cid,
            "functions": funcs.len(), "risk": funcs.iter().map(|f| f.risk_score).sum::<u32>(),
            "path": path, "size": cbor_buf.len(),
        }));

        eprintln!("  shard-{:>2} {:8} {:>2} fns, risk {:>3}, {} bytes → {}",
            prime, domain, funcs.len(),
            funcs.iter().map(|f| f.risk_score).sum::<u32>(),
            cbor_buf.len(), cid);
    }

    // Write manifest
    let manifest_path = format!("{}/manifest.json", out_dir);
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).ok();
    eprintln!("\n{} shards written to {}", shards.len(), out_dir);
    eprintln!("manifest: {}", manifest_path);
}

/// Generate implementation prompts from CBOR shards.
/// Usage: cargo-zkperf prompt ~/.zkperf/shards/zos-server
fn cmd_prompt(shard_dir: &str) {
    let manifest_path = format!("{}/manifest.json", shard_dir);
    let manifest: Vec<serde_json::Value> = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => { eprintln!("no manifest.json in {}", shard_dir); return; }
    };

    for entry in &manifest {
        let path = entry["path"].as_str().unwrap_or("");
        let domain = entry["domain"].as_str().unwrap_or("unknown");
        let prime = entry["prime"].as_u64().unwrap_or(0);

        let raw = match std::fs::read(path) { Ok(r) => r, Err(_) => continue };
        let val: ciborium::Value = ciborium::from_reader(&raw[..]).unwrap();
        let data: serde_json::Value = if let ciborium::Value::Tag(55889, inner) = val {
            ciborium::Value::deserialized(&inner).unwrap_or_default()
        } else { continue };

        let rows = data["table"]["rows"].as_array().unwrap();
        let mut prompt = format!(
            "# Implement {} functions — \"{}\" shard (prime {})\n\n\
             Each function has a performance contract enforced by zkperf.\n\n\
             ```toml\n[dependencies]\nzkperf-macros = {{ path = \"zkperf-macros\" }}\nzkperf-witness = {{ path = \"zkperf-witness\" }}\n```\n\n",
            rows.len(), domain, prime
        );

        for row in rows {
            let r: Vec<&str> = row.as_array().unwrap().iter().map(|v| v.as_str().unwrap_or("")).collect();
            let (name, complexity, max_ms, risk, sig) = (r[0], r[1], r[2], r[3], r[4]);
            let max_n = match complexity { "O(1)" => "1", "O(n)" => "10000", "O(n^2)" => "1000", _ => "50" };
            prompt += &format!(
                "```rust\n#[witness_boundary(complexity = \"{}\", max_n = {}, max_ms = {})]\n\
                 fn {}() -> Result<(), Box<dyn std::error::Error>> {{\n    \
                 // Contract: <{}ms, {}, risk {}, sig {}\n    todo!()\n}}\n```\n\n",
                complexity, max_n, max_ms, name, max_ms, complexity, risk, sig
            );
        }

        prompt += "## Verify: `cargo-zkperf verify <src> <shard_dir>`\n";

        let prompt_path = format!("{}/{}-{}.prompt.md", shard_dir, domain, prime);
        std::fs::write(&prompt_path, &prompt).ok();
        eprintln!("  {} → {}", domain, prompt_path);
    }
}

/// Verify implementation matches shard contracts.
/// Usage: cargo-zkperf verify src ~/.zkperf/shards/project
fn cmd_verify_shard(args: &str) {
    // args = "src_dir shard_dir"
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let (src_dir, shard_dir) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        eprintln!("usage: cargo-zkperf verify <src_dir> <shard_dir>");
        return;
    };

    let manifest_path = format!("{}/manifest.json", shard_dir);
    let manifest: Vec<serde_json::Value> = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => { eprintln!("no manifest.json in {}", shard_dir); return; }
    };

    // Scan source for implemented functions
    let impl_fns = scan_dir(src_dir);
    let impl_map: std::collections::HashMap<&str, &FnRisk> = impl_fns.iter()
        .map(|f| (f.name.as_str(), f)).collect();

    let mut total = 0;
    let mut found = 0;
    let mut instrumented = 0;
    let mut mismatches = Vec::new();

    for entry in &manifest {
        let path = entry["path"].as_str().unwrap_or("");
        let raw = match std::fs::read(path) { Ok(r) => r, Err(_) => continue };
        let val: ciborium::Value = ciborium::from_reader(&raw[..]).unwrap();
        let data: serde_json::Value = if let ciborium::Value::Tag(55889, inner) = val {
            ciborium::Value::deserialized(&inner).unwrap_or_default()
        } else { continue };

        let domain = data["domain"].as_str().unwrap_or("?");
        let rows = data["table"]["rows"].as_array().unwrap();

        for row in rows {
            let name = row[0].as_str().unwrap_or("");
            let expected_complexity = row[1].as_str().unwrap_or("");
            total += 1;

            if let Some(f) = impl_map.get(name) {
                found += 1;
                if f.has_witness_boundary || f.has_zkperf { instrumented += 1; }
                if f.inferred_complexity != expected_complexity {
                    mismatches.push(format!("  {} [{}]: expected {}, got {}",
                        name, domain, expected_complexity, f.inferred_complexity));
                }
            }
        }
    }

    eprintln!("=== Shard Verification ===");
    eprintln!("  Functions in shards: {}", total);
    eprintln!("  Found in source:     {} ({:.0}%)", found, found as f64 / total.max(1) as f64 * 100.0);
    eprintln!("  Instrumented:        {} ({:.0}%)", instrumented, instrumented as f64 / found.max(1) as f64 * 100.0);
    if mismatches.is_empty() {
        eprintln!("  Complexity matches:  ✅ all match");
    } else {
        eprintln!("  Complexity mismatches: {}", mismatches.len());
        for m in &mismatches { eprintln!("{}", m); }
    }
}
