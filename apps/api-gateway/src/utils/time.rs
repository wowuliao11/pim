use chrono::{DateTime, Utc};

/// Get current UTC timestamp
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Get current Unix timestamp in seconds
pub fn unix_timestamp() -> i64 {
    Utc::now().timestamp()
}

/// Get current Unix timestamp in milliseconds
pub fn unix_timestamp_millis() -> i64 {
    Utc::now().timestamp_millis()
}

/// Format datetime to ISO 8601 string
pub fn to_iso_string(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now() {
        let time = now();
        assert!(time.timestamp() > 0);
    }

    #[test]
    fn test_unix_timestamp() {
        let ts = unix_timestamp();
        assert!(ts > 0);
    }
}
