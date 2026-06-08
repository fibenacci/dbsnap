//! Unit tests for private SQL-building helpers (white-box, kept in-crate).

use super::{quote_ident, sql_str_lit};

#[test]
fn quotes_and_escapes_identifiers() {
    assert_eq!(quote_ident("order"), "`order`");
    assert_eq!(quote_ident("we`ird"), "`we``ird`");
}

#[test]
fn escapes_string_literals() {
    assert_eq!(sql_str_lit("name"), "'name'");
    assert_eq!(sql_str_lit("O'Brien"), "'O''Brien'");
    assert_eq!(sql_str_lit("a\\b"), "'a\\\\b'");
}
