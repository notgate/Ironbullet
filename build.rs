fn main() {
    // Force cargo to re-run (and thus re-expand include_dir! macro)
    // whenever any file in gui/build changes.
    println!("cargo:rerun-if-changed=gui/build");
    println!("cargo:rerun-if-changed=assets/brand/ironbullet.ico");

    // Embed the multi-resolution project icon in the Windows executable. The
    // runtime window icon alone does not brand Explorer, Start, shortcuts, or
    // taskbar surfaces before a window has been created.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resources = winres::WindowsResource::new();
        resources.set_icon("assets/brand/ironbullet.ico");
        resources
            .compile()
            .expect("failed to embed assets/brand/ironbullet.ico in ironbullet.exe");
    }
}
