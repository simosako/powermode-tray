#[cfg(debug_assertions)]
use std::sync::{Mutex, OnceLock};

// In debug builds, write timestamped lines to
// %LOCALAPPDATA%\powermode-tray\powermode-tray.log.
// In release builds, this is a no-op.

#[cfg(debug_assertions)]
static DEBUG_LOG_FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

#[cfg(debug_assertions)]
fn debug_log_path() -> Option<std::path::PathBuf> {
    let app_dir = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)?
        .join("powermode-tray");
    std::fs::create_dir_all(&app_dir).ok()?;
    Some(app_dir.join("powermode-tray.log"))
}

#[cfg(debug_assertions)]
pub(crate) fn write_debug_log(args: std::fmt::Arguments<'_>) {
    use std::io::Write as _;

    let log_file = DEBUG_LOG_FILE.get_or_init(|| {
        let path = debug_log_path()?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Mutex::new(file))
    });

    if let Some(log_file) = log_file {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut file = log_file.lock().unwrap_or_else(|err| err.into_inner());
        let _ = writeln!(file, "[{}] {}", now, args);
    }
}

#[cfg(debug_assertions)]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        $crate::debug::write_debug_log(format_args!($($arg)*));
    }};
}

#[cfg(not(debug_assertions))]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}

pub(crate) use debug_log;
