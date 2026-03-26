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
use syn::{Expr, Item, ItemFn, Stmt, Visibility};

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
        _ => eprintln!("usage: cargo zkperf <audit|annotate|report> [path]"),
    }
}
