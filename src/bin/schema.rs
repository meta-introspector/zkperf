//! zkperf-schema — extract linux-perf-data type structure as DA51 metadata,
//! then use it as a dynamic schema to generate typed CBOR shard instances.
//!
//! Two modes:
//!   schema  — emit DA51 CBOR schema from the crate's type definitions
//!   extract <perf.data> <out-dir> — use schema to extract typed instances

use anyhow::Result;
use erdfa_publish::{Component, Shard};
use linux_perf_data::{AttributeDescription, PerfFileReader, PerfFileRecord};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::BufReader;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage:");
        eprintln!("  zkperf-schema schema [out-dir]");
        eprintln!("  zkperf-schema extract <perf.data> <out-dir>");
        std::process::exit(1);
    }
    match args[0].as_str() {
        "schema" => {
            let dir = args
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or("/tmp/perf_schema");
            cmd_schema(dir)?;
        }
        "extract" => cmd_extract(&args[1], &args[2])?,
        _ => eprintln!("unknown: {}", args[0]),
    }
    Ok(())
}

/// Emit the perf type hierarchy as DA51 CBOR shards — one shard per type
fn cmd_schema(out_dir: &str) -> Result<()> {
    fs::create_dir_all(out_dir)?;

    // Schema: each type is a shard with its fields as key-value pairs
    // This is the DA51 reflection of linux-perf-data's own structure
    let types: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "SampleRecord",
            vec![
                ("ip", "Option<u64>"),
                ("timestamp", "Option<u64>"),
                ("pid", "Option<i32>"),
                ("tid", "Option<i32>"),
                ("cpu", "Option<u32>"),
                ("period", "Option<u64>"),
                ("addr", "Option<u64>"),
                ("phys_addr", "Option<u64>"),
                ("user_regs", "Option<Regs>"),
                ("intr_regs", "Option<Regs>"),
                ("callchain", "Option<RawDataU64>"),
                ("user_stack", "Option<(RawData, u64)>"),
                ("data_page_size", "Option<u64>"),
                ("code_page_size", "Option<u64>"),
                ("cpu_mode", "CpuMode"),
            ],
        ),
        (
            "EventRecord",
            vec![
                ("Sample", "SampleRecord"),
                ("Mmap", "MmapRecord"),
                ("Mmap2", "Mmap2Record"),
                ("Comm", "CommOrExecRecord"),
                ("Exit", "ForkOrExitRecord"),
                ("Fork", "ForkOrExitRecord"),
                ("Lost", "LostRecord"),
                ("Throttle", "ThrottleRecord"),
                ("ContextSwitch", "ContextSwitchRecord"),
            ],
        ),
        (
            "CommonData",
            vec![
                ("pid", "Option<i32>"),
                ("tid", "Option<i32>"),
                ("timestamp", "Option<u64>"),
                ("cpu", "Option<u32>"),
            ],
        ),
        (
            "MmapRecord",
            vec![
                ("pid", "i32"),
                ("tid", "i32"),
                ("address", "u64"),
                ("length", "u64"),
                ("page_offset", "u64"),
                ("path", "&[u8]"),
                ("cpu_mode", "CpuMode"),
            ],
        ),
        (
            "Mmap2Record",
            vec![
                ("pid", "i32"),
                ("tid", "i32"),
                ("address", "u64"),
                ("length", "u64"),
                ("page_offset", "u64"),
                ("path", "&[u8]"),
                ("file_id", "Mmap2FileId"),
                ("cpu_mode", "CpuMode"),
            ],
        ),
        (
            "ForkOrExitRecord",
            vec![
                ("pid", "i32"),
                ("ppid", "i32"),
                ("tid", "i32"),
                ("ptid", "i32"),
                ("timestamp", "u64"),
            ],
        ),
        ("LostRecord", vec![("id", "u64"), ("count", "u64")]),
        (
            "PerfEventAttr",
            vec![
                ("type_", "u32"),
                ("size", "u32"),
                ("config", "u64"),
                ("sample_format", "SampleFormat"),
                ("read_format", "ReadFormat"),
                ("flags", "AttrFlags"),
            ],
        ),
        (
            "AttributeDescription",
            vec![
                ("attr", "PerfEventAttr"),
                ("name", "Option<String>"),
                ("event_ids", "Vec<u64>"),
            ],
        ),
        (
            "PerfHeader",
            vec![
                ("data_offset", "u64"),
                ("data_size", "u64"),
                ("attr_size", "u64"),
            ],
        ),
    ];

    for (name, fields) in &types {
        let pairs: Vec<(String, String)> = fields
            .iter()
            .map(|(f, t)| (f.to_string(), t.to_string()))
            .collect();
        let hash = hex::encode(Sha256::digest(format!("{:?}", pairs).as_bytes()));
        let mut all_pairs = vec![
            ("type_name".into(), name.to_string()),
            ("field_count".into(), fields.len().to_string()),
            ("schema_hash".into(), hash[..16].to_string()),
        ];
        all_pairs.extend(pairs);
        let shard = Shard::new(
            format!("schema_{}", name),
            Component::KeyValue { pairs: all_pairs },
        )
        .with_tags(vec![
            "da51".into(),
            "schema".into(),
            "perf".into(),
            name.to_string(),
        ]);
        fs::write(format!("{}/schema_{}.cbor", out_dir, name), shard.to_cbor())?;
    }

    eprintln!("wrote {} schema shards to {}", types.len(), out_dir);
    Ok(())
}

/// Read perf.data and emit typed instances matching the schema
fn cmd_extract(perf_path: &str, out_dir: &str) -> Result<()> {
    fs::create_dir_all(out_dir)?;

    let file = File::open(perf_path)?;
    let reader = BufReader::new(file);
    let PerfFileReader {
        mut perf_file,
        mut record_iter,
    } = PerfFileReader::parse_file(reader)?;

    let events: Vec<String> = perf_file
        .event_attributes()
        .iter()
        .filter_map(AttributeDescription::name)
        .map(|s| s.to_string())
        .collect();

    // Emit schema shards first
    cmd_schema(out_dir)?;

    let mut sample_idx = 0usize;
    let mut mmap_idx = 0usize;
    let mut other_idx = 0usize;

    while let Some(record) = record_iter.next_record(&mut perf_file)? {
        if let PerfFileRecord::EventRecord { attr_index, record } = record {
            let event_name = events.get(attr_index).cloned().unwrap_or_default();
            let record_type = format!("{:?}", record.record_type);

            if let Ok(parsed) = record.parse() {
                let debug = format!("{:?}", parsed);

                // Typed extraction based on record variant
                let (id, pairs, tags) = match &debug {
                    s if s.starts_with("Sample") => {
                        sample_idx += 1;
                        let pairs = extract_fields(
                            &debug,
                            &[
                                "ip",
                                "timestamp",
                                "pid",
                                "tid",
                                "cpu",
                                "period",
                                "addr",
                                "phys_addr",
                                "cpu_mode",
                            ],
                        );
                        let mut p = vec![
                            ("_type".into(), "SampleRecord".into()),
                            ("_event".into(), event_name.clone()),
                            ("_idx".into(), sample_idx.to_string()),
                        ];
                        p.extend(pairs);
                        (
                            format!("sample_{}", sample_idx),
                            p,
                            vec!["da51", "instance", "perf", "SampleRecord"],
                        )
                    }
                    s if s.starts_with("Mmap") => {
                        mmap_idx += 1;
                        let pairs = extract_fields(
                            &debug,
                            &["pid", "tid", "address", "length", "page_offset", "path"],
                        );
                        let mut p = vec![
                            ("_type".into(), "MmapRecord".into()),
                            ("_idx".into(), mmap_idx.to_string()),
                        ];
                        p.extend(pairs);
                        (
                            format!("mmap_{}", mmap_idx),
                            p,
                            vec!["da51", "instance", "perf", "MmapRecord"],
                        )
                    }
                    _ => {
                        other_idx += 1;
                        let p = vec![
                            ("_type".into(), record_type.clone()),
                            ("_idx".into(), other_idx.to_string()),
                            ("_debug".into(), debug.chars().take(200).collect()),
                        ];
                        (
                            format!("other_{}", other_idx),
                            p,
                            vec!["da51", "instance", "perf", &record_type],
                        )
                    }
                };

                let shard = Shard::new(&id, Component::KeyValue { pairs })
                    .with_tags(tags.into_iter().map(|s| s.to_string()).collect());
                fs::write(format!("{}/{}.cbor", out_dir, id), shard.to_cbor())?;
            }
        }
    }

    eprintln!(
        "{}: {} samples, {} mmaps, {} other → DA51 shards in {}",
        perf_path, sample_idx, mmap_idx, other_idx, out_dir
    );
    Ok(())
}

/// Extract named fields from a Debug repr string
fn extract_fields(debug: &str, fields: &[&str]) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for field in fields {
        let pattern = format!("{}: ", field);
        if let Some(pos) = debug.find(&pattern) {
            let rest = &debug[pos + pattern.len()..];
            // Find end: next comma at same nesting level, or closing brace/paren
            let mut depth = 0;
            let end = rest
                .find(|c: char| match c {
                    '(' | '[' | '{' => {
                        depth += 1;
                        false
                    }
                    ')' | ']' | '}' => {
                        if depth == 0 {
                            true
                        } else {
                            depth -= 1;
                            false
                        }
                    }
                    ',' if depth == 0 => true,
                    _ => false,
                })
                .unwrap_or(rest.len());
            let val = rest[..end].trim().to_string();
            result.push((field.to_string(), val));
        }
    }
    result
}
