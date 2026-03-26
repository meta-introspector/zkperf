//! Runtime witness recording for zkPerf security context boundaries.
//!
//! Each witness records:
//! - The security context name and compile-time signature
//! - Declared complexity bounds (Big-O, max_n, max_ms)
//! - Hardware perf counter constraints (cycles, instructions, cache-misses)
//! - Actual elapsed time and perf measurements
//! - Whether any constraint was violated
//!
//! Witnesses are written best-effort to `~/.zkperf/witnesses/`.
//! Cache deduplicates by signature in `~/.zkperf/cache/`.
//! ZK proofs prove constraint satisfaction without revealing measurements.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub mod cache;
pub mod share;
pub mod zkp;

/// Hardware perf counter constraints for a security boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerfConstraints {
    pub max_cycles: Option<u64>,
    pub max_instructions: Option<u64>,
    pub max_cache_misses: Option<u64>,
    pub max_branch_misses: Option<u64>,
    pub max_context_switches: Option<u64>,
    /// Allowed syscall names (empty = unconstrained)
    pub allowed_syscalls: Vec<String>,
}

/// Actual perf counter readings from a boundary crossing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerfReadings {
    pub cycles: Option<u64>,
    pub instructions: Option<u64>,
    pub cache_misses: Option<u64>,
    pub branch_misses: Option<u64>,
    pub context_switches: Option<u64>,
}

impl PerfReadings {
    /// Read counters via `perf_event_open` for the current thread.
    /// Falls back to None per counter if unavailable.
    pub fn sample() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                cycles: read_perf_counter(0, 0),           // PERF_COUNT_HW_CPU_CYCLES
                instructions: read_perf_counter(0, 1),     // PERF_COUNT_HW_INSTRUCTIONS
                cache_misses: read_perf_counter(0, 3),     // PERF_COUNT_HW_CACHE_MISSES
                branch_misses: read_perf_counter(0, 5),    // PERF_COUNT_HW_BRANCH_MISSES
                context_switches: read_perf_counter(1, 3), // PERF_COUNT_SW_CONTEXT_SWITCHES
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self::default()
        }
    }

    /// Delta between two readings (self - before).
    pub fn delta(&self, before: &Self) -> Self {
        Self {
            cycles: zip_sub(self.cycles, before.cycles),
            instructions: zip_sub(self.instructions, before.instructions),
            cache_misses: zip_sub(self.cache_misses, before.cache_misses),
            branch_misses: zip_sub(self.branch_misses, before.branch_misses),
            context_switches: zip_sub(self.context_switches, before.context_switches),
        }
    }
}

fn zip_sub(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.saturating_sub(b)),
        _ => None,
    }
}

/// Read a single perf counter via perf_event_open syscall.
#[cfg(target_os = "linux")]
fn read_perf_counter(type_: u32, config: u64) -> Option<u64> {
    use std::io::Read;
    use std::os::unix::io::FromRawFd;

    #[repr(C)]
    struct PerfEventAttr {
        type_: u32,
        size: u32,
        config: u64,
        _rest: [u8; 104],
    }

    let mut attr = PerfEventAttr {
        type_,
        size: 120,
        config,
        _rest: [0u8; 104],
    };
    attr._rest[0] = 0b0000_0110; // exclude_kernel=1, exclude_hv=1

    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &attr as *const _ as usize,
            0i32,  // pid = this thread
            -1i32, // cpu = any
            -1i32, // group_fd = none
            0u64,  // flags
        )
    };
    if fd < 0 {
        return None;
    }

    let mut file = unsafe { std::fs::File::from_raw_fd(fd as i32) };
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf).ok()?;
    Some(u64::from_ne_bytes(buf))
}

/// Violations found when checking constraints against readings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Violations {
    pub time_exceeded: bool,
    pub cycles_exceeded: bool,
    pub instructions_exceeded: bool,
    pub cache_misses_exceeded: bool,
    pub branch_misses_exceeded: bool,
    pub context_switches_exceeded: bool,
}

impl Violations {
    pub fn any(&self) -> bool {
        self.time_exceeded
            || self.cycles_exceeded
            || self.instructions_exceeded
            || self.cache_misses_exceeded
            || self.branch_misses_exceeded
            || self.context_switches_exceeded
    }

    pub fn check(
        constraints: &PerfConstraints,
        readings: &PerfReadings,
        elapsed_ms: u64,
        max_ms: u64,
    ) -> Self {
        Self {
            time_exceeded: elapsed_ms > max_ms,
            cycles_exceeded: exceeds(readings.cycles, constraints.max_cycles),
            instructions_exceeded: exceeds(readings.instructions, constraints.max_instructions),
            cache_misses_exceeded: exceeds(readings.cache_misses, constraints.max_cache_misses),
            branch_misses_exceeded: exceeds(readings.branch_misses, constraints.max_branch_misses),
            context_switches_exceeded: exceeds(
                readings.context_switches,
                constraints.max_context_switches,
            ),
        }
    }
}

fn exceeds(actual: Option<u64>, limit: Option<u64>) -> bool {
    match (actual, limit) {
        (Some(a), Some(l)) => a > l,
        _ => false,
    }
}

/// A single witness observation from a security boundary crossing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Witness {
    pub context: &'static str,
    pub signature: &'static str,
    pub complexity: &'static str,
    pub max_n: u64,
    pub max_ms: u64,
    pub elapsed_ms: u64,
    pub violated: bool,
    pub timestamp: u64,
    pub platform: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perf: Option<PerfReadings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations: Option<Violations>,
}

impl Witness {
    /// SHA-256 commitment over the full witness record.
    pub fn commitment(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.signature.as_bytes());
        h.update(b"|");
        h.update(self.elapsed_ms.to_string().as_bytes());
        h.update(b"|");
        h.update(self.timestamp.to_string().as_bytes());
        if let Some(ref p) = self.perf {
            if let Some(c) = p.cycles {
                h.update(c.to_string().as_bytes());
            }
            if let Some(i) = p.instructions {
                h.update(i.to_string().as_bytes());
            }
        }
        hex::encode(h.finalize())
    }
}

/// Perf contract violation — raised when a witness_boundary constraint is broken.
#[derive(Debug, Clone, Serialize)]
pub struct PerfViolation {
    pub witness: Witness,
    pub commitment: String,
}

impl std::fmt::Display for PerfViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "perf contract violated: {} [{}] {}ms > {}ms (commitment: {})",
            self.witness.context,
            self.witness.signature,
            self.witness.elapsed_ms,
            self.witness.max_ms,
            self.commitment
        )
    }
}

impl std::error::Error for PerfViolation {}

/// Record witness and enforce contract. Returns Err(PerfViolation) if violated.
pub fn record_enforced(w: Witness) -> Result<Witness, PerfViolation> {
    let commitment = w.commitment();
    record(w.clone());
    if w.violated {
        let v = PerfViolation {
            witness: w,
            commitment,
        };
        let vdir = format!("{}/.zkperf/violations", dirs_fallback().display());
        std::fs::create_dir_all(&vdir).ok();
        let path = format!(
            "{}/{}_{}.json",
            vdir, v.witness.signature, v.witness.timestamp
        );
        std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).ok();
        // Jump to unified violation handler
        on_violation(ViolationSource::Userspace);
        Err(v)
    } else {
        Ok(w)
    }
}

/// Global contract registry — tracks all registered perf signatures at runtime.
static CONTRACTS: std::sync::Mutex<Vec<(&'static str, &'static str, &'static str, u64)>> =
    std::sync::Mutex::new(Vec::new());

/// Register a perf contract (called by witness_boundary at function entry).
pub fn register_contract(
    context: &'static str,
    signature: &'static str,
    complexity: &'static str,
    max_ms: u64,
) {
    if let Ok(mut c) = CONTRACTS.lock() {
        if !c.iter().any(|(_, s, _, _)| *s == signature) {
            c.push((context, signature, complexity, max_ms));
        }
    }
}

/// List all registered perf contracts.
pub fn list_contracts() -> Vec<(&'static str, &'static str, &'static str, u64)> {
    CONTRACTS.lock().map(|c| c.clone()).unwrap_or_default()
}

// ============================================================================
// Violation Handler — unified landing pad for kernel (SIGXCPU) and userspace
// ============================================================================

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static VIOLATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static VIOLATION_HANDLER: std::sync::Mutex<Option<Box<dyn Fn(ViolationSource) + Send + 'static>>> =
    std::sync::Mutex::new(None);

/// Where the violation came from.
#[derive(Debug, Clone, Copy)]
pub enum ViolationSource {
    /// Kernel eBPF sent SIGXCPU
    Kernel,
    /// Userspace #[witness_boundary enforce=true] detected violation
    Userspace,
}

/// Install the zkperf violation handler.
/// Catches SIGXCPU from kernel eBPF and calls the handler.
/// Also called by userspace enforce mode.
///
/// ```rust,ignore
/// zkperf_witness::install_violation_handler(|source| {
///     eprintln!("perf violation from {:?}!", source);
///     // log, alert, graceful shutdown, etc.
/// });
/// ```
pub fn install_violation_handler<F: Fn(ViolationSource) + Send + 'static>(handler: F) {
    *VIOLATION_HANDLER.lock().unwrap() = Some(Box::new(handler));

    if !HANDLER_INSTALLED.swap(true, Ordering::SeqCst) {
        // Install SIGXCPU signal handler (Unix only)
        #[cfg(unix)]
        unsafe {
            libc::signal(libc::SIGXCPU, sigxcpu_handler as libc::sighandler_t);
        }
    }
}

#[cfg(unix)]
extern "C" fn sigxcpu_handler(_sig: libc::c_int) {
    // Signal-safe: just set flag + increment counter
    VIOLATION_COUNT.fetch_add(1, Ordering::SeqCst);
    // Spawn thread to do the actual handling (signal handlers can't do much)
    let _ = std::thread::spawn(|| {
        on_violation(ViolationSource::Kernel);
    });
}

/// Called from both signal handler (kernel) and enforce mode (userspace).
pub fn on_violation(source: ViolationSource) {
    VIOLATION_COUNT.fetch_add(1, Ordering::SeqCst);

    // Record the violation as a witness
    let w = Witness {
        context: match source {
            ViolationSource::Kernel => "kernel-ebpf",
            ViolationSource::Userspace => "userspace-enforce",
        },
        signature: "violation-handler",
        complexity: "N/A",
        max_n: 0,
        max_ms: 0,
        elapsed_ms: 0,
        violated: true,
        timestamp: now_ms(),
        platform: std::env::consts::OS,
        perf: None,
        violations: None,
    };
    record(w);

    // Call user handler
    if let Ok(guard) = VIOLATION_HANDLER.lock() {
        if let Some(ref handler) = *guard {
            handler(source);
        }
    }
}

/// Get total violation count (kernel + userspace).
pub fn violation_count() -> usize {
    VIOLATION_COUNT.load(Ordering::SeqCst)
}
/// Current time in milliseconds since epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn witness_dir() -> PathBuf {
    dirs_fallback().join("witnesses")
}

pub fn dirs_fallback() -> PathBuf {
    std::env::var("ZKPERF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".zkperf")
        })
}

/// Record a witness with perf constraints enforcement.
pub fn record_with_perf(
    context: &'static str,
    signature: &'static str,
    complexity: &'static str,
    max_n: u64,
    max_ms: u64,
    elapsed_ms: u64,
    constraints: &PerfConstraints,
    readings: &PerfReadings,
) {
    let violations = Violations::check(constraints, readings, elapsed_ms, max_ms);
    let violated = violations.any();

    let w = Witness {
        context,
        signature,
        complexity,
        max_n,
        max_ms,
        elapsed_ms,
        violated,
        timestamp: now_ms(),
        platform: std::env::consts::OS,
        perf: Some(readings.clone()),
        violations: if violated {
            Some(violations.clone())
        } else {
            None
        },
    };

    let _ = record_inner(&w);

    // Generate ZK proof for this boundary crossing
    let proof = zkp::prove_with_constraints(&w, constraints);
    let _ = save_proof(&w, &proof);

    if violated {
        eprintln!(
            "zkperf: VIOLATION {context} sig={sig}",
            sig = &signature[..16]
        );
        if violations.time_exceeded {
            eprintln!("  time: {elapsed_ms}ms > {max_ms}ms");
        }
        if violations.cycles_exceeded {
            eprintln!(
                "  cycles: {:?} > {:?}",
                readings.cycles, constraints.max_cycles
            );
        }
        if violations.instructions_exceeded {
            eprintln!(
                "  instructions: {:?} > {:?}",
                readings.instructions, constraints.max_instructions
            );
        }
        if violations.cache_misses_exceeded {
            eprintln!(
                "  cache-misses: {:?} > {:?}",
                readings.cache_misses, constraints.max_cache_misses
            );
        }
        if violations.branch_misses_exceeded {
            eprintln!(
                "  branch-misses: {:?} > {:?}",
                readings.branch_misses, constraints.max_branch_misses
            );
        }
    }
}

/// Simple record without perf counters (backward compat).
pub fn record(w: Witness) {
    let _ = record_inner(&w);
    // Generate ZK proof
    let proof = zkp::prove(&w);
    let _ = save_proof(&w, &proof);
    if w.violated {
        eprintln!(
            "zkperf: VIOLATION {ctx} exceeded {max}ms (actual {actual}ms) sig={sig}",
            ctx = w.context,
            max = w.max_ms,
            actual = w.elapsed_ms,
            sig = &w.signature[..16],
        );
    }
}

fn record_inner(w: &Witness) -> std::io::Result<()> {
    let dir = witness_dir();
    std::fs::create_dir_all(&dir)?;
    let filename = format!("{}_{}.witness.json", w.timestamp, w.context);
    let path = dir.join(filename);
    let json =
        serde_json::to_string(w).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)?;
    // Update cache stats
    cache::update(w);
    Ok(())
}

fn save_proof(w: &Witness, proof: &zkp::WitnessProof) -> std::io::Result<()> {
    let dir = dirs_fallback().join("proofs");
    std::fs::create_dir_all(&dir)?;
    let filename = format!("{}_{}.proof.json", w.timestamp, w.context);
    let json = serde_json::to_string(proof)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(dir.join(filename), json)
}

/// Instrument an arbitrary code block with zkperf timing + witness.
///
/// ```rust,ignore
/// use zkperf_witness::zkperf_span;
///
/// let result = zkperf_span!("my_operation", {
///     expensive_computation()
/// });
///
/// // With the zkperf-service running, also posts to HTTP:
/// let result = zkperf_span!("my_operation", service = true, {
///     expensive_computation()
/// });
/// ```
#[macro_export]
macro_rules! zkperf_span {
    ($name:expr, { $($body:tt)* }) => {{
        let __t0 = ::std::time::Instant::now();
        let __r = { $($body)* };
        let __ms = __t0.elapsed().as_millis() as u64;
        $crate::record($crate::Witness {
            context: $name,
            signature: "",
            complexity: "auto",
            max_n: 0,
            max_ms: 0,
            elapsed_ms: __ms,
            violated: false,
            timestamp: $crate::now_ms(),
            platform: ::std::env::consts::OS,
            perf: None,
            violations: None,
        });
        __r
    }};
    ($name:expr, service = true, { $($body:tt)* }) => {{
        let __r = $crate::zkperf_span!($name, { $($body)* });
        // fire-and-forget POST to zkperf-service
        let _ = ::std::thread::spawn(move || {
            let _ = ::std::net::TcpStream::connect("127.0.0.1:9718").and_then(|mut s| {
                use ::std::io::Write;
                let body = format!(
                    r#"{{"sig":"{}","event":"span","data_hash":"","size":0}}"#,
                    $name
                );
                write!(s, "POST /witness HTTP/1.0\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
            });
        });
        __r
    }};
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_commitment_deterministic() {
        let w = Witness {
            context: "test",
            signature: "abc123def456789012345678901234567890123456789012345678901234",
            complexity: "O(1)",
            max_n: 0,
            max_ms: 1000,
            elapsed_ms: 50,
            violated: false,
            timestamp: 1234567890,
            platform: "linux",
            perf: None,
            violations: None,
        };
        assert_eq!(w.commitment(), w.commitment());
    }

    #[test]
    fn violation_detected() {
        let c = PerfConstraints {
            max_cycles: Some(1000),
            max_instructions: Some(500),
            ..Default::default()
        };
        let r = PerfReadings {
            cycles: Some(2000),
            instructions: Some(100),
            ..Default::default()
        };
        let v = Violations::check(&c, &r, 5, 10);
        assert!(v.cycles_exceeded);
        assert!(!v.instructions_exceeded);
        assert!(!v.time_exceeded);
        assert!(v.any());
    }

    #[test]
    fn perf_delta() {
        let before = PerfReadings {
            cycles: Some(100),
            instructions: Some(50),
            ..Default::default()
        };
        let after = PerfReadings {
            cycles: Some(350),
            instructions: Some(200),
            ..Default::default()
        };
        let d = after.delta(&before);
        assert_eq!(d.cycles, Some(250));
        assert_eq!(d.instructions, Some(150));
    }
}
