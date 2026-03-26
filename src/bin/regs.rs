/// zkperf-regs: register divergence map across hash functions
///
/// Sub-clusters the main code region by IP, compares register patterns,
/// shows where functions diverge and which paths are slow.

use linux_perf_data::{PerfFileReader, PerfFileRecord};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

const REGS: [(u64, &str); 14] = [
    (0,"AX"),(1,"BX"),(2,"CX"),(3,"DX"),(4,"SI"),(5,"DI"),
    (7,"R8"),(8,"R9"),(9,"R10"),(10,"R11"),(11,"R12"),(12,"R13"),(13,"R14"),(14,"R15"),
];

struct Sample { ip: u64, ts: u64, regs: [Option<u64>; 14] }

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: zkperf-regs <perf.data>"); std::process::exit(1);
    });

    let file = BufReader::new(File::open(&path)?);
    let PerfFileReader { mut perf_file, mut record_iter, .. } =
        PerfFileReader::parse_file(file)?;

    let mut samples: Vec<Sample> = Vec::new();
    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { record, .. } = record {
            let ts = record.common_data().ok().and_then(|c| c.timestamp).unwrap_or(0);
            if let Ok(parsed) = record.parse() {
                use linux_perf_data::linux_perf_event_reader::EventRecord;
                if let EventRecord::Sample(s) = parsed {
                    let ip = s.ip.unwrap_or(0);
                    let mut regs = [None; 14];
                    if let Some(ref ir) = s.intr_regs {
                        for (i, &(idx, _)) in REGS.iter().enumerate() {
                            regs[i] = ir.get(idx);
                        }
                    }
                    samples.push(Sample { ip, ts, regs });
                }
            }
        }
    }
    eprintln!("{} samples", samples.len());

    // Find the main code region (largest contiguous user-space IP range)
    let user_samples: Vec<&Sample> = samples.iter().filter(|s| s.ip < 0x7f0000000000).collect();
    let ip_min = user_samples.iter().map(|s| s.ip).min().unwrap_or(0);
    let ip_max = user_samples.iter().map(|s| s.ip).max().unwrap_or(0);

    // Sub-cluster by 256-byte blocks within the binary
    let block_shift = 8; // 256 bytes per block
    let mut blocks: BTreeMap<u64, Vec<usize>> = BTreeMap::new(); // block → sample indices
    for (i, s) in samples.iter().enumerate() {
        if s.ip >= ip_min && s.ip <= ip_max {
            blocks.entry(s.ip >> block_shift).or_default().push(i);
        }
    }

    // Merge adjacent blocks into sub-regions (gaps > 1 block = new sub-region)
    let mut sub_regions: Vec<(u64, u64, Vec<usize>)> = Vec::new(); // (start_block, end_block, sample_indices)
    for (&block, indices) in &blocks {
        if let Some(last) = sub_regions.last_mut() {
            if block <= last.1 + 2 { // allow 1-block gap
                last.1 = block;
                last.2.extend(indices);
                continue;
            }
        }
        sub_regions.push((block, block, indices.clone()));
    }

    // Filter to significant sub-regions (>= 50 samples)
    let sig_regions: Vec<_> = sub_regions.iter()
        .filter(|(_, _, idx)| idx.len() >= 50)
        .collect();

    println!("═══ SUB-REGIONS (256-byte blocks, ≥50 samples) ═══\n");
    println!("{:>4} {:>14} {:>14} {:>8} {:>6}  fingerprint", "id", "start", "end", "samples", "bytes");
    println!("{}", "─".repeat(90));

    // Compute fingerprint per sub-region
    struct RegionInfo { fp: String, reg_vals: HashMap<usize, Vec<u64>>, count: usize, start: u64, end: u64 }
    let mut infos: Vec<RegionInfo> = Vec::new();

    for (sri, (start, end, indices)) in sig_regions.iter().enumerate() {
        let mut reg_vals: HashMap<usize, Vec<u64>> = HashMap::new();
        for &i in indices.iter() {
            for (ri, &v) in samples[i].regs.iter().enumerate() {
                if let Some(val) = v { reg_vals.entry(ri).or_default().push(val); }
            }
        }
        let mut fp = String::new();
        for (ri, &(_, _name)) in REGS.iter().enumerate() {
            let tag = match reg_vals.get(&ri) {
                None => ' ',
                Some(v) if v.is_empty() => ' ',
                Some(v) => {
                    let mn = *v.iter().min().unwrap();
                    let mx = *v.iter().max().unwrap();
                    let r = mx.saturating_sub(mn);
                    if r == 0 { 'C' } else if r < 0x100 { 'L' } else if r < 0x10000 { 'M' } else { 'H' }
                }
            };
            fp.push(tag);
        }
        let s = *start; let e = *end;
        let bytes = ((e - s + 1) << block_shift) as usize;
        println!("S{:<3} {:14x} {:14x} {:8} {:6}  {}", sri, s << block_shift, (e+1) << block_shift, indices.len(), bytes, fp);
        infos.push(RegionInfo { fp, reg_vals, count: indices.len(), start: s, end: e });
    }

    // Divergence map: compare adjacent sub-regions
    println!("\n═══ DIVERGENCE MAP (register changes between adjacent sub-regions) ═══\n");
    for i in 0..infos.len().saturating_sub(1) {
        let a = &infos[i];
        let b = &infos[i + 1];
        let mut changes = Vec::new();
        for (ri, &(_, name)) in REGS.iter().enumerate() {
            let a_fp = a.fp.chars().nth(ri).unwrap_or(' ');
            let b_fp = b.fp.chars().nth(ri).unwrap_or(' ');
            if a_fp != b_fp {
                changes.push(format!("{}:{}→{}", name, a_fp, b_fp));
            }
        }
        if !changes.is_empty() {
            println!("  S{} → S{}: {}", i, i+1, changes.join(", "));
        }
    }

    // Slow paths: sub-regions with highest sample density (samples per byte)
    println!("\n═══ SLOW PATHS (highest sample density = most time per byte) ═══\n");
    let mut density: Vec<(usize, f64)> = infos.iter().enumerate()
        .map(|(i, info)| {
            let bytes = ((info.end - info.start + 1) << block_shift) as f64;
            (i, info.count as f64 / bytes.max(1.0))
        }).collect();
    density.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!("{:>4} {:>10} {:>8} {:>8}  fingerprint", "id", "density", "samples", "bytes");
    println!("{}", "─".repeat(50));
    for (sri, d) in density.iter().take(10) {
        let info = &infos[*sri];
        let bytes = (info.end - info.start + 1) << block_shift;
        println!("S{:<3} {:10.1} {:8} {:8}  {}", sri, d, info.count, bytes, info.fp);
    }

    // Identical sub-regions (same fingerprint = same computation pattern)
    println!("\n═══ IDENTICAL COMPUTATION PATTERNS ═══\n");
    let mut fp_groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, info) in infos.iter().enumerate() {
        fp_groups.entry(&info.fp).or_default().push(i);
    }
    for (fp, sris) in &fp_groups {
        if sris.len() > 1 {
            let total: usize = sris.iter().map(|&i| infos[i].count).sum();
            println!("  pattern [{}]: S{:?} ({} total samples)", fp, sris, total);
        }
    }

    // Unique sub-regions (fingerprint appears only once = unique algorithm)
    println!("\n═══ UNIQUE COMPUTATION PATTERNS ═══\n");
    for (fp, sris) in &fp_groups {
        if sris.len() == 1 {
            let i = sris[0];
            println!("  S{}: [{}] ({} samples) — unique algorithm", i, fp, infos[i].count);
        }
    }

    // Shared constant values (potential round constants / IV)
    println!("\n═══ SHARED CONSTANTS (same value in 3+ sub-regions) ═══\n");
    for (ri, &(_, name)) in REGS.iter().enumerate() {
        let mut val_sris: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, info) in infos.iter().enumerate() {
            if let Some(vals) = info.reg_vals.get(&ri) {
                let mn = *vals.iter().min().unwrap();
                let mx = *vals.iter().max().unwrap();
                if mn == mx { val_sris.entry(mn).or_default().push(i); }
            }
        }
        for (val, sris) in &val_sris {
            if sris.len() >= 3 {
                println!("  {}=0x{:x} in S{:?}", name, val, sris);
            }
        }
    }

    Ok(())
}
