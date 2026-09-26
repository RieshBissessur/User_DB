/// Client version gate, preserved from the legacy service: a client is
/// supported when its reported version is not older than the app version.
pub fn version_supported(client_version: f32, app_version: f32) -> bool {
    client_version >= app_version
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_version_rejected() {
        assert!(!version_supported(0.0, 0.1));
        assert!(!version_supported(0.099, 0.1));
    }

    #[test]
    fn equal_or_newer_version_accepted() {
        assert!(version_supported(0.1, 0.1));
        assert!(version_supported(1.0, 0.1));
    }
}
