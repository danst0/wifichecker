fn main() {
    // Re-run this script whenever VERSION changes
    println!("cargo:rerun-if-changed=VERSION");

    // Emit link-search paths for poppler-glib so lld can find the library
    // regardless of where it is installed (system, /app/lib in Flatpak, etc.).
    if let Ok(lib) = pkg_config::probe_library("poppler-glib") {
        for path in &lib.link_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }

    let version = std::fs::read_to_string("VERSION")
        .expect("VERSION file not found")
        .trim()
        .to_string();

    // Expose to the app as env!("APP_VERSION")
    println!("cargo:rustc-env=APP_VERSION={}", version);

    // Guard: Cargo.toml must match VERSION
    let cargo_version = env!("CARGO_PKG_VERSION");
    assert_eq!(
        version, cargo_version,
        "VERSION file ({version}) does not match Cargo.toml version ({cargo_version}). Keep them in sync."
    );
}
