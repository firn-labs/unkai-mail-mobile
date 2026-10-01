//! Desktop entry point for the mobile app's webview shell.
//!
//! The real entry point on iOS is `unkai_mobile_lib::run`, which
//! `tauri::mobile_entry_point` exports as the symbol UIKit calls
//! (see `lib.rs`).  This binary exists so `cargo run` still brings
//! the same app up in a desktop window, which is by far the
//! fastest way to iterate on the UI — the simulator is for
//! verifying touch behaviour and native chrome.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    unkai_mobile_lib::run()
}
