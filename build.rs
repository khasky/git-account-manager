fn main() {
    // GPUI loads the window icon from resource 1. Both binaries carry the
    // resources; Task Manager shows FileDescription as the process name.
    #[cfg(target_os = "windows")]
    winresource::WindowsResource::new()
        .set_icon_with_id("icons/icon.ico", "1")
        .set("ProductName", "Git Account Manager")
        .set("FileDescription", "Git Account Manager")
        .set("CompanyName", "Khasky")
        .set("LegalCopyright", "Copyright (c) 2026 Khasky")
        .compile()
        .expect("cannot embed Windows resources");
}
