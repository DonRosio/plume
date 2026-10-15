//! Embeds the icon and version information in plume.exe, so that Explorer, the taskbar and
//! pinned shortcuts show the quill (the runtime window icon alone is not enough for pins).

fn main() {
    println!("cargo:rerun-if-changed=assets/plume.ico");
    // Only when building *on* Windows *for* Windows: the resource compiler (rc.exe) comes
    // with the Visual Studio C++ tools. Cross-checks from Linux skip this step.
    let target_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    if cfg!(windows) && target_windows {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/plume.ico")
            .set("ProductName", "Plume")
            .set("FileDescription", "Plume")
            .set("LegalCopyright", "Plume n'est pas affilié à Discord");
        if let Err(err) = resource.compile() {
            panic!(
                "cannot embed the icon (rc.exe from the Visual Studio C++ tools is required): {err}"
            );
        }
    }
}
