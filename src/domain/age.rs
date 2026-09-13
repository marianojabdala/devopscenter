//! Kubernetes-style relative-age formatting (`5s`, `12m`, `3h`, `4d`), used
//! to render the `events` table the way `kubectl get events` does.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, Time};

/// Format an age in whole seconds the way `kubectl` does: the largest unit
/// that fits, no fractional/compound parts.
pub fn format_secs(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    }
}

/// Seconds elapsed between `now` and `time` (0 if `time` is in the future).
pub fn secs_since(now: i64, time: &Time) -> i64 {
    now - time.0.as_second()
}

pub fn secs_since_micro(now: i64, time: &MicroTime) -> i64 {
    now - time.0.as_second()
}

/// Now, as seconds since the Unix epoch — pass the same value to every
/// `secs_since*` call in one render so ages within a table are consistent.
pub fn now_secs() -> i64 {
    k8s_openapi::jiff::Timestamp::now().as_second()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_secs_picks_the_largest_fitting_unit() {
        assert_eq!(format_secs(0), "0s");
        assert_eq!(format_secs(59), "59s");
        assert_eq!(format_secs(60), "1m");
        assert_eq!(format_secs(3599), "59m");
        assert_eq!(format_secs(3600), "1h");
        assert_eq!(format_secs(86399), "23h");
        assert_eq!(format_secs(86400), "1d");
        assert_eq!(format_secs(200_000), "2d");
    }

    #[test]
    fn format_secs_clamps_negative_to_zero() {
        assert_eq!(format_secs(-5), "0s");
    }
}
