//! Unit tests for private helpers. These stay in-crate (not in `tests/`)
//! because they exercise crate-private functions — the idiomatic Rust place
//! for white-box tests.

use super::quote_ident;

#[test]
fn quotes_and_escapes() {
    assert_eq!(quote_ident("order"), "\"order\"");
    assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
}
