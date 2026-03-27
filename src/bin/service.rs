//! zkperf-service — perf monitoring daemon for kagenti service mesh
//!
//! API (HTTP on port 9718):
//!   POST /attach       {"pid": 12345}              — start perf record on PID
//!   POST /detach       {"pid": 12345}              — stop perf record, parse data
//!   POST /boundary     {"pid": 12345, "sig": "...", "event": "start"|"end"}
//!   GET  /witnesses                                — list all witnesses
//!   GET  /witnesses/:sig                           — get witness by signature
//!   GET  /health                                   — health check

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

struct PerfSession {
    child: Child,
    pid: u32,
    data_path: String,
    boundaries: Vec<Boundary>,
}

struct Boundary {
    sig: String,
    start_ns: u64,
    end_ns: Option<u64>,
}

struct State {
    sessions: HashMap<u32, PerfSession>,
    data_dir: String,
}

fn main() {
    eprintln!("zkperf-service build: {}", env!("ZKPERF_BUILD_WITNESS"));

    let data_dir = format!(
        "{}/.zkperf/service",
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
    );
    std::fs::create_dir_all(&data_dir).ok();

    // Write build witness to data dir
    let _ = std::fs::write(
        format!("{}/build-witness.json", data_dir),
        env!("ZKPERF_BUILD_WITNESS"),
    );

    let state = Arc::new(Mutex::new(State {
        sessions: HashMap::new(),
        data_dir,
    }));

    let listener = TcpListener::bind("127.0.0.1:9718").expect("bind :9718");
    eprintln!("zkperf-service listening on 127.0.0.1:9718");

    for stream in listener.incoming().flatten() {
        let state = Arc::clone(&state);
        std::thread::spawn(move || handle(stream, state));
    }
}

fn handle(mut stream: std::net::TcpStream, state: Arc<Mutex<State>>) {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);

    let (method, path, body) = parse_req(&req);
    let (status, resp) = route(&method, &path, &body, &state);

    let ct = if path == "/metrics" { "text/plain" } else { "application/json" };
    let out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\n\r\n{resp}",
        resp.len()
    );
    stream.write_all(out.as_bytes()).ok();
}

fn parse_req(raw: &str) -> (String, String, String) {
    let mut lines = raw.lines();
    let first = lines.next().unwrap_or("");
    let parts: Vec<&str> = first.split_whitespace().collect();
    let method = parts.first().unwrap_or(&"GET").to_string();
    let path = parts.get(1).unwrap_or(&"/").to_string();
    // body after blank line
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (method, path, body)
}

fn route(method: &str, path: &str, body: &str, state: &Arc<Mutex<State>>) -> (String, String) {
    let t0 = std::time::Instant::now();
    let (status, resp) = match (method, path) {
        ("GET", "/health") => ("200 OK".into(), r#"{"status":"ok"}"#.into()),
        ("GET", "/build") => ("200 OK".into(), env!("ZKPERF_BUILD_WITNESS").into()),
        ("GET", "/metrics") => cmd_metrics(),
        ("GET", "/contracts") => cmd_list_contracts(),
        ("GET", "/violations") => cmd_list_violations(),
        ("POST", "/attach") => cmd_attach(body, state),
        ("POST", "/detach") => cmd_detach(body, state),
        ("POST", "/boundary") => cmd_boundary(body, state),
        ("POST", "/witness") => cmd_record_witness(body, state),
        ("GET", "/witnesses") => cmd_list_witnesses(state),
        _ if path.starts_with("/witnesses/") => {
            let sig = &path["/witnesses/".len()..];
            cmd_get_witness(sig, state)
        }
        _ => ("404 Not Found".into(), r#"{"error":"not found"}"#.into()),
    };
    let ms = t0.elapsed().as_millis();
    if ms > 0 {
        eprintln!("{} {} → {} ({}ms)", method, path, &status[..3], ms);
    }
    (status, resp)
}

fn cmd_attach(body: &str, state: &Arc<Mutex<State>>) -> (String, String) {
    let pid: u32 = extract_u64(body, "pid") as u32;
    if pid == 0 {
        return (
            "400 Bad Request".into(),
            r#"{"error":"pid required"}"#.into(),
        );
    }

    let mut st = state.lock().unwrap();
    if st.sessions.contains_key(&pid) {
        return (
            "409 Conflict".into(),
            r#"{"error":"already attached"}"#.into(),
        );
    }

    let data_path = format!("{}/perf-{}.data", st.data_dir, pid);
    let child = match Command::new("perf")
        .args([
            "record",
            "-g",
            "--call-graph",
            "dwarf,65528",
            "-e",
            "cycles:u,instructions:u,cache-misses:u,branch-misses:u",
            "-p",
            &pid.to_string(),
            "-o",
            &data_path,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                "500 Internal Server Error".into(),
                format!(r#"{{"error":"{}"}}"#, e),
            )
        }
    };

    st.sessions.insert(
        pid,
        PerfSession {
            child,
            pid,
            data_path: data_path.clone(),
            boundaries: Vec::new(),
        },
    );

    (
        "200 OK".into(),
        format!(r#"{{"pid":{},"data":"{}"}}"#, pid, data_path),
    )
}

fn cmd_detach(body: &str, state: &Arc<Mutex<State>>) -> (String, String) {
    let pid = extract_u64(body, "pid") as u32;
    let mut st = state.lock().unwrap();

    if let Some(mut session) = st.sessions.remove(&pid) {
        // SIGINT to perf
        unsafe {
            libc::kill(session.child.id() as i32, libc::SIGINT);
        }
        session.child.wait().ok();

        let n_boundaries = session.boundaries.len();
        // Generate witnesses for completed boundaries
        let witnesses: Vec<String> = session
            .boundaries
            .iter()
            .filter(|b| b.end_ns.is_some())
            .map(|b| {
                let elapsed_ms = (b.end_ns.unwrap() - b.start_ns) / 1_000_000;
                let w = zkperf_witness::Witness {
                    context: "perf-service",
                    signature: "zkperf-service-boundary",
                    complexity: "measured",
                    max_n: 0,
                    max_ms: elapsed_ms,
                    elapsed_ms,
                    violated: false,
                    timestamp: b.start_ns / 1_000_000_000,
                    platform: std::env::consts::OS,
                    perf: None,
                    violations: None,
                };
                zkperf_witness::record(w);
                b.sig.clone()
            })
            .collect();

        (
            "200 OK".into(),
            format!(
                r#"{{"pid":{},"data":"{}","boundaries":{},"witnesses":{}}}"#,
                pid,
                session.data_path,
                n_boundaries,
                serde_json::to_string(&witnesses).unwrap()
            ),
        )
    } else {
        (
            "404 Not Found".into(),
            r#"{"error":"pid not attached"}"#.into(),
        )
    }
}

fn cmd_boundary(body: &str, state: &Arc<Mutex<State>>) -> (String, String) {
    let pid = extract_u64(body, "pid") as u32;
    let sig = extract_str(body, "sig");
    let event = extract_str(body, "event");
    let ts = now_ns();

    let mut st = state.lock().unwrap();
    if let Some(session) = st.sessions.get_mut(&pid) {
        match event.as_str() {
            "start" => {
                session.boundaries.push(Boundary {
                    sig: sig.clone(),
                    start_ns: ts,
                    end_ns: None,
                });
                (
                    "200 OK".into(),
                    format!(r#"{{"sig":"{}","start_ns":{}}}"#, sig, ts),
                )
            }
            "end" => {
                if let Some(b) = session
                    .boundaries
                    .iter_mut()
                    .rev()
                    .find(|b| b.sig == sig && b.end_ns.is_none())
                {
                    b.end_ns = Some(ts);
                    let elapsed_ms = (ts - b.start_ns) / 1_000_000;
                    (
                        "200 OK".into(),
                        format!(r#"{{"sig":"{}","elapsed_ms":{}}}"#, sig, elapsed_ms),
                    )
                } else {
                    (
                        "404 Not Found".into(),
                        r#"{"error":"no open boundary"}"#.into(),
                    )
                }
            }
            _ => (
                "400 Bad Request".into(),
                r#"{"error":"event must be start|end"}"#.into(),
            ),
        }
    } else {
        (
            "404 Not Found".into(),
            r#"{"error":"pid not attached"}"#.into(),
        )
    }
}

/// POST /witness — record a witness directly without perf attachment
/// Body: {"sig": "stego-exchange", "event": "meme-upload", "data_hash": "sha256hex", "size": 316}
fn cmd_record_witness(body: &str, _state: &Arc<Mutex<State>>) -> (String, String) {
    let v: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return ("400 Bad Request".into(), r#"{"error":"bad json"}"#.into()),
    };
    let sig = v["sig"].as_str().unwrap_or("unknown");
    let event = v["event"].as_str().unwrap_or("event");
    let data_hash = v["data_hash"].as_str().unwrap_or("");
    let size = v["size"].as_u64().unwrap_or(0);

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let witness = serde_json::json!({
        "sig": sig,
        "event": event,
        "data_hash": data_hash,
        "size": size,
        "timestamp": ts,
        "source": "direct",
    });

    let witness_dir = format!(
        "{}/.zkperf/witnesses",
        std::env::var("HOME").unwrap_or_default()
    );
    std::fs::create_dir_all(&witness_dir).ok();
    let path = format!("{}/{}_{}.json", witness_dir, sig, ts);
    std::fs::write(&path, serde_json::to_string_pretty(&witness).unwrap()).ok();

    ("200 OK".into(), serde_json::to_string(&witness).unwrap())
}


fn cmd_metrics() -> (String, String) {
    let home = std::env::var("HOME").unwrap_or_default();
    let w_count = std::fs::read_dir(format!("{}/.zkperf/witnesses", home))
        .map(|d| d.count()).unwrap_or(0);
    let v_count = std::fs::read_dir(format!("{}/.zkperf/violations", home))
        .map(|d| d.count()).unwrap_or(0);
    let c_count = std::fs::read_dir(format!("{}/.zkperf/shards", home))
        .map(|d| d.count()).unwrap_or(0);

    let body = format!(
        "# HELP zkperf_witnesses_total Total witness records\n\
         # TYPE zkperf_witnesses_total gauge\n\
         zkperf_witnesses_total {}\n\
         # HELP zkperf_violations_total Total contract violations\n\
         # TYPE zkperf_violations_total gauge\n\
         zkperf_violations_total {}\n\
         # HELP zkperf_shards_total Total shard projects\n\
         # TYPE zkperf_shards_total gauge\n\
         zkperf_shards_total {}\n\
         # HELP zkperf_up Service is up\n\
         # TYPE zkperf_up gauge\n\
         zkperf_up 1\n",
        w_count, v_count, c_count
    );
    ("200 OK".into(), body)
}
fn cmd_list_contracts() -> (String, String) {
    let witness_dir = format!(
        "{}/.zkperf/witnesses",
        std::env::var("HOME").unwrap_or_default()
    );
    // Scan witnesses to extract unique contracts (sig → latest witness)
    let mut contracts: HashMap<String, serde_json::Value> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&witness_dir) {
        for e in entries.flatten() {
            if let Ok(data) = std::fs::read_to_string(e.path()) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(sig) = v["signature"].as_str() {
                        contracts.insert(sig.to_string(), v);
                    }
                }
            }
        }
    }
    let list: Vec<_> = contracts.values().collect();
    ("200 OK".into(), serde_json::to_string(&list).unwrap())
}

fn cmd_list_violations() -> (String, String) {
    let vdir = format!(
        "{}/.zkperf/violations",
        std::env::var("HOME").unwrap_or_default()
    );
    let entries: Vec<serde_json::Value> = std::fs::read_dir(&vdir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect();
    ("200 OK".into(), serde_json::to_string(&entries).unwrap())
}

fn cmd_list_witnesses(state: &Arc<Mutex<State>>) -> (String, String) {
    let st = state.lock().unwrap();
    let witness_dir = format!(
        "{}/.zkperf/witnesses",
        std::env::var("HOME").unwrap_or_default()
    );
    let entries: Vec<String> = std::fs::read_dir(&witness_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    ("200 OK".into(), serde_json::to_string(&entries).unwrap())
}

fn cmd_get_witness(sig: &str, _state: &Arc<Mutex<State>>) -> (String, String) {
    let path = format!(
        "{}/.zkperf/witnesses/{}.json",
        std::env::var("HOME").unwrap_or_default(),
        sig
    );
    match std::fs::read_to_string(&path) {
        Ok(data) => ("200 OK".into(), data),
        Err(_) => (
            "404 Not Found".into(),
            r#"{"error":"witness not found"}"#.into(),
        ),
    }
}

// Minimal JSON field extraction (no serde dependency for the body parsing)
fn extract_u64(json: &str, key: &str) -> u64 {
    let pat = format!(r#""{}""#, key);
    json.find(&pat)
        .and_then(|i| {
            let rest = &json[i + pat.len()..];
            let start = rest.find(|c: char| c.is_ascii_digit())?;
            let end = rest[start..]
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len() - start);
            rest[start..start + end].parse().ok()
        })
        .unwrap_or(0)
}

fn extract_str(json: &str, key: &str) -> String {
    let pat = format!(r#""{}""#, key);
    json.find(&pat)
        .and_then(|i| {
            let rest = &json[i + pat.len()..];
            let start = rest.find('"')? + 1;
            let end = rest[start..].find('"')?;
            Some(rest[start..start + end].to_string())
        })
        .unwrap_or_default()
}
