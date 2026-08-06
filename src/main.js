import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { register, unregister, isRegistered } from "@tauri-apps/plugin-global-shortcut";

const windowHandle = getCurrentWindow();
const COMPACT_HEIGHT = 176;
const SETTINGS_HEIGHT = 570;
const PREVIEW_INTERVAL_MS = 1800;

const elements = {
  recordButton: document.querySelector("#record-button"),
  statusTitle: document.querySelector("#status-title"),
  statusDetail: document.querySelector("#status-detail"),
  statusDot: document.querySelector("#status-dot"),
  meter: document.querySelector("#meter"),
  transcript: document.querySelector("#transcript"),
  copyButton: document.querySelector("#copy-button"),
  settingsButton: document.querySelector("#settings-button"),
  hideButton: document.querySelector("#hide-button"),
  settingsPanel: document.querySelector("#settings-panel"),
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
  elements.settingsPanel.hidden = !open;
  elements.settingsButton.classList.toggle("active", open);
  await windowHandle.setSize(new LogicalSize(520, open ? SETTINGS_HEIGHT : COMPACT_HEIGHT));
}

async function refreshModelStatus() {
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
  previewTimer = window.setInterval(updatePreview, PREVIEW_INTERVAL_MS);
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
      const installed = await refreshModelStatus();
      if (!installed) {
        await setSettingsOpen(true);
        throw new Error("Descarga primero un modelo local.");
      }
      const info = await invoke("start_recording");
      recording = true;
      elements.settingsButton.disabled = true;
      setStatus("recording", "Escuchando", `${info.deviceName} · transcripción en vivo`);
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
  elements.modelName.value = settings.modelName;
  elements.language.value = settings.language;
  elements.hotkey.value = settings.hotkey;
  elements.autoPaste.checked = settings.autoPaste;
  await setGlobalHotkey(settings.hotkey);
  await refreshModelStatus();
}

elements.recordButton.addEventListener("click", toggleRecording);
elements.settingsButton.addEventListener("click", () => setSettingsOpen(elements.settingsPanel.hidden));
elements.hideButton.addEventListener("click", () => windowHandle.hide());
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
    modelName: elements.modelName.value,
    language: elements.language.value,
    hotkey: elements.hotkey.value.trim(),
    autoPaste: elements.autoPaste.checked,
  };
  try {
    await invoke("save_settings", { settings });
    await setGlobalHotkey(settings.hotkey);
    preparedModel = null;
    void warmModel(settings.modelName);
    await setSettingsOpen(false);
    showToast("Preferencias guardadas.");
  } catch (error) {
    showToast(String(error), true);
  }
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

setSettingsOpen(false).catch(() => {});
loadSettings().catch((error) => {
  setStatus("error", "Error de inicio", String(error));
  showToast(String(error), true);
});
