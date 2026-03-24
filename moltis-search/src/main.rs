use serde::Serialize;
use std::{env, fs, io::Write, path::PathBuf, time::Instant};

#[derive(Serialize)]
struct SearchResult { title: String, url: String, snippet: String }

#[derive(Serialize)]
struct WitnessRecord {
    query: String,
    results: Vec<SearchResult>,
    witness: WitnessData,
}

#[derive(Serialize)]
struct WitnessData {
    timestamp: String,
    request_url: String,
    request_headers: Vec<(String, String)>,
    response_status: u16,
    response_headers: Vec<(String, String)>,
    response_body_hash: String,
    response_body_bytes: usize,
    elapsed_ms: u128,
    witness_path: String,
}

fn sha256_hex(data: &[u8]) -> String {
    // Simple SHA-256 via bit manipulation (djb2 + fnv for witness, not crypto-grade)
    // For a real deployment, use ring or sha2 crate
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

fn search_ddg(query: &str, witness_dir: &PathBuf) -> Result<WitnessRecord, Box<dyn std::error::Error>> {
    let encoded: String = query.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        b' ' => "+".to_string(),
        _ => format!("%{:02X}", b),
    }).collect();

    let url = format!("https://html.duckduckgo.com/html/?q={encoded}");
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) moltis-search/0.1";

    let start = Instant::now();
    let resp = ureq::get(&url).set("User-Agent", ua).call()?;
    let status = resp.status();

    // Capture response headers
    let resp_headers: Vec<(String, String)> = resp.headers_names()
        .iter()
        .filter_map(|name| resp.header(name).map(|v| (name.clone(), v.to_string())))
        .collect();

    let body = resp.into_string()?;
    let elapsed = start.elapsed().as_millis();
    let body_hash = sha256_hex(body.as_bytes());

    // Save raw IO streams
    let ts = chrono_stamp();
    let slug: String = query.chars().filter(|c| c.is_alphanumeric() || *c == ' ').take(40).collect::<String>().replace(' ', "_");
    let base = format!("{ts}_{slug}");

    fs::create_dir_all(witness_dir)?;

    // Save raw response body
    let body_path = witness_dir.join(format!("{base}.response.html"));
    fs::write(&body_path, &body)?;

    // Save request metadata
    let req_path = witness_dir.join(format!("{base}.request.txt"));
    let mut req_file = fs::File::create(&req_path)?;
    writeln!(req_file, "GET {url}")?;
    writeln!(req_file, "User-Agent: {ua}")?;
    writeln!(req_file, "---")?;
    writeln!(req_file, "Status: {status}")?;
    for (k, v) in &resp_headers {
        writeln!(req_file, "{k}: {v}")?;
    }

    // Parse results
    let mut results = Vec::new();
    for block in body.split("class=\"result results_links").skip(1).take(10) {
        let url = extract(block, "class=\"result__a\" href=\"", "\"")
            .and_then(extract_ddg_url)
            .unwrap_or_default();
        let title = extract(block, "class=\"result__a\"", "</a>")
            .map(|s| s.split('>').last().unwrap_or(s))
            .unwrap_or_default();
        let snippet = extract(block, "class=\"result__snippet\">", "</")
            .unwrap_or_default();
        if !url.is_empty() {
            results.push(SearchResult {
                title: clean(title), url, snippet: clean(snippet),
            });
        }
    }

    let witness_json_path = witness_dir.join(format!("{base}.witness.json"));

    let record = WitnessRecord {
        query: query.to_string(),
        results,
        witness: WitnessData {
            timestamp: ts.clone(),
            request_url: url,
            request_headers: vec![("User-Agent".into(), ua.into())],
            response_status: status,
            response_headers: resp_headers,
            response_body_hash: body_hash,
            response_body_bytes: body.len(),
            elapsed_ms: elapsed,
            witness_path: witness_json_path.to_string_lossy().to_string(),
        },
    };

    // Save full witness JSON
    let json = serde_json::to_string_pretty(&record)?;
    fs::write(&witness_json_path, &json)?;

    Ok(record)
}

fn chrono_stamp() -> String {
    // Simple timestamp without chrono crate
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

fn extract_ddg_url(raw: &str) -> Option<String> {
    if let Some(i) = raw.find("uddg=") {
        let rest = &raw[i + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        Some(urldecode(&rest[..end]))
    } else if raw.starts_with("//") {
        Some(format!("https:{raw}"))
    } else {
        Some(raw.to_string())
    }
}

fn urldecode(s: &str) -> String {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i+1..i+3], 16) {
                out.push(v); i += 3; continue;
            }
        }
        out.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn extract<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(&s[i..j])
}

fn clean(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&#x27;", "'")
     .replace("<b>", "").replace("</b>", "").trim().to_string()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: moltis-search <query> [witness-dir]");
        eprintln!("  Saves full IO stream to witness-dir (default: ./witnesses/)");
        std::process::exit(1);
    }
    let query = &args[1];
    let witness_dir = PathBuf::from(args.get(2).map(|s| s.as_str()).unwrap_or("witnesses"));

    match search_ddg(query, &witness_dir) {
        Ok(r) => println!("{}", serde_json::to_string_pretty(&r).unwrap()),
        Err(e) => { eprintln!("{e}"); std::process::exit(1); }
    }
}
