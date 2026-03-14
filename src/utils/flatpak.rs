/// Returns `true` when this process is running inside a Flatpak sandbox.
///
/// Detection order:
/// 1. `FLATPAK_ID` environment variable (set by the Flatpak runtime)
/// 2. `/.flatpak-info` file presence (always present inside the sandbox)
pub fn is_flatpak() -> bool {
    std::env::var("FLATPAK_ID").is_ok() || std::path::Path::new("/.flatpak-info").exists()
}
