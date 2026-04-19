# AGENTS.md — powermode-tray

## Project Overview

A lightweight utility for switching Windows 11 power modes (Balanced / Best Performance / Best Power Efficiency) from the task tray.

- Language: Rust
- Target: Windows 11 x86_64 (MSVC)
- External crate dependency: `windows-sys` only
- No GUI framework used (direct Win32 API calls)

## Build Instructions

Normally, build on Windows using the MSVC toolchain.

```bash
# Release build (no log output, smallest binary)
cargo build --release

# Debug build (with log output)
cargo build
```

Output locations:
- Release: `target/release/powermode-tray.exe`
- Debug: `target/debug/powermode-tray.exe`

Use `cargo-xwin` only when cross-compiling from Linux.

```bash
# Release cross-compilation from Linux
cargo xwin build --target x86_64-pc-windows-msvc --release
```

> **Note**: Do not use `cargo build --target x86_64-pc-windows-gnu`, because the combination of llvm-mingw + `panic = "abort"` causes a linker error.

## Debug Logging

- Output is written to `%LOCALAPPDATA%\powermode-tray\powermode-tray.log` in **debug builds only**
- In release builds, the logging code is removed completely (zero cost)
- Use the `debug_log!` macro defined in `src/debug.rs` and re-exported crate-wide with `pub(crate) use`
- Logs record timestamps (UNIX epoch seconds) along with debug messages from the tray, window, and power-management paths

## Architecture

| File | Role |
|---|---|
| `src/main.rs` | Entry point, window procedure, and message loop |
| `src/debug.rs` | Debug logging implementation and `debug_log!` macro |
| `src/tray.rs` | Hidden window creation and tray icon add/remove |
| `src/menu.rs` | Right-click context menu construction and display |
| `src/power.rs` | Power mode retrieval and setting via the `powrprof.dll` API |
| `src/util.rs` | UTF-16 string conversion helpers for Win32 API calls |

## Design Notes

- Prioritize **minimal memory usage** above all else
- For power mode retrieval and setting, dynamically load undocumented `powrprof.dll` APIs with `LoadLibraryW`/`GetProcAddress`
  - `PowerGetEffectiveOverlayScheme` — get the currently effective overlay (preferred)
  - `PowerGetActualOverlayScheme` — fallback
  - `PowerSetActiveOverlayScheme` — set the overlay
  - These are not exported from the SDK `.lib`, so static linking with `#[link]` is not possible
- Do **not** use the `powercfg` command (it can produce parameter errors in Japanese environments and adds process creation overhead)
- For the system icon, load `LoadIconW(NULL, IDI_APPLICATION)` — passing `NULL` as `hInstance` retrieves a system-defined icon
- Use `#![windows_subsystem = "windows"]` so the app starts without a console window
- In the release profile, set `panic = "abort"`, `lto = true`, and `strip = true` to minimize binary size
