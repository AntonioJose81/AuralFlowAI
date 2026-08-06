import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition, LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { register, unregister, isRegistered } from "@tauri-apps/plugin-global-shortcut";

const windowHandle = getCurrentWindow();
const COMPACT_WIDTH = 400;
const COMPACT_HEIGHT = 96;
const SETTINGS_HEIGHT = 650;
const LOCAL_PREVIEW_INTERVAL_MS = 1800;
const ONLINE_PREVIEW_INTERVAL_MS = 6000;
const POSITION_STORAGE_KEY = "auralflow-dock-position";

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
  autoPaste: document.querySelector("#auto-paste"),
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
let currentHotkey = null;
let lastTranscript = "";
let preparedModel = null;
let settingsOpen = false;
let movingProgrammatically = false;
let compactAnchorPosition = null;

async function placeDock() {
  await windowHandle.setSize(new LogicalSize(COMPACT_WIDTH, COMPACT_HEIGHT));
  const saved = window.localStorage.getItem(POSITION_STORAGE_KEY);
  if (saved) {
    try {
      const position = JSON.parse(saved);
      if (Number.isFinite(position.x) && Number.isFinite(position.y)) {
        await windowHandle.setPosition(new PhysicalPosition(position.x, position.y));
        return;
      }
    } catch (_) {
      window.localStorage.removeItem(POSITION_STORAGE_KEY);
    }
  }
  const left = (window.screen.availLeft ?? 0) + window.screen.availWidth - COMPACT_WIDTH - 12;
  const top = (window.screen.availTop ?? 0) + window.screen.availHeight - COMPACT_HEIGHT - 12;
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
  elements.meter.classList.toggle("active", kind === "recording");
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

async function setSettingsOpen(open) {
  if (settingsOpen === open && elements.settingsPanel.hidden === !open) return;
  const position = await windowHandle.outerPosition();
  const scale = window.devicePixelRatio || 1;
  const delta = (SETTINGS_HEIGHT - COMPACT_HEIGHT) * scale;
  movingProgrammatically = true;
  if (open) compactAnchorPosition = position;
  settingsOpen = open;
  elements.settingsPanel.hidden = !open;
  elements.settingsButton.classList.toggle("active", open);
  await windowHandle.setSize(new LogicalSize(COMPACT_WIDTH, open ? SETTINGS_HEIGHT : COMPACT_HEIGHT));
  const nextPosition = open
    ? new PhysicalPosition(position.x, Math.round(Math.max(0, position.y - delta)))
    : compactAnchorPosition ?? new PhysicalPosition(position.x, Math.round(position.y + delta));
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
    const active = await invoke("groq_key_status");
    elements.groqKeyStatus.textContent = active
      ? "Clave activa en esta sesión. El audio se enviará a Groq."
      : "Pega una clave. Solo vivirá en memoria durante esta sesión; no se guarda en disco.";
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
  await register(hotkey, async (event) => {
    if (event.state === "Released") await toggleRecording();
  });
  currentHotkey = hotkey;
  if (!recording) setStatus("ready", "Listo", hotkey);
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

async function toggleRecording() {
  if (busy) return;
  busy = true;
  try {
    if (!recording) {
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
      const info = await invoke("start_recording");
      recording = true;
      elements.settingsButton.disabled = true;
      const engineLabel = elements.engine.value === "groq" ? "Groq online" : "Whisper local";
      setStatus("recording", "Escuchando", `${engineLabel} · ${info.deviceName}`);
      elements.transcript.textContent = "Habla con naturalidad…";
      elements.transcript.className = "transcript empty partial";
      startPreviewLoop();
    } else {
      recording = false;
      stopPreviewLoop();
      setStatus("processing", "Terminando", "Preparando el texto final…");
      const result = await invoke("stop_and_transcribe");
      showTranscript(result.text, false);
      const pasteMessage = result.pasted ? " · pegado" : "";
      setStatus("ready", "Listo", `${result.elapsedMs} ms${pasteMessage}`);
      if (result.warning) showToast(result.warning, true);
    }
  } catch (error) {
    recording = false;
    stopPreviewLoop();
    setStatus("error", "No se pudo completar", String(error));
    showToast(String(error), true);
  } finally {
    busy = false;
    elements.settingsButton.disabled = recording;
  }
}

async function loadSettings() {
  const settings = await invoke("load_settings");
  elements.engine.value = settings.engine;
  elements.modelName.value = settings.modelName;
  elements.language.value = settings.language;
  elements.hotkey.value = settings.hotkey;
  elements.autoPaste.checked = settings.autoPaste;
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
    await invoke("save_settings", { settings });
    await setGlobalHotkey(settings.hotkey);
    preparedModel = null;
    if (settings.engine === "local") void warmModel(settings.modelName);
    await setSettingsOpen(false);
    showToast("Preferencias guardadas.");
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

await windowHandle.onMoved(() => {
  window.clearTimeout(rememberPosition.timeout);
  rememberPosition.timeout = window.setTimeout(() => rememberPosition().catch(() => {}), 250);
});

placeDock().then(loadSettings).catch((error) => {
  setStatus("error", "Error de inicio", String(error));
  showToast(String(error), true);
});
