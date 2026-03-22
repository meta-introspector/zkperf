use zkperf_macros::witness_boundary;

// Simple time-only boundary
#[witness_boundary(complexity = "O(1)", max_n = 0, max_ms = 5000)]
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// Perf-constrained boundary
#[witness_boundary(
    complexity = "O(n)",
    max_n = 1000,
    max_ms = 100,
    max_cycles = 50_000_000,
    max_instructions = 20_000_000,
    max_cache_misses = 10_000,
    context = "search_tool"
)]
fn linear_search(haystack: &[i32], needle: i32) -> Option<usize> {
    haystack.iter().position(|&x| x == needle)
}

// Tight budget — will violate if perf counters are available
#[witness_boundary(
    complexity = "O(1)",
    max_n = 0,
    max_ms = 1000,
    max_cycles = 100,
    max_instructions = 50,
    context = "tight_budget"
)]
fn will_violate_perf() -> u64 {
    (0..10_000u64).sum()
}

#[tokio::main]
async fn main() {
    // Run boundaries (generates witnesses + cache + proofs automatically)
    let result = add(2, 3);
    println!("add(2,3) = {result}");

    let data: Vec<i32> = (0..100).collect();
    let found = linear_search(&data, 42);
    println!("linear_search found 42 at: {found:?}");

    let sum = will_violate_perf();
    println!("tight_budget sum = {sum}");

    // Run add again to show cache aggregation
    let _ = add(10, 20);
    let _ = add(100, 200);

    // --- Cache ---
    println!("\n=== CACHE ===");
    for entry in zkperf_witness::cache::list_all() {
        println!(
            "  {} count={} avg={}ms min={}ms max={}ms violations={}",
            entry.context, entry.count, entry.avg_ms(),
            entry.min_ms, entry.max_elapsed_ms, entry.violation_count,
        );
    }

    // --- Share ---
    println!("\n=== SHARE ===");
    let bundle = zkperf_witness::share::bundle_all("demo-node").unwrap();
    println!("  bundle: {} witnesses, merkle_root={}", bundle.witnesses.len(), &bundle.merkle_root[..16]);
    println!("  verified: {}", bundle.verify());

    let tmp = std::env::temp_dir().join("zkperf-demo-bundle.json");
    bundle.save(&tmp).unwrap();
    let loaded = zkperf_witness::share::WitnessBundle::load(&tmp).unwrap();
    println!("  round-trip: {} witnesses, verified={}", loaded.witnesses.len(), loaded.verify());

    // --- ZKP ---
    println!("\n=== ZKP ===");
    let proof_dir = zkperf_witness::dirs_fallback().join("proofs");
    if let Ok(rd) = std::fs::read_dir(&proof_dir) {
        for entry in rd.filter_map(|e| e.ok()).take(3) {
            let data = std::fs::read_to_string(entry.path()).unwrap();
            let proof: zkperf_witness::zkp::WitnessProof = serde_json::from_str(&data).unwrap();
            let name = entry.path().file_name().unwrap().to_string_lossy().to_string();
            println!(
                "  {} satisfied={} ranges={} verified={}",
                name, proof.satisfied, proof.range_proofs.len(),
                zkperf_witness::zkp::verify(&proof),
            );
        }
    }
}
