import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition, LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { register, unregister, isRegistered } from "@tauri-apps/plugin-global-shortcut";

const windowHandle = getCurrentWindow();
const COMPACT_WIDTH = 336;
const COMPACT_HEIGHT = 58;
const SETTINGS_WIDTH = 380;
const SETTINGS_HEIGHT = 628;
const LOCAL_PREVIEW_INTERVAL_MS = 1800;
const ONLINE_PREVIEW_INTERVAL_MS = 6000;
const AUDIO_LEVEL_INTERVAL_MS = 55;
const POSITION_STORAGE_KEY = "auralflow-dock-position-v3";
const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform);
const METER_WEIGHTS = [0.54, 0.78, 0.94, 0.68, 1, 0.74, 0.9, 0.66, 0.5];

const elements = {
  recordButton: document.querySelector("#record-button"),
  dragHandle: document.querySelector("#drag-handle"),
  statusTitle: document.querySelector("#status-title"),
  statusDetail: document.querySelector("#status-detail"),
  statusDot: document.querySelector("#status-dot"),
  meter: document.querySelector("#meter"),
  transcript: document.querySelector("#transcript"),
  copyButton: document.querySelector("#copy-button"),
  settingsButton: document.querySelector("#settings-button"),
  hideButton: document.querySelector("#hide-button"),
  settingsPanel: document.querySelector("#settings-panel"),
  engine: document.querySelector("#engine"),
  privacyPill: document.querySelector("#privacy-pill"),
  groqSettings: document.querySelector("#groq-settings"),
  groqApiKey: document.querySelector("#groq-api-key"),
  groqKeyStatus: document.querySelector("#groq-key-status"),
  clearGroqKey: document.querySelector("#clear-groq-key"),
  localSettings: document.querySelector("#local-settings"),
  modelActions: document.querySelector("#model-actions"),
  modelProgress: document.querySelector("#model-progress"),
  modelName: document.querySelector("#model-name"),
  language: document.querySelector("#language"),
  hotkey: document.querySelector("#hotkey"),
  holdToTalk: document.querySelector("#hold-to-talk"),
  fnHold: document.querySelector("#fn-hold"),
  fnHoldRow: document.querySelector("#fn-hold-row"),
  platformHint: document.querySelector("#platform-hint"),
  autoPaste: document.querySelector("#auto-paste"),
  autoPasteHint: document.querySelector("#auto-paste-hint"),
  modelStatus: document.querySelector("#model-status"),
  modelPath: document.querySelector("#model-path"),
  downloadButton: document.querySelector("#download-button"),
  downloadProgress: document.querySelector("#download-progress"),
  saveButton: document.querySelector("#save-button"),
  toast: document.querySelector("#toast"),
};

let recording = false;
let busy = false;
let previewInFlight = false;
let previewTimer = null;
let audioLevelTimer = null;
let audioLevelInFlight = false;
let smoothedLevel = 0;
let currentHotkey = null;
let lastTranscript = "";
let preparedModel = null;
let settingsOpen = false;
let movingProgrammatically = false;
let compactAnchorPosition = null;
let holdToTalkMode = true;
let fnHoldEnabled = true;
let holdRequested = false;
let shortcutDown = false;
let hotkeyCaptureActive = false;

async function placeDock() {
  await windowHandle.setSize(new LogicalSize(COMPACT_WIDTH, COMPACT_HEIGHT));
  const saved = window.localStorage.getItem(POSITION_STORAGE_KEY);
  if (saved) {
    try {
      const position = JSON.parse(saved);
      const scale = window.devicePixelRatio || 1;
      const minX = (window.screen.availLeft ?? 0) * scale;
      const minY = (window.screen.availTop ?? 0) * scale;
      const maxX = minX + window.screen.availWidth * scale;
      const maxY = minY + window.screen.availHeight * scale;
      const visible = Number.isFinite(position.x)
        && Number.isFinite(position.y)
        && position.x + COMPACT_WIDTH * scale > minX
        && position.x < maxX
        && position.y + COMPACT_HEIGHT * scale > minY
        && position.y < maxY;
      if (visible) {
        await windowHandle.setPosition(new PhysicalPosition(position.x, position.y));
        return;
      }
    } catch (_) {}
    window.localStorage.removeItem(POSITION_STORAGE_KEY);
  }
  const left = (window.screen.availLeft ?? 0) + (window.screen.availWidth - COMPACT_WIDTH) / 2;
  const top = (window.screen.availTop ?? 0) + window.screen.availHeight - COMPACT_HEIGHT - 8;
  await windowHandle.setPosition(new LogicalPosition(Math.max(0, left), Math.max(0, top)));
}

async function rememberPosition() {
  if (movingProgrammatically || settingsOpen) return;
  const position = await windowHandle.outerPosition();
  window.localStorage.setItem(POSITION_STORAGE_KEY, JSON.stringify({ x: position.x, y: position.y }));
}

function setStatus(kind, title, detail) {
  elements.statusTitle.textContent = title;
  elements.statusDetail.textContent = detail;
  elements.statusDot.dataset.state = kind;
  elements.recordButton.classList.toggle("recording", kind === "recording");
  elements.recordButton.disabled = kind === "processing";
  elements.recordButton.setAttribute("aria-label", kind === "recording" ? "Detener y transcribir" : "Empezar a grabar");
}

function showTranscript(text, partial = false) {
  if (!text) return;
  lastTranscript = text;
  elements.transcript.textContent = text;
  elements.transcript.classList.remove("empty");
  elements.transcript.classList.toggle("partial", partial);
  elements.copyButton.disabled = false;
}

function showToast(message, isError = false) {
  elements.toast.textContent = message;
  elements.toast.classList.toggle("error", isError);
  elements.toast.classList.add("visible");
  window.clearTimeout(showToast.timeout);
  showToast.timeout = window.setTimeout(() => elements.toast.classList.remove("visible"), 3000);
}

function keyForAccelerator(event) {
  const modifierKeys = new Set(["Alt", "AltGraph", "Control", "Meta", "Shift"]);
  if (modifierKeys.has(event.key)) return null;
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit[0-9]$/.test(event.code)) return event.code.slice(5);
  if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(event.key)) return event.key.toUpperCase();
  const names = {
    " ": "Space",
    ArrowUp: "ArrowUp",
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
    Enter: "Enter",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
  };
  return names[event.key] ?? null;
}

function acceleratorFromEvent(event) {
  const key = keyForAccelerator(event);
  if (!key) return null;
  const parts = [];
  if (event.metaKey) parts.push(IS_MAC ? "CommandOrControl" : "Super");
  if (event.ctrlKey) parts.push("Control");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(key);
  return [...new Set(parts)].join("+");
}

async function beginHotkeyCapture() {
  hotkeyCaptureActive = true;
  elements.hotkey.classList.add("capturing");
  elements.hotkey.placeholder = "Pulsa ahora la combinación…";
  if (currentHotkey && (await isRegistered(currentHotkey))) await unregister(currentHotkey);
}

async function finishHotkeyCapture(registerValue = true) {
  hotkeyCaptureActive = false;
  elements.hotkey.classList.remove("capturing");
  elements.hotkey.placeholder = "Pulsa una combinación";
  if (registerValue && elements.hotkey.value) await setGlobalHotkey(elements.hotkey.value);
}

async function refreshAccessibility(prompt = false) {
  if (!IS_MAC || !elements.autoPaste.checked) {
    elements.autoPasteHint.textContent = "Usa ⌘V o Ctrl+V en la aplicación activa.";
    elements.autoPasteHint.classList.remove("permission-error");
    return true;
  }
  const allowed = await invoke("accessibility_status", { prompt });
  elements.autoPasteHint.textContent = allowed
    ? "Accesibilidad activa; el texto se pegará automáticamente."
    : "Falta Accesibilidad: vuelve a activar AuralFlow en Privacidad y seguridad.";
  elements.autoPasteHint.classList.toggle("permission-error", !allowed);
  return allowed;
}

async function setSettingsOpen(open) {
  if (settingsOpen === open && elements.settingsPanel.hidden === !open) return;
  const position = await windowHandle.outerPosition();
  const scale = window.devicePixelRatio || 1;
  const deltaY = (SETTINGS_HEIGHT - COMPACT_HEIGHT) * scale;
  const deltaX = (SETTINGS_WIDTH - COMPACT_WIDTH) * scale;
  movingProgrammatically = true;
  if (open) compactAnchorPosition = position;
  settingsOpen = open;
  elements.settingsPanel.hidden = !open;
  elements.settingsButton.classList.toggle("active", open);
  await windowHandle.setSize(new LogicalSize(open ? SETTINGS_WIDTH : COMPACT_WIDTH, open ? SETTINGS_HEIGHT : COMPACT_HEIGHT));
  const nextPosition = open
    ? new PhysicalPosition(Math.round(position.x - deltaX / 2), Math.round(Math.max(0, position.y - deltaY)))
    : compactAnchorPosition ?? new PhysicalPosition(Math.round(position.x + deltaX / 2), Math.round(position.y + deltaY));
  await windowHandle.setPosition(nextPosition);
  if (!open) compactAnchorPosition = null;
  movingProgrammatically = false;
}

async function updateEngineUi() {
  const online = elements.engine.value === "groq";
  elements.groqSettings.hidden = !online;
  elements.localSettings.hidden = online;
  elements.modelActions.hidden = online;
  elements.modelProgress.hidden = online;
  elements.privacyPill.textContent = online ? "Groq · online" : "Local · privado";
  elements.privacyPill.classList.toggle("online", online);
  if (online) {
    let active = false;
    let keyStatusError = null;
    try {
      active = await invoke("groq_key_status");
    } catch (error) {
      keyStatusError = String(error);
    }
    elements.groqKeyStatus.textContent = active
      ? "Clave guardada de forma segura en el llavero del sistema. El audio se enviará a Groq."
      : keyStatusError ?? "Pega una clave. Se guardará cifrada en el llavero seguro de este dispositivo.";
    elements.clearGroqKey.disabled = !active;
  }
}

async function refreshModelStatus() {
  if (elements.engine.value === "groq") return true;
  const status = await invoke("model_status", { modelName: elements.modelName.value });
  elements.modelStatus.textContent = status.installed
    ? `${status.modelName} instalado`
    : `Falta ${status.modelName}`;
  elements.modelStatus.classList.toggle("ok", status.installed);
  elements.modelPath.textContent = status.path;
  elements.downloadButton.textContent = status.installed ? "Descargar de nuevo" : "Descargar";
  if (status.installed) void warmModel(status.modelName);
  return status.installed;
}

async function warmModel(modelName) {
  if (preparedModel === modelName) return;
  try {
    await invoke("prepare_model", { modelName });
    preparedModel = modelName;
  } catch (error) {
    if (!recording) showToast(String(error), true);
  }
}

async function setGlobalHotkey(hotkey) {
  if (currentHotkey && (await isRegistered(currentHotkey))) await unregister(currentHotkey);
  shortcutDown = false;
  await register(hotkey, async (event) => {
    if (!holdToTalkMode) {
      if (event.state === "Released") await toggleRecording();
      return;
    }
    if (event.state === "Pressed" && !shortcutDown) {
      shortcutDown = true;
      await handleHoldState(true);
    } else if (event.state === "Released" && shortcutDown) {
      shortcutDown = false;
      await handleHoldState(false);
    }
  });
  currentHotkey = hotkey;
  if (!recording) setStatus("ready", "Listo", IS_MAC && fnHoldEnabled ? "Mantén Fn para hablar" : `Mantén ${hotkey}`);
}

async function updatePreview() {
  if (!recording || previewInFlight) return;
  previewInFlight = true;
  try {
    const result = await invoke("preview_transcription");
    if (recording && result.text) showTranscript(result.text, true);
  } catch (_) {
    // La transcripción final sigue siendo la fuente de verdad.
  } finally {
    previewInFlight = false;
  }
}

function startPreviewLoop() {
  window.clearInterval(previewTimer);
  const interval = elements.engine.value === "groq"
    ? ONLINE_PREVIEW_INTERVAL_MS
    : LOCAL_PREVIEW_INTERVAL_MS;
  previewTimer = window.setInterval(updatePreview, interval);
}

function stopPreviewLoop() {
  window.clearInterval(previewTimer);
  previewTimer = null;
}

function renderAudioLevel(rawLevel = 0) {
  const normalized = Math.max(0, Math.min(1, (rawLevel - 0.008) * 13));
  smoothedLevel = normalized > smoothedLevel
    ? smoothedLevel * 0.38 + normalized * 0.62
    : smoothedLevel * 0.72 + normalized * 0.28;
  const speaking = smoothedLevel > 0.035;
  elements.meter.classList.toggle("speaking", speaking);
  const bars = elements.meter.querySelectorAll("i");
  bars.forEach((bar, index) => {
    const movement = speaking ? smoothedLevel * METER_WEIGHTS[index] : 0;
    bar.style.height = `${Math.round(3 + movement * 24)}px`;
  });
}

async function updateAudioLevel() {
  if (!recording || audioLevelInFlight) return;
  audioLevelInFlight = true;
  try {
    renderAudioLevel(await invoke("audio_level"));
  } catch (_) {
    renderAudioLevel(0);
  } finally {
    audioLevelInFlight = false;
  }
}

function startAudioLevelLoop() {
  window.clearInterval(audioLevelTimer);
  renderAudioLevel(0);
  audioLevelTimer = window.setInterval(updateAudioLevel, AUDIO_LEVEL_INTERVAL_MS);
}

function stopAudioLevelLoop() {
  window.clearInterval(audioLevelTimer);
  audioLevelTimer = null;
  renderAudioLevel(0);
}

async function beginRecording(source = "toggle") {
  if (busy || recording) return;
  busy = true;
  try {
    if (elements.engine.value === "groq") {
      if (!(await invoke("groq_key_status"))) {
        await setSettingsOpen(true);
        throw new Error("Añade una clave API de Groq para usar el modo online.");
      }
    } else {
      const installed = await refreshModelStatus();
      if (!installed) {
        await setSettingsOpen(true);
        throw new Error("Descarga primero un modelo local.");
      }
    }
    if (source === "hold" && !holdRequested) return;
    await invoke("start_recording");
    recording = true;
    elements.settingsButton.disabled = true;
    const engineLabel = elements.engine.value === "groq" ? "Groq online" : "Whisper local";
    setStatus("recording", "Escuchando", `${engineLabel} · suelta para terminar`);
    elements.transcript.textContent = "Habla con naturalidad…";
    elements.transcript.className = "transcript empty partial";
    startPreviewLoop();
    startAudioLevelLoop();
  } catch (error) {
    recording = false;
    stopPreviewLoop();
    stopAudioLevelLoop();
    setStatus("error", "No se pudo completar", String(error));
    showToast(String(error), true);
  } finally {
    busy = false;
    elements.settingsButton.disabled = recording;
    if (source === "hold" && recording && !holdRequested) void endRecording();
  }
}

async function endRecording() {
  if (busy || !recording) return;
  busy = true;
  recording = false;
  stopPreviewLoop();
  stopAudioLevelLoop();
  setStatus("processing", "Terminando", "Preparando el texto…");
  try {
    const result = await invoke("stop_and_transcribe");
    showTranscript(result.text, false);
    const pasteMessage = result.pasted ? " · pegado" : "";
    setStatus("ready", "Listo", `${result.elapsedMs} ms${pasteMessage}`);
    if (result.warning) showToast(result.warning, true);
  } catch (error) {
    setStatus("error", "No se pudo completar", String(error));
    showToast(String(error), true);
  } finally {
    busy = false;
    elements.settingsButton.disabled = false;
  }
}

async function handleHoldState(pressed) {
  holdRequested = pressed;
  if (pressed) await beginRecording("hold");
  else if (recording) await endRecording();
}

async function toggleRecording() {
  holdRequested = false;
  if (recording) await endRecording();
  else await beginRecording("toggle");
}

async function loadSettings() {
  const settings = await invoke("load_settings");
  elements.engine.value = settings.engine;
  elements.modelName.value = settings.modelName;
  elements.language.value = settings.language;
  elements.hotkey.value = settings.hotkey;
  holdToTalkMode = settings.holdToTalk ?? true;
  fnHoldEnabled = settings.fnHold ?? true;
  elements.holdToTalk.checked = holdToTalkMode;
  elements.fnHold.checked = fnHoldEnabled;
  elements.fnHoldRow.hidden = !IS_MAC;
  elements.platformHint.textContent = IS_MAC
    ? "Fn funciona de forma global y puede requerir Accesibilidad en macOS."
    : "Windows no expone Fn de forma estándar; usa el atajo configurable manteniéndolo pulsado.";
  elements.autoPaste.checked = settings.autoPaste;
  await refreshAccessibility(settings.autoPaste);
  await updateEngineUi();
  await setGlobalHotkey(settings.hotkey);
  await refreshModelStatus();
}

elements.recordButton.addEventListener("click", toggleRecording);
elements.settingsButton.addEventListener("click", () => setSettingsOpen(elements.settingsPanel.hidden));
elements.hideButton.addEventListener("click", () => windowHandle.hide());
elements.dragHandle.addEventListener("mousedown", async (event) => {
  if (event.button === 0) await windowHandle.startDragging();
});
elements.engine.addEventListener("change", updateEngineUi);
elements.hotkey.addEventListener("focus", () => {
  beginHotkeyCapture().catch((error) => showToast(String(error), true));
});
elements.hotkey.addEventListener("keydown", async (event) => {
  event.preventDefault();
  event.stopPropagation();
  if (event.key === "Escape") {
    await finishHotkeyCapture(true);
    elements.hotkey.blur();
    return;
  }
  const accelerator = acceleratorFromEvent(event);
  if (!accelerator) return;
  elements.hotkey.value = accelerator;
  await finishHotkeyCapture(true);
  elements.hotkey.blur();
  showToast(`Atajo: ${accelerator}`);
});
elements.hotkey.addEventListener("blur", () => {
  if (hotkeyCaptureActive) finishHotkeyCapture(true).catch((error) => showToast(String(error), true));
});
elements.holdToTalk.addEventListener("change", () => {
  holdToTalkMode = elements.holdToTalk.checked;
});
elements.fnHold.addEventListener("change", () => {
  fnHoldEnabled = elements.fnHold.checked;
});
elements.autoPaste.addEventListener("change", () => {
  refreshAccessibility(elements.autoPaste.checked).catch((error) => showToast(String(error), true));
});
elements.modelName.addEventListener("change", () => {
  preparedModel = null;
  refreshModelStatus();
});

elements.copyButton.addEventListener("click", async () => {
  try {
    await invoke("copy_text", { text: lastTranscript });
    showToast("Texto copiado.");
  } catch (error) {
    showToast(String(error), true);
  }
});

elements.saveButton.addEventListener("click", async () => {
  const settings = {
    engine: elements.engine.value,
    modelName: elements.modelName.value,
    language: elements.language.value,
    hotkey: elements.hotkey.value.trim(),
    holdToTalk: elements.holdToTalk.checked,
    fnHold: elements.fnHold.checked,
    autoPaste: elements.autoPaste.checked,
  };
  try {
    const key = elements.groqApiKey.value.trim();
    if (key) {
      await invoke("set_groq_api_key", { apiKey: key });
      elements.groqApiKey.value = "";
    }
    if (settings.engine === "groq" && !(await invoke("groq_key_status"))) {
      throw new Error("Añade una clave API de Groq.");
    }
    const accessibilityAllowed = settings.autoPaste
      ? await refreshAccessibility(true)
      : true;
    await invoke("save_settings", { settings });
    holdToTalkMode = settings.holdToTalk;
    fnHoldEnabled = settings.fnHold;
    await setGlobalHotkey(settings.hotkey);
    preparedModel = null;
    if (settings.engine === "local") void warmModel(settings.modelName);
    await setSettingsOpen(false);
    showToast(
      accessibilityAllowed ? "Preferencias guardadas." : "Guardado. Falta activar Accesibilidad para pegar.",
      !accessibilityAllowed,
    );
  } catch (error) {
    showToast(String(error), true);
  }
});

elements.clearGroqKey.addEventListener("click", async () => {
  await invoke("clear_groq_api_key");
  elements.groqApiKey.value = "";
  await updateEngineUi();
  showToast("Clave de Groq olvidada.");
});

elements.downloadButton.addEventListener("click", async () => {
  elements.downloadButton.disabled = true;
  elements.downloadProgress.style.width = "2%";
  try {
    await invoke("download_model", { modelName: elements.modelName.value });
    preparedModel = null;
    await refreshModelStatus();
    showToast("Modelo descargado.");
  } catch (error) {
    showToast(String(error), true);
  } finally {
    elements.downloadButton.disabled = false;
  }
});

await listen("model-download-progress", ({ payload }) => {
  const percent = payload.totalBytes
    ? Math.round((payload.downloadedBytes / payload.totalBytes) * 100)
    : 5;
  elements.downloadProgress.style.width = `${percent}%`;
  elements.modelStatus.textContent = `Descargando… ${percent}%`;
});

await listen("push-to-talk", ({ payload }) => {
  if (!IS_MAC || !fnHoldEnabled || !holdToTalkMode) return;
  void handleHoldState(payload?.state === "pressed");
});

await windowHandle.onMoved(() => {
  window.clearTimeout(rememberPosition.timeout);
  rememberPosition.timeout = window.setTimeout(() => rememberPosition().catch(() => {}), 250);
});

placeDock().then(loadSettings).catch((error) => {
  setStatus("error", "Error de inicio", String(error));
  showToast(String(error), true);
});
