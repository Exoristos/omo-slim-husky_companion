# 🐺 oh-my-opencode-slim Companion

A frameless, transparent, always-on-top **desktop pet** for [oh-my-opencode-slim](https://github.com/alvinunreal/oh-my-opencode-slim). A chibi husky that lives on your screen and reacts to your OpenCode agents in real time.

When your agents are working, the husky runs. When OpenCode waits for your input, it sits and waits. When idle, it lounges. Click it — or press the shortcut — to type a response.

Built with **Tauri v2** (Rust + WebKitGTK) and vanilla HTML/CSS/JS. No network calls, no telemetry, fully offline.

## ✨ Features

- **Live agent status** — `idle` / `busy` / `waiting-input` mapped to sprite animations
- **Active agent label** — shows which agent is working (e.g. `orchestrator · busy`)
- **Overlay window** — frameless, transparent, always-on-top, hidden from taskbar (240×320)
- **Global shortcut** — `Ctrl+Space` toggles the window on X11
- **Wayland support** — compositor keybind + `--toggle` flag (see [Wayland](#wayland))
- **Single-instance guard** — no duplicate windows, even from keybind spawns
- **Prompt input** — click the pet or hit the shortcut to type a response
- **Demo mode** — test the UI without OpenCode running
- **Lightweight** — ~195 MB RSS, ~0.4% idle CPU
- **Crash-safe** — missing/corrupt state file → idle view, never crashes

## 📋 Requirements

- **Linux** — X11 or Wayland (with a compositor; transparency requires one)
- **Rust** ≥ 1.77.2
- **Node.js** + npm
- **Tauri v2 system dependencies** (Arch):

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl librsvg
```

For other distros, see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## 🔨 Build

```bash
npm install
npm run tauri build
```

The binary is produced at `src-tauri/target/release/oh-my-opencode-slim-companion`.

## 🚀 Install

1. **Symlink the binary** into your PATH:

```bash
mkdir -p ~/.local/bin
ln -sf "$(pwd)/src-tauri/target/release/oh-my-opencode-slim-companion" ~/.local/bin/opencode-companion
```

2. **Enable it in the plugin config** — `~/.config/opencode/oh-my-opencode-slim.jsonc`:

```jsonc
{
  "companion": {
    "enabled": true,
    "binaryPath": "/home/you/.local/bin/opencode-companion",
    "position": "bottom-right",
    "size": "medium",
    "loopStyle": "classic",
    "speed": 1
  }
}
```

3. **Restart OpenCode.** The plugin spawns the companion automatically for each session.

> The plugin spawns the binary with `OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID` set. Without it (and without `--dev`/`--toggle`) the binary exits immediately — that is intentional.

## 🎮 Usage

| Action | How |
|---|---|
| See agent status | The pet animates: runs while busy, waits on `waiting-input`, lounges when idle |
| Open prompt input | Click the pet, or press the shortcut |
| Submit prompt | `Enter` (v1: echoes to console; forwarding to the OpenCode CLI is planned) |
| Hide prompt | `Escape` |
| Move the window | Drag the pet area |

### Shortcut

- **X11:** `Ctrl+Space` is registered automatically by the app.
- **Wayland:** global shortcuts are X11-only, so bind a compositor key to the `--toggle` flag:

```ini
# Hyprland — ~/.config/hypr/hyprland.conf
bind = CTRL SPACE, exec, ~/.local/bin/opencode-companion --toggle
```

```text
# KDE — System Settings → Shortcuts → Custom Shortcuts
# New shortcut → Command/URL: ~/.local/bin/opencode-companion --toggle
```

**KDE window rules** (System Settings → Window Management → Window Rules) for the class `oh-my-opencode-slim-companion`: **Floating**, **Always on top**, **Size 240×320**. On Hyprland:

```ini
windowrulev2 = float, class:^(oh-my-opencode-slim-companion)$
windowrulev2 = size 240 320, class:^(oh-my-opencode-slim-companion)$
windowrulev2 = pin, class:^(oh-my-opencode-slim-companion)$
windowrulev2 = noborder noinitialfocus, class:^(oh-my-opencode-slim-companion)$
```

## 🧪 Demo mode

Run the UI standalone without OpenCode — it cycles through `idle` → `busy` → `waiting-input` every 3 seconds:

```bash
# Browser (no Tauri needed)
open src/index.html?demo=1

# Or as a standalone window
~/.local/bin/opencode-companion --dev
```

## ⚙️ CLI flags & env vars

| Flag / Env var | Purpose |
|---|---|
| `--dev` | Standalone test mode (no plugin env var required) |
| `--toggle` | Show/hide the window — used by compositor keybinds |
| `OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID` | Set by the plugin; pins this instance to one session |
| `OH_MY_OPENCODE_SLIM_COMPANION_DEBUG=1` | Enable stderr debug logging |

## 🔧 How it works

The plugin writes a shared state file:

```
$XDG_DATA_HOME/opencode/storage/oh-my-opencode-slim/companion-state.json
# default: ~/.local/share/opencode/storage/oh-my-opencode-slim/companion-state.json
```

The companion polls the file's mtime every **250 ms**, selects the most relevant session, and emits `opencode:state-change` events to the webview, which maps `status` → sprite animation row.

**Session selection priority** (mirrors the plugin's own logic):

1. Session pinned by `OH_MY_OPENCODE_SLIM_COMPANION_SESSION_ID`
2. Newest session with status `waiting-input`
3. Newest session with active agents (excluding `intro`)
4. Newest `busy` session
5. Newest session overall
6. No sessions → idle/intro view

## 🛠 Development

```bash
npm run tauri dev        # live-reload overlay window
cargo test               # unit tests (state parsing, session selection, arg parsing)
```

## 🐛 Troubleshooting

| Problem | Fix |
|---|---|
| Black corners / GBM "Error 71" on NVIDIA + Wayland | Handled automatically (`WEBKIT_DISABLE_DMABUF_RENDERER=1`, `__NV_DISABLE_EXPLICIT_SYNC=1`) |
| Shortcut does nothing on Wayland | Expected — use a compositor keybind with `--toggle` |
| Pet stuck / stale | `pkill -f oh-my-opencode-slim-companion`, then restart OpenCode |
| Window not transparent | A compositor is required; on Wayland, always-on-top is a hint most compositors honor |
| Pet shows idle while agents work | The state file is missing or corrupt — the app degrades to idle instead of crashing |

## 🗺 Roadmap (not in v1)

- `error` state visualization (not part of the plugin protocol yet)
- `window_positions` write-back (v1.1)
- Forwarding prompt input to the OpenCode CLI (v2)
- Tray icon, multi-monitor positioning, post-lock/sleep state recovery

## 🙏 Credits

- [oh-my-opencode-slim](https://github.com/alvinunreal/oh-my-opencode-slim) — the plugin this companion integrates with
- Husky sprite sheet generated with the OpenDesign "Hatch" pipeline (chibi pixel-art, 8×9 grid, 192×208 cells)

## 📄 License

MIT