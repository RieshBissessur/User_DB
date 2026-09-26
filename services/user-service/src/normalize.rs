/// Normalize a username or email for storage and lookup.
/// The legacy service lowercased usernames/emails for all file paths and the
/// email map, so we keep that rule (now enforced by the DB unique keys).
pub fn normalize(value: &str) -> String {
    value.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_ascii() {
        assert_eq!(normalize("Alice"), "alice");
        assert_eq!(normalize("ALICE@Example.COM"), "alice@example.com");
    }

    #[test]
    fn already_lower_is_unchanged() {
        assert_eq!(normalize("alice"), "alice");
    }
}
