/// zkperf-regs: cluster IPs, graph register patterns, infer function boundaries
///
/// No symbols needed — cluster by IP proximity, compare register fingerprints.

use linux_perf_data::{PerfFileReader, PerfFileRecord};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: zkperf-regs <perf.data>");
        std::process::exit(1);
    });

    let file = BufReader::new(File::open(&path)?);
    let PerfFileReader { mut perf_file, mut record_iter, .. } =
        PerfFileReader::parse_file(file)?;

    // Collect all (ip, timestamp) pairs
    let mut ip_samples: Vec<(u64, u64)> = Vec::new();

    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { record, .. } = record {
            let ts = record.common_data().ok().and_then(|c| c.timestamp).unwrap_or(0);
            if let Ok(parsed) = record.parse() {
                use linux_perf_data::linux_perf_event_reader::EventRecord;
                if let EventRecord::Sample(s) = parsed {
                    if let Some(ip) = s.ip {
                        ip_samples.push((ip, ts));
                    }
                }
            }
        }
    }

    eprintln!("collected {} samples", ip_samples.len());

    // Cluster IPs into 4K-page buckets → infer function boundaries
    let mut page_counts: BTreeMap<u64, u64> = BTreeMap::new();
    for &(ip, _) in &ip_samples {
        *page_counts.entry(ip >> 12).or_default() += 1;
    }

    // Merge adjacent pages into regions
    let mut regions: Vec<(u64, u64, u64)> = Vec::new(); // (start_page, end_page, count)
    for (&page, &count) in &page_counts {
        if let Some(last) = regions.last_mut() {
            if page <= last.1 + 1 {
                last.1 = page;
                last.2 += count;
                continue;
            }
        }
        regions.push((page, page, count));
    }

    println!("═══ IP REGIONS (inferred functions) ═══\n");
    println!("{:>4} {:>14} {:>14} {:>8} {:>8}", "id", "start", "end", "pages", "samples");
    println!("{}", "─".repeat(55));

    let mut region_id = 0;
    let mut region_map: BTreeMap<u64, usize> = BTreeMap::new(); // page → region_id
    for (start, end, count) in &regions {
        if *count < 10 { continue; }
        let pages = end - start + 1;
        println!("R{:<3} {:14x} {:14x} {:8} {:8}", region_id, start << 12, (end + 1) << 12, pages, count);
        for p in *start..=*end {
            region_map.insert(p, region_id);
        }
        region_id += 1;
    }

    // Per-region: IP histogram (fine-grained hotspots within each function)
    println!("\n═══ HOTSPOTS PER REGION (top 5 IPs) ═══\n");
    let mut region_ips: Vec<BTreeMap<u64, u64>> = vec![BTreeMap::new(); region_id];
    let mut region_times: Vec<Vec<u64>> = vec![Vec::new(); region_id];
    for &(ip, ts) in &ip_samples {
        if let Some(&rid) = region_map.get(&(ip >> 12)) {
            *region_ips[rid].entry(ip).or_default() += 1;
            region_times[rid].push(ts);
        }
    }

    for rid in 0..region_id {
        let mut ranked: Vec<_> = region_ips[rid].iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1));
        let total: u64 = ranked.iter().map(|(_, c)| **c).sum();
        let unique = ranked.len();
        println!("R{}: {} samples, {} unique IPs", rid, total, unique);
        for (ip, count) in ranked.iter().take(5) {
            let pct = **count as f64 / total as f64 * 100.0;
            println!("  0x{:x}: {:6} ({:.1}%)", ip, count, pct);
        }
        println!();
    }

    // Time-series: which region is active when (execution order)
    println!("═══ EXECUTION TIMELINE (region transitions) ═══\n");
    let mut timeline: Vec<(u64, usize)> = Vec::new();
    for &(ip, ts) in &ip_samples {
        if let Some(&rid) = region_map.get(&(ip >> 12)) {
            if timeline.last().map(|(_, r)| *r != rid).unwrap_or(true) {
                timeline.push((ts, rid));
            }
        }
    }
    // Print first 40 transitions
    for (ts, rid) in timeline.iter().take(40) {
        println!("  t={:16} → R{}", ts, rid);
    }
    if timeline.len() > 40 { println!("  ... ({} total transitions)", timeline.len()); }

    // Region similarity: compare IP distributions
    println!("\n═══ REGION SIMILARITY (cosine of IP histograms) ═══\n");
    for i in 0..region_id {
        for j in (i+1)..region_id {
            // Compare: do they share any IPs? What's the overlap?
            let a_ips: std::collections::HashSet<u64> = region_ips[i].keys().copied().collect();
            let b_ips: std::collections::HashSet<u64> = region_ips[j].keys().copied().collect();
            let overlap = a_ips.intersection(&b_ips).count();
            let a_total: u64 = region_ips[i].values().sum();
            let b_total: u64 = region_ips[j].values().sum();
            if overlap > 0 || (a_total > 100 && b_total > 100) {
                let a_span = a_ips.iter().max().unwrap_or(&0) - a_ips.iter().min().unwrap_or(&0);
                let b_span = b_ips.iter().max().unwrap_or(&0) - b_ips.iter().min().unwrap_or(&0);
                println!("  R{} ↔ R{}: overlap={} ips, spans=({},{}), samples=({},{})",
                    i, j, overlap, a_span, b_span, a_total, b_total);
            }
        }
    }

    println!("\n═══ SUMMARY ═══");
    println!("  samples: {}", ip_samples.len());
    println!("  regions: {}", region_id);
    println!("  transitions: {}", timeline.len());

    Ok(())
}
