//! Append-only diagnostic log for the manual verification loop.
//!
//! The file lives next to the executable (`<exe dir>/logs/rndwalker.log`) so a
//! `cargo run` build and a distributed build both leave their log somewhere
//! predictable, instead of under `%LOCALAPPDATA%`.
//!
//! Every line is `<local time> [<event>] <detail>`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Resolved once; `None` means we could not create the directory and logging
/// is silently skipped — diagnostics must never take the app down.
fn log_file() -> Option<&'static PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = std::env::current_exe().ok()?.parent()?.join("logs");
        fs::create_dir_all(&dir).ok()?;
        Some(dir.join("rndwalker.log"))
    })
    .as_ref()
}

pub fn log(event: &str, detail: &str) {
    let Some(path) = log_file() else {
        return;
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{} [{event}] {detail}", timestamp());
}

#[cfg(windows)]
fn timestamp() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut now = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut now) };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond, now.wMilliseconds
    )
}

#[cfg(not(windows))]
fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or(0);
    format!("epoch+{millis}ms")
}
