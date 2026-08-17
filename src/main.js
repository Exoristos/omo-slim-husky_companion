const EVENT_NAME = "opencode:state-change";
const SHOW_PROMPT_EVENT = "opencode:show-prompt";
const SUPPORTED_STATES = new Set(["idle", "busy", "waiting-input"]);
const DEMO_STATES = ["idle", "busy", "waiting-input"];

const body = document.body;
const statusLabel = document.querySelector("#status-label");
const promptInput = document.querySelector("#prompt-input");
const petStage = document.querySelector(".pet-stage");

let demoTimer;
let demoIndex = 0;

function hasTauri() {
  return Boolean(window.__TAURI__ || window.__TAURI_INTERNALS__);
}

function normalizeState(value) {
  const state = typeof value === "string" ? value.trim().toLowerCase() : "";
  return SUPPORTED_STATES.has(state) ? state : "idle";
}

function getAgentName(activeAgents) {
  let agent = activeAgents;

  if (Array.isArray(activeAgents)) {
    agent = activeAgents.find((candidate) => {
      if (typeof candidate === "string") return candidate.trim().length > 0;
      return Boolean(candidate?.name || candidate?.agent);
    });
  }

  if (agent && typeof agent === "object") {
    agent = agent.name ?? agent.agent ?? "";
  }

  return typeof agent === "string" ? agent.trim().slice(0, 40) : "";
}

function setState(status, activeAgents, fallbackAgent = "intro") {
  const state = normalizeState(status);
  const agent = getAgentName(activeAgents) || fallbackAgent;

  document.body.dataset.state = state;
  document.body.dataset.agent = agent.toLowerCase();
  statusLabel.textContent = agent ? `${agent} · ${state}` : state;
}

function showPrompt() {
  body.dataset.promptVisible = "true";
  if (promptInput) {
    promptInput.focus();
    promptInput.select();
  }
}

function hidePrompt() {
  delete body.dataset.promptVisible;
  if (promptInput) promptInput.blur();
}

function startDemo() {
  if (demoTimer) return;

  demoIndex = 0;
  setState(DEMO_STATES[demoIndex], "demo", "demo");
  demoTimer = window.setInterval(() => {
    demoIndex = (demoIndex + 1) % DEMO_STATES.length;
    setState(DEMO_STATES[demoIndex], "demo", "demo");
  }, 3000);
}

function parsePayload(payload) {
  if (typeof payload !== "string") return payload ?? {};

  try {
    const parsed = JSON.parse(payload);
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

async function loadListen() {
  // Vanilla template'te bundler yok — bare-specifier import çözülemez.
  // withGlobalTauri: true sayesinde window.__TAURI__.event.listen tek çalışan yoldur.
  const globalEvent = window.__TAURI__?.event;
  return typeof globalEvent?.listen === "function"
    ? globalEvent.listen.bind(globalEvent)
    : null;
}

async function connectToTauri() {
  const demoRequested = new URLSearchParams(window.location.search).get("demo") === "1";

  if (demoRequested || !hasTauri()) {
    startDemo();
    return;
  }

  const listen = await loadListen();
  if (!listen) {
    startDemo();
    return;
  }

  try {
    await listen(EVENT_NAME, (event) => {
      const payload = parsePayload(event?.payload);
      setState(payload.status, payload.active_agents);
    });

    await listen(SHOW_PROMPT_EVENT, async () => {
      // The IPC round-trip guarantees the window show request was processed
      // before focusing; set_focus() right after show() is a silent no-op on
      // tao Linux (the widget is not visible yet when the focus request is
      // gated).
      try {
        await window.__TAURI__.core.invoke("plugin:window|set_focus", { label: "main" });
      } catch (error) {
        console.warn("set_focus başarısız:", error);
      }
      showPrompt();
    });

    // Handshake: the poller's first emit can land before this listener is
    // registered, so pull the current state explicitly to avoid a stale pet.
    try {
      const current = await window.__TAURI__.core.invoke("get_current_state");
      const payload = parsePayload(current);
      setState(payload.status, payload.active_agents);
    } catch (error) {
      console.warn("get_current_state handshake başarısız:", error);
    }
  } catch (error) {
    console.warn("opencode:state-change dinleyicisi kurulamadı, demo moduna düşüldü:", error);
    startDemo();
  }
}

// Clicking the pet opens the prompt input (the only interactive surface until
// the shortcut). Coexists with data-tauri-drag-region dragging.
if (petStage) {
  petStage.addEventListener("click", () => {
    if (hasTauri()) showPrompt();
  });
}

// Prompt lifecycle: Escape hides, Enter echoes (v1; Phase 4+ may forward to
// the opencode CLI).
if (promptInput) {
  promptInput.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      hidePrompt();
    } else if (event.key === "Enter") {
      const value = promptInput.value;
      console.log("[prompt]", value);
      promptInput.value = "";
    }
  });
}

setState(body.dataset.state, body.dataset.agent);
void connectToTauri();