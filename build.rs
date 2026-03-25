//! build.rs — self-instrumenting build that records a witness of its own compilation.
//!
//! Captures: timestamp, rustc version, target, profile, crate count, env vars.
//! Writes witness to OUT_DIR/build-witness.json and prints cargo metadata.

use std::process::Command;
use std::time::Instant;

fn main() {
    let t0 = Instant::now();

    // Gather build metadata
    let rustc_ver = Command::new("rustc").arg("--version").output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    let target = std::env::var("TARGET").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| "/tmp".into());
    let num_jobs = std::env::var("NUM_JOBS").unwrap_or_else(|_| "1".into());
    let host = std::env::var("HOST").unwrap_or_default();

    // Count workspace crates
    let crate_count = std::fs::read_to_string("Cargo.toml")
        .map(|s| s.matches("[").count())
        .unwrap_or(0);

    let elapsed_ms = t0.elapsed().as_millis();

    // Build witness JSON
    let witness = format!(
        r#"{{"event":"build","rustc":"{}","target":"{}","profile":"{}","host":"{}","jobs":{},"crates":{},"build_rs_ms":{},"timestamp":{}}}"#,
        rustc_ver, target, profile, host, num_jobs, crate_count, elapsed_ms,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
    );

    // Write witness
    let witness_path = format!("{}/build-witness.json", out_dir);
    std::fs::write(&witness_path, &witness).ok();

    // Embed as compile-time constant
    println!("cargo:rustc-env=ZKPERF_BUILD_WITNESS={}", witness);
    println!("cargo:rustc-env=ZKPERF_RUSTC={}", rustc_ver);
    println!("cargo:rustc-env=ZKPERF_TARGET={}", target);
    println!("cargo:rustc-env=ZKPERF_PROFILE={}", profile);

    // Rerun only if Cargo.toml changes
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
}
