//! zkperf-record — all perf interaction, raw binary reader, DA51 CBOR output
//!
//! Commands:
//!   run  <cmd> <out-dir>           — perf record then read raw → DA51 shards
//!   read <perf.data> <out-dir>     — read raw perf.data → DA51 shards
//!   agda <shard-dir> <out.agda>    — export shards as Agda module

use anyhow::Result;
use erdfa_publish::{Component, Shard};
use linux_perf_data::{PerfFileReader, PerfFileRecord};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Command;

/// Side-channel fields that leak information and should be redacted in private mode.
const PRIVATE_FIELDS: &[&str] = &["ip", "samples", "count", "pct", "cmdline", "source"];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let private = args.iter().any(|a| a == "--private");
    let args: Vec<&str> = args
        .iter()
        .filter(|a| *a != "--private")
        .map(|s| s.as_str())
        .collect();
    if args.is_empty() {
        eprintln!("usage:");
        eprintln!("  zkperf-record [--private] run  <cmd> <out-dir>");
        eprintln!("  zkperf-record [--private] read <perf.data> <out-dir>");
        eprintln!("  zkperf-record trace <perf.data>  — raw timeline (ts,ip,event) to stdout");
        eprintln!("  zkperf-record agda <shard-dir> <out.agda> [module]");
        eprintln!(
            "\n  --private  commit side-channel values (Merkle tree), redact sensitive fields"
        );
        std::process::exit(1);
    }
    match args[0] {
        "run" => cmd_run(args[1], args[2], private)?,
        "read" => cmd_read(args[1], args[2], private)?,
        "trace" => cmd_trace(args[1])?,
        "flow" => cmd_flow(args[1])?,
        "agda" => {
            let m = args.get(3).copied().unwrap_or("PerfTrace");
            cmd_agda(args[1], args[2], m)?;
        }
        _ => eprintln!("unknown: {}", args[0]),
    }
    Ok(())
}

fn cmd_run(cmd: &str, out_dir: &str, private: bool) -> Result<()> {
    let perf_data = format!("{}/perf.data", out_dir);
    fs::create_dir_all(out_dir)?;
    eprintln!(
        "recording{}: {}",
        if private { " (private)" } else { "" },
        cmd
    );
    Command::new("perf")
        .args([
            "record",
            "-g",
            "--call-graph",
            "dwarf,65528",
            "-e",
            "cycles:u,instructions:u,cache-misses:u,branch-misses:u",
            "-c",
            "100",
            "-o",
            &perf_data,
            "--",
            "sh",
            "-c",
            cmd,
        ])
        .status()?;
    cmd_read(&perf_data, out_dir, private)
}

/// Read raw perf.data binary — extract functions, instructions, timestamps
fn cmd_read(perf_path: &str, out_dir: &str, private: bool) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let file = File::open(perf_path)?;
    let reader = BufReader::new(file);
    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)?;

    // Metadata
    let arch = perf_file
        .arch()
        .ok()
        .flatten()
        .unwrap_or_default()
        .to_string();
    let cmdline = perf_file
        .cmdline()
        .ok()
        .flatten()
        .map(|v| {
            v.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let events: Vec<String> = perf_file
        .event_attributes()
        .iter()
        .filter_map(|a| a.name().map(|s| s.to_string()))
        .collect();

    let mut func_counts: HashMap<String, u64> = HashMap::new();
    let mut ip_counts: BTreeMap<u64, u64> = BTreeMap::new();
    let mut event_counts: HashMap<String, u64> = HashMap::new();
    let mut timestamps: Vec<u64> = Vec::new();
    let mut samples: Vec<SampleRecord> = Vec::new();
    let mut total = 0u64;

    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { attr_index, record } = record {
            total += 1;
            let event_name = events.get(attr_index).cloned().unwrap_or_default();
            *event_counts.entry(event_name.clone()).or_insert(0) += 1;

            // Timestamp
            let ts = record
                .common_data()
                .ok()
                .and_then(|cd| cd.timestamp)
                .unwrap_or(0);
            if ts > 0 {
                timestamps.push(ts);
            }

            // Parse the record for IP, pid, tid
            if let Ok(parsed) = record.parse() {
                let record_type = format!("{:?}", record.record_type);
                *func_counts.entry(record_type).or_insert(0) += 1;

                // Extract IP from debug repr (format: "Sample { ip: 0x..., ... }")
                let debug = format!("{:?}", parsed);
                if let Some(pos) = debug.find("ip: ") {
                    let rest = &debug[pos + 4..];
                    let end = rest
                        .find(|c: char| !c.is_ascii_hexdigit() && c != 'x')
                        .unwrap_or(rest.len());
                    if let Ok(ip) = u64::from_str_radix(rest[..end].trim_start_matches("0x"), 16) {
                        *ip_counts.entry(ip).or_insert(0) += 1;
                        samples.push(SampleRecord {
                            ts,
                            ip,
                            event: event_name,
                        });
                    }
                }
            }
        }
    }

    // === Emit DA51 CBOR shards ===

    // 1. Summary shard with metadata
    let commitment = hex::encode(Sha256::digest(
        format!("{}:{}:{}", perf_path, total, ip_counts.len()).as_bytes(),
    ));
    let summary_pairs = vec![
        ("source".into(), perf_path.into()),
        ("arch".into(), arch),
        ("cmdline".into(), cmdline),
        ("events".into(), events.join(",")),
        ("total_samples".into(), total.to_string()),
        ("unique_functions".into(), func_counts.len().to_string()),
        ("unique_ips".into(), ip_counts.len().to_string()),
        ("commitment".into(), commitment),
    ];
    write_shard_privacy(
        out_dir,
        "summary",
        summary_pairs,
        vec!["perf", "da51", "summary"],
        private,
    )?;

    // 2. Event count shards
    for (event, count) in &event_counts {
        let pairs = vec![
            ("event".into(), event.clone()),
            ("count".into(), count.to_string()),
        ];
        let id = format!("event_{}", event.replace('/', "_").replace(':', "_"));
        write_shard_privacy(out_dir, &id, pairs, vec!["perf", "da51", "event"], private)?;
    }

    // 3. Function/record-type shards (all of them)
    let mut ranked_funcs: Vec<_> = func_counts.iter().collect();
    ranked_funcs.sort_by(|a, b| b.1.cmp(a.1));
    for (i, (sym, count)) in ranked_funcs.iter().enumerate() {
        let pairs = vec![
            ("rank".into(), i.to_string()),
            ("record_type".into(), sym.to_string()),
            ("samples".into(), count.to_string()),
            (
                "pct".into(),
                format!("{:.4}", **count as f64 / total.max(1) as f64 * 100.0),
            ),
        ];
        write_shard_privacy(
            out_dir,
            &format!("func_{}", i),
            pairs,
            vec!["perf", "da51", "function"],
            private,
        )?;
    }

    // 4. Instruction address shards (top 200)
    let mut ranked_ips: Vec<_> = ip_counts.iter().collect();
    ranked_ips.sort_by(|a, b| b.1.cmp(a.1));
    for (i, (ip, count)) in ranked_ips.iter().take(200).enumerate() {
        let pairs = vec![
            ("rank".into(), i.to_string()),
            ("ip".into(), format!("0x{:x}", ip)),
            ("samples".into(), count.to_string()),
        ];
        write_shard_privacy(
            out_dir,
            &format!("instr_{}", i),
            pairs,
            vec!["perf", "da51", "instruction"],
            private,
        )?;
    }

    // 5. Timestamp trace shard (raw sample stream, first 10000)
    let trace_pairs: Vec<(String, String)> = samples
        .iter()
        .take(10000)
        .enumerate()
        .map(|(i, s)| (i.to_string(), format!("{}:0x{:x}:{}", s.ts, s.ip, s.event)))
        .collect();
    if !trace_pairs.is_empty() {
        write_shard_privacy(
            out_dir,
            "trace",
            trace_pairs,
            vec!["perf", "da51", "trace"],
            private,
        )?;
    }

    let n_shards = 1 + event_counts.len() + ranked_funcs.len() + ranked_ips.len().min(200) + 1;
    let mode = if private {
        " (PRIVATE — sensitive fields redacted)"
    } else {
        ""
    };
    eprintln!(
        "{}: {} samples, {} functions, {} IPs, {} events → {} DA51 shards{}",
        perf_path,
        total,
        func_counts.len(),
        ip_counts.len(),
        event_counts.len(),
        n_shards,
        mode
    );
    Ok(())
}

struct SampleRecord {
    ts: u64,
    ip: u64,
    event: String,
}

fn write_shard(dir: &str, id: &str, pairs: Vec<(String, String)>, tags: Vec<&str>) -> Result<()> {
    write_shard_privacy(dir, id, pairs, tags, false)
}

fn write_shard_privacy(
    dir: &str,
    id: &str,
    pairs: Vec<(String, String)>,
    tags: Vec<&str>,
    private: bool,
) -> Result<()> {
    let tag_strings: Vec<String> = tags.into_iter().map(|s| s.to_string()).collect();
    if private {
        use erdfa_publish::privacy::PrivacyShard;
        let mut ps = PrivacyShard::from_pairs(id, &pairs, tag_strings);
        ps.redact(&PRIVATE_FIELDS);
        fs::write(format!("{}/{}.priv.cbor", dir, id), ps.to_cbor())?;
    } else {
        let shard = Shard::new(id, Component::KeyValue { pairs }).with_tags(tag_strings);
        fs::write(format!("{}/{}.cbor", dir, id), shard.to_cbor())?;
    }
    Ok(())
}

fn cmd_agda(shard_dir: &str, out_path: &str, module: &str) -> Result<()> {
    let mut shards: Vec<PathBuf> = fs::read_dir(shard_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "cbor").unwrap_or(false))
        .collect();
    shards.sort();

    let mut agda = format!(
        "-- Auto-generated by zkperf-record from {}\nmodule {} where\n\n\
         open import Agda.Builtin.Nat\nopen import Agda.Builtin.List\n\
         open import Agda.Builtin.String\nopen import Agda.Builtin.Bool\n\n\
         data CborVal : Set where\n  cnat  : Nat → CborVal\n  ctext : String → CborVal\n\
         cpair : String → CborVal → CborVal\n  clist : List CborVal → CborVal\n\
         ctag  : Nat → CborVal → CborVal\n\n",
        shard_dir, module
    );

    for (i, path) in shards.iter().enumerate() {
        let raw = fs::read(path)?;
        let data = if raw.len() > 2 && raw[0] == 0xda && raw[1] == 0x51 {
            &raw[2..]
        } else {
            &raw
        };
        if let Ok(val) = ciborium::from_reader::<ciborium::Value, _>(data) {
            agda.push_str(&format!(
                "shard-{} : CborVal\nshard-{} = {}\n\n",
                i,
                i,
                cbor_to_agda(&val)
            ));
        }
    }

    agda.push_str("shards : List CborVal\nshards =\n");
    for i in 0..shards.len() {
        agda.push_str(if i == 0 { "  " } else { "  ∷ " });
        agda.push_str(&format!("shard-{}\n", i));
    }
    agda.push_str(if shards.is_empty() {
        "  []\n"
    } else {
        "  ∷ []\n"
    });

    fs::write(out_path, &agda)?;
    eprintln!("wrote {} ({} shards)", out_path, shards.len());
    Ok(())
}

fn cbor_to_agda(val: &ciborium::Value) -> String {
    use ciborium::Value::*;
    match val {
        Integer(n) => {
            let n: i128 = (*n).into();
            format!("(cnat {})", n.max(0))
        }
        Text(s) => format!(
            "(ctext \"{}\")",
            s.replace('\\', "\\\\").replace('"', "\\\"")
        ),
        Array(xs) if xs.is_empty() => "(clist [])".into(),
        Array(xs) => {
            let items: Vec<String> = xs.iter().map(cbor_to_agda).collect();
            format!("(clist ({} ∷ []))", items.join(" ∷ "))
        }
        Map(kvs) if kvs.is_empty() => "(clist [])".into(),
        Map(kvs) => {
            let items: Vec<String> = kvs
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        Text(s) => s.clone(),
                        _ => format!("{:?}", k),
                    };
                    format!("(cpair \"{}\" {})", key, cbor_to_agda(v))
                })
                .collect();
            format!("(clist ({} ∷ []))", items.join(" ∷ "))
        }
        Tag(t, inner) => format!("(ctag {} {})", t, cbor_to_agda(inner)),
        Bool(b) => format!("(ctext \"{}\")", b),
        Null => "(ctext \"null\")".into(),
        Float(f) => format!("(cnat {})", *f as u64),
        Bytes(bs) if bs.is_empty() => "(clist [])".into(),
        Bytes(bs) => {
            let items: Vec<String> = bs.iter().map(|b| format!("(cnat {})", b)).collect();
            format!("(clist ({} ∷ []))", items.join(" ∷ "))
        }
        _ => "(ctext \"?\")".into(),
    }
}

/// Emit raw per-sample timeline: ts, ip, event — one line per sample
fn cmd_trace(perf_path: &str) -> Result<()> {
    let file = File::open(perf_path)?;
    let reader = BufReader::new(file);
    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)?;

    let events: Vec<String> = perf_file
        .event_attributes()
        .iter()
        .filter_map(|a| a.name().map(|s| s.to_string()))
        .collect();

    println!("ts\tip\tevent");
    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { attr_index, record } = record {
            let event_name = events.get(attr_index).cloned().unwrap_or_default();
            let ts = record.common_data().ok().and_then(|cd| cd.timestamp).unwrap_or(0);
            if let Ok(parsed) = record.parse() {
                use linux_perf_data::linux_perf_event_reader::EventRecord;
                if let EventRecord::Sample(s) = parsed {
                    let ip = s.ip.unwrap_or(0);
                    // Collect register values
                    let mut regs = Vec::new();
                    if let Some(ref ir) = s.intr_regs {
                        for idx in [0,1,2,3,4,5,7,8,9,10,11,12,13,14] {
                            if let Some(v) = ir.get(idx) { regs.push((idx, v)); }
                        }
                    }
                    if regs.is_empty() {
                        println!("{}\t0x{:x}\t{}", ts, ip, event_name);
                    } else {
                        let rv: String = regs.iter().map(|(i,v)| format!("r{}=0x{:x}", i, v)).collect::<Vec<_>>().join(",");
                        println!("{}\t0x{:x}\t{}\t{}", ts, ip, event_name, rv);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Emit register value flow: only when a register value changes
fn cmd_flow(perf_path: &str) -> Result<()> {
    let file = File::open(perf_path)?;
    let reader = BufReader::new(file);
    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)?;

    let reg_names = ["AX","BX","CX","DX","SI","DI","??","R8","R9","R10","R11","R12","R13","R14","R15"];
    let reg_indices: Vec<u64> = vec![0,1,2,3,4,5,7,8,9,10,11,12,13,14];

    let mut prev: HashMap<u64, u64> = HashMap::new(); // reg_idx → last value
    let mut prev_ip: u64 = 0;

    println!("ts\tip\tchanged_regs");
    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { record, .. } = record {
            let ts = record.common_data().ok().and_then(|cd| cd.timestamp).unwrap_or(0);
            if let Ok(parsed) = record.parse() {
                use linux_perf_data::linux_perf_event_reader::EventRecord;
                if let EventRecord::Sample(s) = parsed {
                    let ip = s.ip.unwrap_or(0);
                    if let Some(ref ir) = s.intr_regs {
                        let mut changes = Vec::new();
                        for &idx in &reg_indices {
                            if let Some(v) = ir.get(idx) {
                                let old = prev.get(&idx).copied();
                                if old != Some(v) {
                                    let name = reg_names.get(idx as usize).unwrap_or(&"??");
                                    match old {
                                        Some(ov) => changes.push(format!("{}:0x{:x}→0x{:x}", name, ov, v)),
                                        None => changes.push(format!("{}:=0x{:x}", name, v)),
                                    }
                                    prev.insert(idx, v);
                                }
                            }
                        }
                        if !changes.is_empty() || ip != prev_ip {
                            let ip_change = if ip != prev_ip { format!("ip:0x{:x}→0x{:x}", prev_ip, ip) } else { String::new() };
                            prev_ip = ip;
                            if !changes.is_empty() || !ip_change.is_empty() {
                                let all = if ip_change.is_empty() { changes.join(",") }
                                    else if changes.is_empty() { ip_change }
                                    else { format!("{},{}", ip_change, changes.join(",")) };
                                println!("{}\t0x{:x}\t{}", ts, ip, all);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
