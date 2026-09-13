//! Kubernetes quantity parsing.
//!
//! Replaces the Python `convert_to_milicore` / `convert_to_mi`
//! (`cluster_utils.py`), which only handled `n`/`u` for CPU and `Ki`/`Mi` for
//! memory and raised `UnboundLocalError` on anything else (migration.md **O4**;
//! spec: `tests/test_quantity.py`). This version accepts the full suffix set
//! and is total — every parse either returns a value or a typed error.
//!
//! Note a deliberate fix vs. the Python spec: memory `Ki`/`Mi`/… are treated as
//! binary (1024-based), so `1024000Ki` is `1000 Mi`, not the `1024.0` the buggy
//! Python `/1000` produced.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuantityError {
    #[error("empty quantity")]
    Empty,
    #[error("invalid number in quantity {0:?}")]
    Number(String),
    #[error("unknown unit suffix {suffix:?} in quantity {input:?}")]
    Unit { input: String, suffix: String },
}

/// Split `"250000u"` into (`250000f64`, `"u"`). The suffix is everything after
/// the leading number (digits, one optional `.`, optional leading `-`/`+`).
fn split_number_suffix(raw: &str) -> Result<(f64, &str), QuantityError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(QuantityError::Empty);
    }
    let end = s
        .find(|c: char| {
            !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')
        })
        .unwrap_or(s.len());
    let (num, suffix) = s.split_at(end);
    let value: f64 = num
        .parse()
        .map_err(|_| QuantityError::Number(raw.to_string()))?;
    Ok((value, suffix))
}

/// Parse a CPU quantity to **millicores**, rounding up (matches the Python
/// `math.ceil`). `"1"` -> 1000, `"100m"` -> 100, `"250000u"` -> 250,
/// `"1000000n"` -> 1, `"1500u"` -> 2.
pub fn cpu_millicores(raw: &str) -> Result<i64, QuantityError> {
    let (value, suffix) = split_number_suffix(raw)?;
    // cores per unit
    let cores = match suffix {
        "" => value,
        "n" => value / 1e9,
        "u" | "µ" => value / 1e6,
        "m" => value / 1e3,
        "k" => value * 1e3,
        "M" => value * 1e6,
        "G" => value * 1e9,
        other => {
            return Err(QuantityError::Unit {
                input: raw.to_string(),
                suffix: other.to_string(),
            })
        }
    };
    Ok((cores * 1000.0).ceil() as i64)
}

/// Parse a memory quantity to a number of **mebibytes** (1 Mi = 1024*1024 B).
/// `"512Mi"` -> 512.0, `"2Gi"` -> 2048.0, `"1024000Ki"` -> 1000.0,
/// `"1000000000"` -> ~953.67.
pub fn memory_mib(raw: &str) -> Result<f64, QuantityError> {
    let (value, suffix) = split_number_suffix(raw)?;
    let bytes = match suffix {
        "" => value,
        "Ki" => value * 1024.0,
        "Mi" => value * 1024f64.powi(2),
        "Gi" => value * 1024f64.powi(3),
        "Ti" => value * 1024f64.powi(4),
        "Pi" => value * 1024f64.powi(5),
        "k" | "K" => value * 1e3,
        "M" => value * 1e6,
        "G" => value * 1e9,
        "T" => value * 1e12,
        other => {
            return Err(QuantityError::Unit {
                input: raw.to_string(),
                suffix: other.to_string(),
            })
        }
    };
    Ok(bytes / 1024f64.powi(2))
}

/// `"250m"` style rendering used by the `usage` view CPU column.
pub fn format_millicores(millicores: i64) -> String {
    format!("{millicores}m")
}

/// `"512.0Mi"` style rendering used by the `usage` view memory column.
pub fn format_mib(mib: f64) -> String {
    format!("{mib}Mi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_known_suffixes_round_up() {
        assert_eq!(cpu_millicores("1000000n").unwrap(), 1);
        assert_eq!(cpu_millicores("500000000n").unwrap(), 500);
        assert_eq!(cpu_millicores("1n").unwrap(), 1);
        assert_eq!(cpu_millicores("250000u").unwrap(), 250);
        assert_eq!(cpu_millicores("1500u").unwrap(), 2);
    }

    #[test]
    fn cpu_millicore_and_core_inputs_that_python_got_wrong() {
        // Python O4: no branch -> "0m". Rust must be correct.
        assert_eq!(cpu_millicores("100m").unwrap(), 100);
        assert_eq!(cpu_millicores("2").unwrap(), 2000);
        assert_eq!(cpu_millicores("0").unwrap(), 0);
        assert_eq!(cpu_millicores("1500m").unwrap(), 1500);
    }

    #[test]
    fn cpu_rejects_garbage() {
        assert!(matches!(
            cpu_millicores("abc"),
            Err(QuantityError::Number(_))
        ));
        assert!(matches!(
            cpu_millicores("10Zi"),
            Err(QuantityError::Unit { .. })
        ));
        assert_eq!(cpu_millicores("   "), Err(QuantityError::Empty));
    }

    #[test]
    fn memory_known_suffixes_binary() {
        assert_eq!(memory_mib("512Mi").unwrap(), 512.0);
        assert_eq!(memory_mib("1024000Ki").unwrap(), 1000.0);
        assert_eq!(memory_mib("2Gi").unwrap(), 2048.0);
        assert_eq!(memory_mib("1Ti").unwrap(), 1024.0 * 1024.0);
    }

    #[test]
    fn memory_unhandled_suffixes_that_python_crashed_on() {
        // Python O4: UnboundLocalError. Rust returns a number.
        assert!((memory_mib("1000000000").unwrap() - 953.674_316).abs() < 1e-3);
        assert!(memory_mib("900000").unwrap() > 0.0);
        assert_eq!(memory_mib("1G").unwrap(), 1e9 / 1024f64.powi(2));
    }
}
