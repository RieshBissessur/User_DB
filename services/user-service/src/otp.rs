use chrono::{Duration, NaiveDateTime};
use rand::Rng;

/// Generate a 4-digit numeric one-time password (leading zeros allowed).
pub fn generate_otp() -> String {
    (0..4)
        .map(|_| rand::thread_rng().gen_range(0..=9).to_string())
        .collect()
}

/// Expiry timestamp for a freshly generated OTP.
pub fn expires_at(now: NaiveDateTime, ttl_minutes: i64) -> NaiveDateTime {
    let delta = Duration::try_minutes(ttl_minutes).unwrap_or_else(Duration::zero);
    now + delta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_is_four_digits() {
        for _ in 0..100 {
            let otp = generate_otp();
            assert_eq!(otp.len(), 4);
            assert!(otp.chars().all(|c| c.is_ascii_digit()), "otp={otp}");
        }
    }

    #[test]
    fn expiry_adds_ttl() {
        let now =
            NaiveDateTime::parse_from_str("2026-01-01 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(
            expires_at(now, 120),
            NaiveDateTime::parse_from_str("2026-01-01 14:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        );
    }
}
