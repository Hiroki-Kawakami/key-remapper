#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod rule;

#[cfg(target_os = "macos")]
#[path = "macos/mod.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows/mod.rs"]
mod platform;

use std::path::PathBuf;

fn main() {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        platform::report_error("usage: key-remapper <config.toml>");
        std::process::exit(2);
    };
    if let Err(e) = platform::run(&path) {
        platform::report_error(&e);
        std::process::exit(1);
    }
}
