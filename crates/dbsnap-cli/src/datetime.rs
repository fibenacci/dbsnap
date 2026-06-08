//! Minimal UTC formatting from Unix epoch seconds, without a date dependency.

/// Render a Unix timestamp as a readable UTC string.
pub fn format_ts(secs: i64) -> String {
    if secs <= 0 {
        return "(unknown)".to_string();
    }
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// Howard Hinnant's days-from-epoch → (year, month, day) algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// White-box test of the private conversion: stays beside the code, the
// idiomatic Rust place for testing crate-private helpers.
#[cfg(test)]
mod tests {
    use super::format_ts;

    #[test]
    fn formats_known_epoch() {
        // 2021-01-01T00:00:00Z
        assert_eq!(format_ts(1_609_459_200), "2021-01-01 00:00:00 UTC");
    }

    #[test]
    fn non_positive_is_unknown() {
        assert_eq!(format_ts(0), "(unknown)");
    }
}
