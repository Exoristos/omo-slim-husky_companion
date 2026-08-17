mod state;

use serde_json::Value;
use state::{get_current_state, poll_loop, ENV_SESSION_ID};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

/// Parsed CLI flags. Unknown args are ignored.
#[derive(Debug, Clone, Copy, Default)]
pub struct CliArgs {
    /// Standalone test mode: run without the plugin's env var.
    pub dev: bool,
    /// Control invocation: a compositor keybind spawns the binary without the
    /// env var; it must reach the single-instance plugin.
    pub toggle: bool,
}

pub fn parse_args() -> CliArgs {
    parse_args_from(std::env::args().skip(1))
}

fn parse_args_from<I>(args: I) -> CliArgs
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = CliArgs::default();
    for arg in args {
        match arg.as_str() {
            "--dev" => parsed.dev = true,
            "--toggle" => parsed.toggle = true,
            _ => {}
        }
    }
    parsed
}

/// Wayland detection: `XDG_SESSION_TYPE` containing "wayland" or
/// `WAYLAND_DISPLAY` being set.
fn is_wayland() -> bool {
    is_wayland_from(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    )
}

fn is_wayland_from(xdg_session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    xdg_session_type
        .map(|t| t.to_lowercase().contains("wayland"))
        .unwrap_or(false)
        || wayland_display.filter(|s| !s.is_empty()).is_some()
}

fn is_x11() -> bool {
    !is_wayland()
}

/// Show/hide the main window. When showing, also tell the frontend to open and
/// focus the prompt input.
fn toggle_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = app.emit("opencode:show-prompt", ()); // frontend shows+focuses the prompt input
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args = parse_args();
    let session_id = std::env::var(ENV_SESSION_ID)
        .ok()
        .filter(|s| !s.is_empty());

    // The plugin always sets OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID before
    // spawning the companion. Absence (without --dev/--toggle) means standalone
    // misuse. Control invocations (--toggle) must reach the single-instance
    // plugin, so they skip this exit.
    if session_id.is_none() && !args.dev && !args.toggle {
        eprintln!(
            "oh-my-opencode-slim-companion: {} is not set; the plugin always sets it. \
             Run with --dev for standalone testing.",
            ENV_SESSION_ID
        );
        std::process::exit(1);
    }

    let shared_state: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let x11 = is_x11();

    let mut builder = tauri::Builder::default();

    // Global shortcut: X11 only. On Wayland the compositor keybind (hint
    // printed in setup) spawns `--toggle` instead.
    if x11 {
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        // global-hotkey runs its event loop on a spawned thread;
                        // GTK widget calls must happen on the main thread.
                        let app2 = app.clone();
                        let _ = app.run_on_main_thread(move || toggle_window(&app2));
                    }
                })
                .build(),
        );
    }

    // Single-instance: a second invocation (e.g. the Wayland keybind) forwards
    // its argv to the primary instance, which toggles its window.
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
        if argv.iter().any(|a| a == "--toggle") {
            // single-instance on Linux dispatches via zbus; GTK widget calls
            // must happen on the main thread.
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || toggle_window(&app2));
        }
    }));

    builder
        .manage(shared_state.clone())
        .invoke_handler(tauri::generate_handler![get_current_state])
        .setup(move |app| {
            if x11 {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Err(e) = app.global_shortcut().register("Ctrl+Space") {
                    eprintln!(
                        "[companion] warning: failed to register global shortcut Ctrl+Space: {e}"
                    );
                }
            } else {
                let exe = std::env::args()
                    .next()
                    .unwrap_or_else(|| "opencode-companion".to_string());
                eprintln!(
                    "oh-my-opencode-slim-companion: Wayland session detected — global shortcut \
                     not registered. Add a compositor keybind, e.g. for hyprland:\n  \
                     bind = CTRL SPACE, exec, {} --toggle",
                    exe
                );
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || poll_loop(handle, session_id, shared_state));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dev_and_toggle_flags() {
        let args = parse_args_from(
            ["--dev", "--toggle", "--unknown", "positional"]
                .into_iter()
                .map(String::from),
        );
        assert!(args.dev);
        assert!(args.toggle);
    }

    #[test]
    fn ignores_unknown_args() {
        let args = parse_args_from(["--bogus"].into_iter().map(String::from));
        assert!(!args.dev);
        assert!(!args.toggle);
    }

    #[test]
    fn detects_wayland_from_session_type() {
        assert!(is_wayland_from(Some("wayland"), None));
        assert!(is_wayland_from(Some("Wayland"), None));
        assert!(is_wayland_from(Some("x11"), Some("wayland-0")));
        assert!(!is_wayland_from(Some("x11"), None));
        assert!(!is_wayland_from(None, None));
        assert!(!is_wayland_from(Some("x11"), Some("")));
    }
}