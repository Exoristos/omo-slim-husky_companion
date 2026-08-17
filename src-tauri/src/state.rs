//! Phase 2: state poller for the oh-my-opencode-slim companion overlay.
//!
//! Reads the plugin's shared state file (`companion-state.json`) and emits
//! `opencode:state-change` events to the frontend whenever the selected
//! session state actually changes.
//!
//! Pure functions (`parse_state`, `choose_session`, `build_payload`) are kept
//! separate from the poll loop so the selection logic is unit-testable.

use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub const EVENT_NAME: &str = "opencode:state-change";
pub const POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const STATE_VERSION: u32 = 1;

/// Env var set by the plugin to pin this companion instance to one session.
pub const ENV_SESSION_ID: &str = "OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID";
/// Env var that enables gated stderr debug logging.
pub const ENV_DEBUG: &str = "OH_MY_OPENCODE_SLIM_COMPANION_DEBUG";

// Schema fields not yet read by the poller (config, window_positions,
// active_agent, pid, session.config) are tolerated for forward compatibility
// with the plugin's state file and may be used by future phases.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CompanionState {
    pub version: Option<u32>,
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub window_positions: Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub active_agents: Option<Vec<String>>,
    #[serde(default)]
    pub active_agent: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub pid: Option<i64>,
    #[serde(default)]
    pub config: Option<Value>,
}

/// `$XDG_DATA_HOME/opencode/storage/oh-my-opencode-slim/companion-state.json`
/// (defaults to `~/.local/share/...` when XDG_DATA_HOME is unset).
pub fn state_file_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("opencode")
        .join("storage")
        .join("oh-my-opencode-slim")
        .join("companion-state.json")
}

/// Parse the state file. Returns `None` on corrupt JSON or a version mismatch
/// (only version 1 is supported; a missing version is tolerated).
pub fn parse_state(raw: &str) -> Option<CompanionState> {
    let state: CompanionState = serde_json::from_str(raw).ok()?;
    match state.version {
        None => Some(state),
        Some(v) if v == STATE_VERSION => Some(state),
        Some(_) => None,
    }
}

/// Session selection, mirroring the plugin's `app.rs::choose_session`:
/// 1. env-pinned session id wins if present,
/// 2. else newest session with status "waiting-input",
/// 3. else newest session with non-empty active_agents (excluding "intro"),
/// 4. else the newest "busy" session,
/// 5. else the newest session,
/// 6. no sessions at all → `None` (idle/intro).
///
/// NOTE: "newest" is interpreted as the LAST element of the `sessions` array
/// (sessions are appended as they start). If upstream ever prepends instead,
/// flip the `.rev()`/`.last()` calls.
pub fn choose_session<'a>(
    state: &'a CompanionState,
    env_session_id: Option<&str>,
) -> Option<&'a Session> {
    let sessions = &state.sessions;
    if sessions.is_empty() {
        return None;
    }

    // 1. Owner-session priority.
    if let Some(id) = env_session_id {
        if let Some(s) = sessions.iter().find(|s| s.session_id == id) {
            return Some(s);
        }
    }

    // 2. Newest waiting-input session.
    if let Some(s) = sessions
        .iter()
        .rev()
        .find(|s| s.status.as_deref() == Some("waiting-input"))
    {
        return Some(s);
    }

    // 3. Newest session with non-empty active_agents, excluding "intro".
    if let Some(s) = sessions.iter().rev().find(|s| {
        s.active_agents
            .as_ref()
            .map_or(false, |a| !a.is_empty() && a.iter().any(|agent| agent != "intro"))
    }) {
        return Some(s);
    }

    // 4. Newest busy session.
    if let Some(s) = sessions.iter().rev().find(|s| s.status.as_deref() == Some("busy")) {
        return Some(s);
    }

    // 5. Newest session.
    sessions.last()
}

/// Build the event payload for a selected session.
pub fn build_payload(session: &Session) -> Value {
    let status = match session.status.as_deref() {
        Some("idle") | Some("busy") | Some("waiting-input") => session.status.clone().unwrap(),
        _ => "idle".to_string(),
    };
    serde_json::json!({
        "session_id": session.session_id,
        "cwd": session.cwd.clone().unwrap_or_default(),
        "active_agents": session.active_agents.clone().unwrap_or_default(),
        "status": status,
        "message": null,
    })
}

/// Payload emitted when the plugin is not running (no file / no sessions).
pub fn idle_payload() -> Value {
    serde_json::json!({
        "session_id": "",
        "cwd": "",
        "active_agents": [],
        "status": "idle",
        "message": null,
    })
}

/// Frontend handshake: returns the last emitted payload, or the idle payload
/// if the poller has not produced one yet. Fixes the stale-pet bug where the
/// poller's first event lands before the webview listener is registered.
#[tauri::command]
pub fn get_current_state(state: tauri::State<'_, Arc<Mutex<Option<Value>>>>) -> Value {
    state
        .lock()
        .map(|guard| guard.clone().unwrap_or_else(idle_payload))
        .unwrap_or_else(|_| idle_payload())
}

fn debug_enabled() -> bool {
    std::env::var(ENV_DEBUG).as_deref() == Ok("1")
}

/// Poll loop: checks the state file mtime every 250 ms, re-reads only on
/// change, and emits `opencode:state-change` only when the selected state
/// actually changes. Never crashes on bad input; parse errors keep the last
/// good state and are logged at most once per error-state transition.
///
/// The last emitted payload is kept in `shared_state` so the frontend can pull
/// it via `get_current_state` (the first event often lands before the webview
/// listener is registered).
pub fn poll_loop(
    app: tauri::AppHandle,
    env_session_id: Option<String>,
    shared_state: Arc<Mutex<Option<Value>>>,
) {
    let path = state_file_path();
    let debug = debug_enabled();
    if debug {
        eprintln!("[companion] state file: {}", path.display());
    }

    let mut last_mtime: Option<SystemTime> = None;
    let mut parse_error_logged = false;
    let mut first_pass = true;

    loop {
        let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        // first_pass forces the body to run once even when the file is absent
        // from startup (mtime == None == last_mtime would otherwise skip it).
        if first_pass || mtime != last_mtime {
            first_pass = false;
            last_mtime = mtime;
            if debug {
                eprintln!("[companion] mtime changed: {:?}", mtime);
            }

            match std::fs::read_to_string(&path) {
                Ok(raw) => match parse_state(&raw) {
                    Some(state) => {
                        parse_error_logged = false;
                        let selected = choose_session(&state, env_session_id.as_deref());
                        let payload = selected.map(build_payload).unwrap_or_else(idle_payload);
                        if debug {
                            eprintln!(
                                "[companion] selected session: {:?}",
                                selected.map(|s| s.session_id.as_str())
                            );
                        }
                        emit_if_changed(&app, &shared_state, payload, debug);
                    }
                    None => {
                        // Corrupt / version-mismatched file: keep last good
                        // state, do not crash, log at most once per transition.
                        if !parse_error_logged {
                            if debug {
                                eprintln!(
                                    "[companion] parse error in state file; keeping last good state"
                                );
                            }
                            parse_error_logged = true;
                        }
                    }
                },
                Err(_) => {
                    // File absent / unreadable → plugin not running → idle/intro.
                    parse_error_logged = false;
                    emit_if_changed(&app, &shared_state, idle_payload(), debug);
                }
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Emit `payload` only if it differs from the last stored one, and always keep
/// the shared state fresh so `get_current_state` can serve it to the frontend.
fn emit_if_changed(
    app: &tauri::AppHandle,
    shared_state: &Arc<Mutex<Option<Value>>>,
    payload: Value,
    debug: bool,
) {
    use tauri::Emitter;

    let mut guard = shared_state.lock().unwrap();
    if guard.as_ref() != Some(&payload) {
        if debug {
            eprintln!("[companion] emitting: {}", payload);
        }
        let _ = app.emit(EVENT_NAME, &payload);
        *guard = Some(payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> CompanionState {
        serde_json::from_str(
            r#"{
                "version": 1,
                "sessions": [
                    {
                        "session_id": "s1",
                        "cwd": "/proj/a",
                        "active_agents": ["orchestrator"],
                        "status": "busy",
                        "pid": 100
                    },
                    {
                        "session_id": "s2",
                        "cwd": "/proj/b",
                        "active_agents": ["fixer"],
                        "status": "waiting-input",
                        "pid": 200
                    }
                ],
                "config": {},
                "window_positions": {}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn parses_valid_state() {
        let state = parse_state(r#"{"version":1,"sessions":[]}"#);
        assert!(state.is_some());
        assert!(state.unwrap().sessions.is_empty());
    }

    #[test]
    fn rejects_corrupt_json() {
        assert!(parse_state("not json {").is_none());
        assert!(parse_state("").is_none());
    }

    #[test]
    fn rejects_version_mismatch() {
        assert!(parse_state(r#"{"version":2,"sessions":[]}"#).is_none());
    }

    #[test]
    fn accepts_missing_version() {
        assert!(parse_state(r#"{"sessions":[]}"#).is_some());
    }

    #[test]
    fn empty_sessions_yields_none() {
        let state = parse_state(r#"{"version":1,"sessions":[]}"#).unwrap();
        assert!(choose_session(&state, None).is_none());
        assert!(choose_session(&state, Some("s1")).is_none());
    }

    #[test]
    fn owner_session_priority() {
        let state = sample_state();
        // s2 is newer AND waiting-input, but the env-pinned s1 must win.
        let chosen = choose_session(&state, Some("s1")).unwrap();
        assert_eq!(chosen.session_id, "s1");
    }

    #[test]
    fn owner_session_unknown_id_falls_through() {
        let state = sample_state();
        // Unknown pinned id → falls through to waiting-input priority.
        let chosen = choose_session(&state, Some("nope")).unwrap();
        assert_eq!(chosen.session_id, "s2");
    }

    #[test]
    fn waiting_input_priority() {
        let state = sample_state();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s2");
    }

    #[test]
    fn active_agents_excluding_intro() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"idle","active_agents":["intro"]},
                    {"session_id":"s2","status":"idle","active_agents":["fixer"]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s2");
    }

    #[test]
    fn busy_fallback() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"busy","active_agents":[]},
                    {"session_id":"s2","status":"idle","active_agents":[]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s1");
    }

    #[test]
    fn newest_session_fallback() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"idle","active_agents":[]},
                    {"session_id":"s2","status":"idle","active_agents":[]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s2"); // last element = newest
    }

    #[test]
    fn no_session_yields_idle_payload() {
        let state = parse_state(r#"{"version":1,"sessions":[]}"#).unwrap();
        assert!(choose_session(&state, None).is_none());
        let payload = idle_payload();
        assert_eq!(payload["status"], "idle");
        assert_eq!(payload["active_agents"], serde_json::json!([]));
        assert!(payload["message"].is_null());
    }

    #[test]
    fn payload_shape() {
        let state = sample_state();
        let chosen = choose_session(&state, Some("s1")).unwrap();
        let payload = build_payload(chosen);
        assert_eq!(payload["session_id"], "s1");
        assert_eq!(payload["cwd"], "/proj/a");
        assert_eq!(payload["active_agents"], serde_json::json!(["orchestrator"]));
        assert_eq!(payload["status"], "busy");
        assert!(payload["message"].is_null());
    }

    #[test]
    fn unknown_status_normalized_to_idle() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"weird","active_agents":[]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        let payload = build_payload(chosen);
        assert_eq!(payload["status"], "idle");
    }

    #[test]
    fn newest_waiting_input_wins() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"waiting-input","active_agents":[]},
                    {"session_id":"s2","status":"waiting-input","active_agents":[]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s2"); // last element = newest
    }

    #[test]
    fn newest_active_agents_wins() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"idle","active_agents":["orchestrator"]},
                    {"session_id":"s2","status":"idle","active_agents":["fixer"]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s2"); // last element = newest
    }

    #[test]
    fn newest_busy_wins() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"busy","active_agents":[]},
                    {"session_id":"s2","status":"busy","active_agents":[]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        assert_eq!(chosen.session_id, "s2"); // last element = newest
    }

    #[test]
    fn payload_missing_cwd_defaults_empty() {
        let state = parse_state(
            r#"{
                "version": 1,
                "sessions": [
                    {"session_id":"s1","status":"busy","active_agents":[]}
                ]
            }"#,
        )
        .unwrap();
        let chosen = choose_session(&state, None).unwrap();
        let payload = build_payload(chosen);
        assert_eq!(payload["cwd"], "");
    }
}