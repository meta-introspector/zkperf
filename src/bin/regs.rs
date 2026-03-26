/// zkperf-regs: IP clustering + register pattern analysis
///
/// Clusters IPs into regions, extracts register values per region,
/// finds reused values and shared patterns across potential hash functions.

use linux_perf_data::{PerfFileReader, PerfFileRecord};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

// x86_64 register indices for perf
const REG_NAMES: [(u64, &str); 14] = [
    (0, "AX"), (1, "BX"), (2, "CX"), (3, "DX"),
    (4, "SI"), (5, "DI"),
    (7, "R8"), (8, "R9"), (9, "R10"), (10, "R11"),
    (11, "R12"), (12, "R13"), (13, "R14"), (14, "R15"),
];

struct Sample { ip: u64, regs: Vec<(u64, u64)> } // (reg_idx, value)

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: zkperf-regs <perf.data>");
        std::process::exit(1);
    });

    let file = BufReader::new(File::open(&path)?);
    let PerfFileReader { mut perf_file, mut record_iter, .. } =
        PerfFileReader::parse_file(file)?;

    let mut samples: Vec<Sample> = Vec::new();

    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { record, .. } = record {
            if let Ok(parsed) = record.parse() {
                use linux_perf_data::linux_perf_event_reader::EventRecord;
                if let EventRecord::Sample(s) = parsed {
                    let ip = s.ip.unwrap_or(0);
                    let mut regs = Vec::new();
                    if let Some(ref ir) = s.intr_regs {
                        for &(idx, _) in &REG_NAMES {
                            if let Some(val) = ir.get(idx) {
                                regs.push((idx, val));
                            }
                        }
                    }
                    samples.push(Sample { ip, regs });
                }
            }
        }
    }

    let with_regs = samples.iter().filter(|s| !s.regs.is_empty()).count();
    eprintln!("samples: {}, with registers: {}", samples.len(), with_regs);

    // Cluster IPs into 4K-page regions
    let mut page_counts: BTreeMap<u64, u64> = BTreeMap::new();
    for s in &samples { *page_counts.entry(s.ip >> 12).or_default() += 1; }

    let mut regions: Vec<(u64, u64, u64)> = Vec::new();
    for (&page, &count) in &page_counts {
        if let Some(last) = regions.last_mut() {
            if page <= last.1 + 1 { last.1 = page; last.2 += count; continue; }
        }
        regions.push((page, page, count));
    }

    // Assign samples to regions
    let mut region_map: BTreeMap<u64, usize> = BTreeMap::new();
    let mut rid = 0;
    for (start, end, count) in &regions {
        if *count < 10 { continue; }
        for p in *start..=*end { region_map.insert(p, rid); }
        rid += 1;
    }
    let n_regions = rid;

    // Collect register values per region
    let mut region_regs: Vec<HashMap<u64, Vec<u64>>> = vec![HashMap::new(); n_regions];
    for s in &samples {
        if let Some(&rid) = region_map.get(&(s.ip >> 12)) {
            for &(idx, val) in &s.regs {
                region_regs[rid].entry(idx).or_default().push(val);
            }
        }
    }

    // Print regions
    println!("═══ REGIONS ═══\n");
    rid = 0;
    for (start, end, count) in &regions {
        if *count < 10 {continue;}
        let has_regs = !region_regs[rid].is_empty();
        println!("R{}: 0x{:x}-0x{:x} ({} samples, regs={})",
            rid, start << 12, (end+1) << 12, count, has_regs);
        rid += 1;
    }

    if with_regs == 0 {
        println!("\n⚠ No register data found. Re-record with --intr-regs=AX,BX,CX,DX,SI,DI,R8,...");
        println!("  make -f Makefile.zkperf record");
        return Ok(());
    }

    // Per-region register fingerprint
    println!("\n═══ REGISTER FINGERPRINTS PER REGION ═══\n");
    let mut fingerprints: Vec<Vec<char>> = Vec::new();
    for rid in 0..n_regions {
        let mut fp = Vec::new();
        print!("R{:2}: ", rid);
        for &(idx, name) in &REG_NAMES {
            let vals = region_regs[rid].get(&idx);
            let tag = match vals {
                None => ' ',
                Some(v) if v.is_empty() => ' ',
                Some(v) => {
                    let mn = *v.iter().min().unwrap();
                    let mx = *v.iter().max().unwrap();
                    let range = mx.saturating_sub(mn);
                    if range == 0 { 'C' }       // constant
                    else if range < 0x100 { 'L' } // low
                    else if range < 0x10000 { 'M' } // medium
                    else { 'H' }                  // high variance
                }
            };
            fp.push(tag);
            print!("{}:{} ", name, tag);
        }
        println!();
        fingerprints.push(fp);
    }

    // Find regions with identical fingerprints
    println!("\n═══ IDENTICAL REGISTER PATTERNS ═══\n");
    let mut fp_groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (rid, fp) in fingerprints.iter().enumerate() {
        let key: String = fp.iter().collect();
        fp_groups.entry(key).or_default().push(rid);
    }
    for (fp, rids) in &fp_groups {
        if rids.len() > 1 {
            println!("  pattern [{}]: regions {:?}", fp, rids);
        }
    }

    // Find reused register VALUES across regions
    println!("\n═══ SHARED REGISTER VALUES ACROSS REGIONS ═══\n");
    for &(idx, name) in &REG_NAMES {
        let mut val_to_regions: HashMap<u64, HashSet<usize>> = HashMap::new();
        for rid in 0..n_regions {
            if let Some(vals) = region_regs[rid].get(&idx) {
                for &v in vals {
                    val_to_regions.entry(v).or_default().insert(rid);
                }
            }
        }
        // Values appearing in 3+ regions
        let shared: Vec<_> = val_to_regions.iter()
            .filter(|(_, rs)| rs.len() >= 3)
            .collect();
        if !shared.is_empty() {
            println!("  {}: {} values shared across 3+ regions", name, shared.len());
            for (val, rs) in shared.iter().take(5) {
                let mut rids: Vec<_> = rs.iter().collect();
                rids.sort();
                println!("    0x{:016x} → R{:?}", val, rids);
            }
        }
    }

    // Per-region: constant registers (potential magic numbers / state)
    println!("\n═══ CONSTANT REGISTERS (magic numbers per region) ═══\n");
    for rid in 0..n_regions {
        let mut constants = Vec::new();
        for &(idx, name) in &REG_NAMES {
            if let Some(vals) = region_regs[rid].get(&idx) {
                if vals.len() >= 10 {
                    let mn = *vals.iter().min().unwrap();
                    let mx = *vals.iter().max().unwrap();
                    if mn == mx {
                        constants.push((name, mn));
                    }
                }
            }
        }
        if !constants.is_empty() {
            print!("  R{}: ", rid);
            for (name, val) in &constants {
                print!("{}=0x{:x} ", name, val);
            }
            println!();
        }
    }

    println!("\n═══ SUMMARY ═══");
    println!("  samples: {} ({} with regs)", samples.len(), with_regs);
    println!("  regions: {}", n_regions);
    println!("  fingerprint groups: {}", fp_groups.len());

    Ok(())
}
