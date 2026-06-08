//! Deterministic, domain-separated hashing primitives for dbsnap.
//!
//! Everything in dbsnap that produces a content hash routes through this crate
//! so the algorithm and domain-separation rules live in exactly one place. We
//! use BLAKE3 for speed and a 256-bit output.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Bumped if the on-disk hashing scheme ever changes incompatibly.
pub const DOMAIN: &str = "dbsnap-v1";

const SEP: &[u8] = b"\x1f"; // ASCII unit separator, never appears in identifiers

/// A 256-bit content hash, rendered as lowercase hex in text contexts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DbHash([u8; 32]);

impl DbHash {
    pub fn from_bytes(b: [u8; 32]) -> Self {
        Self(b)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(nibble(b >> 4));
            s.push(nibble(b & 0x0f));
        }
        s
    }

    /// Short prefix for human-facing output (à la `git log --oneline`).
    pub fn short(&self) -> String {
        self.to_hex()[..12].to_string()
    }
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

impl fmt::Display for DbHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for DbHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DbHash({})", self.short())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseHashError(String);

impl fmt::Display for ParseHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid hash: {}", self.0)
    }
}
impl std::error::Error for ParseHashError {}

impl FromStr for DbHash {
    type Err = ParseHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 {
            return Err(ParseHashError(format!(
                "expected 64 hex chars, got {}",
                s.len()
            )));
        }
        let mut out = [0u8; 32];
        let bytes = s.as_bytes();
        for i in 0..32 {
            let hi = unhex(bytes[i * 2]).ok_or_else(|| ParseHashError(s.to_string()))?;
            let lo = unhex(bytes[i * 2 + 1]).ok_or_else(|| ParseHashError(s.to_string()))?;
            out[i] = (hi << 4) | lo;
        }
        Ok(DbHash(out))
    }
}

fn unhex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl Serialize for DbHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for DbHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = DbHash;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a 64-char hex hash string")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<DbHash, E> {
                DbHash::from_str(v).map_err(de::Error::custom)
            }
        }
        d.deserialize_str(V)
    }
}

/// Incremental hasher with mandatory domain separation.
///
/// `Hasher::new(tag)` mixes in the global [`DOMAIN`] and a per-purpose `tag`
/// (e.g. `"row"`, `"table"`, `"commit"`) so that a value hashed for one purpose
/// can never collide with a structurally identical value hashed for another.
pub struct Hasher(blake3::Hasher);

impl Hasher {
    pub fn new(tag: &str) -> Self {
        let mut h = blake3::Hasher::new();
        h.update(DOMAIN.as_bytes());
        h.update(SEP);
        h.update(tag.as_bytes());
        h.update(SEP);
        Hasher(h)
    }

    pub fn update(&mut self, bytes: &[u8]) -> &mut Self {
        self.0.update(bytes);
        self
    }

    /// Length-prefixed string update so that ("ab","c") differs from ("a","bc").
    pub fn update_str(&mut self, s: &str) -> &mut Self {
        self.0.update(&(s.len() as u64).to_le_bytes());
        self.0.update(s.as_bytes());
        self
    }

    pub fn update_hash(&mut self, h: &DbHash) -> &mut Self {
        self.0.update(h.as_bytes());
        self
    }

    pub fn finalize(&self) -> DbHash {
        DbHash(*self.0.finalize().as_bytes())
    }
}

/// Hash an arbitrary JSON value canonically.
///
/// Relies on `serde_json`'s default `BTreeMap`-backed `Map` (object keys sorted)
/// and `arbitrary_precision` (numbers preserved verbatim). The same logical row
/// therefore always serializes to the same bytes regardless of column order.
pub fn hash_json(value: &serde_json::Value) -> DbHash {
    let bytes = serde_json::to_vec(value).expect("serializing serde_json::Value cannot fail");
    let mut h = Hasher::new("row");
    h.update(&bytes);
    h.finalize()
}
