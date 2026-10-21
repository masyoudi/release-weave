#![allow(dead_code)]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;

fn main() {
    match release_weave::logging::init() {
        Ok(path) => tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            log_file = %path.display(),
            "ReleaseWeave starting"
        ),
        Err(e) => eprintln!("Logging disabled: {e}"),
    }

    app::run();
}
