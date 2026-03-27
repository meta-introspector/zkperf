//! SemanticFractran — universal interface for kagenti mesh components
//!
//! Every component (door, service, agent, plugin) implements this trait.
//! State = Gödel number (prime factorization). Actions = prime ratios.
//! Output = witness + eRDFa triple + DA51 CBOR datagram.

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

/// 20 Monster primes — universal semantic dimensions
pub const PRIMES: [(u64, &str); 20] = [
    (2, "position"), (3, "credits"), (5, "crypto"), (7, "network"),
    (11, "count"), (13, "peers"), (17, "turn"), (19, "health"),
    (23, "cargo"), (29, "monitor"), (31, "build"), (37, "deploy"),
    (41, "test"), (43, "render"), (47, "agent"), (53, "stego"),
    (59, "tunnel"), (61, "record"), (67, "shard"), (71, "meta"),
];

/// DA51 CBOR tag
pub const DASL_TAG: u64 = 55889;

/// Prime factorization: vec of (prime, exponent)
pub type Factors = Vec<(u64, u32)>;

/// FRACTRAN ratio: (numerator_factors, denominator_factors)
pub type Ratio = (Factors, Factors);

/// Result of applying an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractranResult {
    pub new_state: Factors,
    pub datagram: String,
    pub triple: (String, String, String),
    pub description: String,
    pub witness_cid: String,
}

/// The universal trait — implement for any component
pub trait SemanticFractran {
    fn name(&self) -> &str;
    fn prime(&self) -> u64;
    fn state(&self) -> Factors;
    fn actions(&self) -> Vec<(&str, Ratio)>;
    fn apply(&mut self, action: &str) -> FractranResult;
    fn describe(&self) -> String;
}

/// Encode factors as string: "2^3.5^1.71^2"
pub fn factors_str(f: &Factors) -> String {
    f.iter().map(|(p, e)| format!("{}^{}", p, e)).collect::<Vec<_>>().join(".")
}

/// Compute Gödel number from factors
pub fn godel(f: &Factors) -> u128 {
    f.iter().fold(1u128, |acc, (p, e)| acc * (*p as u128).pow(*e))
}

/// Decode Gödel number to factors
pub fn decode(mut n: u128) -> Factors {
    let mut f = Vec::new();
    for &(p, _) in &PRIMES {
        let mut e = 0u32;
        while n % (p as u128) == 0 { n /= p as u128; e += 1; }
        if e > 0 { f.push((p, e)); }
    }
    f
}

/// Generate SF datagram
pub fn datagram(name: &str, action: &str, state: &Factors, triple: &(String, String, String)) -> String {
    let h = hex::encode(&Sha256::digest(format!("{}:{}:{}", name, action, factors_str(state)).as_bytes())[..8]);
    format!("SF|1.0|{}|{}|{}|bafk{}|{}:{}:{}", name, action, factors_str(state), h, triple.0, triple.1, triple.2)
}

/// Generate eRDFa HTML
pub fn erdfa(name: &str, action: &str, result: &str) -> String {
    format!(r#"<div vocab="https://schema.org/" typeof="Action"><meta property="name" content="{}:{}"/><div property="object">{}</div></div>"#,
        name, action, result)
}

/// Encode as DA51 CBOR
pub fn to_cbor(name: &str, state: &Factors, action: &str) -> Vec<u8> {
    let val = serde_json::json!({
        "id": format!("{}-{}", name, action),
        "state": factors_str(state),
        "godel": godel(state).to_string(),
        "action": action,
    });
    let cbor_val = ciborium::Value::serialized(&val).unwrap();
    let tagged = ciborium::Value::Tag(DASL_TAG, Box::new(cbor_val));
    let mut buf = Vec::new();
    ciborium::into_writer(&tagged, &mut buf).unwrap();
    buf
}

/// Helper: build a FractranResult
pub fn result(name: &str, prime: u64, action: &str, new_state: Factors, subj: &str, pred: &str, obj: &str, desc: &str) -> FractranResult {
    let triple = (subj.into(), pred.into(), obj.into());
    let dg = datagram(name, action, &new_state, &triple);
    let h = hex::encode(&Sha256::digest(dg.as_bytes())[..8]);
    FractranResult { new_state, datagram: dg, triple, description: desc.into(), witness_cid: format!("bafk{}", h) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f = vec![(2, 3), (5, 1), (71, 2)];
        let n = godel(&f);
        let d = decode(n);
        assert_eq!(f, d);
    }

    #[test]
    fn datagram_format() {
        let f = vec![(29, 1)];
        let dg = datagram("zkperf", "health", &f, &("zkperf".into(), "responds".into(), "ok".into()));
        assert!(dg.starts_with("SF|1.0|zkperf|health|29^1|"));
    }

    #[test]
    fn cbor_tagged() {
        let f = vec![(71, 1)];
        let cbor = to_cbor("test", &f, "init");
        assert_eq!(cbor[0], 0xd9); // CBOR tag marker
        assert_eq!(cbor[1], 0xda); // DA51 high byte
        assert_eq!(cbor[2], 0x51); // DA51 low byte
    }
}
