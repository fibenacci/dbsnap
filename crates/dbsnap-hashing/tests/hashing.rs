//! Behavioural tests for the hashing primitives (public API only).

use dbsnap_hashing::{hash_json, DbHash, Hasher};
use serde_json::json;

#[test]
fn hex_roundtrip() {
    let h = hash_json(&json!({"a": 1}));
    let parsed: DbHash = h.to_hex().parse().unwrap();
    assert_eq!(h, parsed);
}

#[test]
fn json_key_order_is_irrelevant() {
    // Object key order must not change the hash (canonical form).
    let a = hash_json(&json!({"a": 1, "b": 2}));
    let b = hash_json(&json!({"b": 2, "a": 1}));
    assert_eq!(a, b);
}

#[test]
fn distinct_values_differ() {
    assert_ne!(hash_json(&json!({"a": 1})), hash_json(&json!({"a": 2})));
}

#[test]
fn domain_separation() {
    // Same bytes, different tag => different hash.
    let x = Hasher::new("row").update(b"hello").finalize();
    let y = Hasher::new("table").update(b"hello").finalize();
    assert_ne!(x, y);
}

#[test]
fn rejects_malformed_hex() {
    assert!("xyz".parse::<DbHash>().is_err());
    assert!("ab".parse::<DbHash>().is_err()); // too short
}
