/// Returns `true` when this process is running inside a Flatpak sandbox.
///
/// Detection order:
/// 1. `FLATPAK_ID` environment variable (set by the Flatpak runtime)
/// 2. `/.flatpak-info` file presence (always present inside the sandbox)
pub fn is_flatpak() -> bool {
    std::env::var("FLATPAK_ID").is_ok() || std::path::Path::new("/.flatpak-info").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_flatpak_returns_bool() {
        // Verify the function runs without panic in any environment
        let _: bool = is_flatpak();
    }

    #[test]
    fn test_is_not_flatpak_in_dev_environment() {
        // In a standard development environment (no sandbox), this should be false
        if std::env::var("FLATPAK_ID").is_err() && !std::path::Path::new("/.flatpak-info").exists() {
            assert!(!is_flatpak());
        }
    }

    #[test]
    fn test_is_flatpak_when_flatpak_id_set() {
        // Guard: only run this assertion if FLATPAK_ID is already present
        if std::env::var("FLATPAK_ID").is_ok() {
            assert!(is_flatpak());
        }
    }
}
