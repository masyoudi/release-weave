fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/release-weave.ico");
        res.set("ProductName", "ReleaseWeave");
        res.set(
            "FileDescription",
            "Autonomous Release Orchestrator & Studio",
        );
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 ReleaseWeave Contributors",
        );
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {e}");
        }
    }
}
