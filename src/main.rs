//! Plume — a lightweight native window for the official Discord web app.
//!
//! The window hosts https://discord.com unmodified in Microsoft WebView2 (the Chromium
//! engine shipped with Windows). No script is injected and the page has no bridge to
//! native code: the attack surface Plume adds on top of a browser tab is `app.rs` and
//! `policy.rs`.

// Release builds are GUI applications: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// On other platforms only the unit tests use these (pure, platform-independent) modules.
#[cfg_attr(not(windows), allow(dead_code))]
mod badge;
#[cfg_attr(not(windows), allow(dead_code))]
mod policy;
#[cfg_attr(not(windows), allow(dead_code))]
mod settings;

#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    app::run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Plume ne fonctionne que sous Windows (WebView2).");
    std::process::exit(1);
}
