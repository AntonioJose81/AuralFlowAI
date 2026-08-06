import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  register,
  unregister,
  isRegistered,
} from "@tauri-apps/plugin-global-shortcut";

const elements = {
  recordButton: document.querySelector("#record-button"),
  recordLabel: document.querySelector("#record-label"),
  statusTitle: document.querySelector(".status-title"),
  statusDetail: document.querySelector("#status-detail"),
  statusDot: document.querySelector("#status-dot"),
  meter: document.querySelector("#meter"),
  transcript: document.querySelector("#transcript"),
  copyButton: document.querySelector("#copy-button"),
  shortcutHint: document.querySelector("#shortcut-hint"),
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
let currentHotkey = null;
let lastTranscript = "";

function setStatus(kind, title, detail) {
  elements.statusTitle.textContent = title;
  elements.statusDetail.textContent = detail;
  elements.statusDot.dataset.state = kind;
  elements.meter.classList.toggle("active", kind === "recording");
  elements.recordButton.classList.toggle("recording", kind === "recording");
  elements.recordButton.disabled = kind === "processing";
  elements.recordLabel.textContent =
    kind === "recording" ? "Detener y transcribir" : "Empezar a grabar";
}

function showToast(message, isError = false) {
  elements.toast.textContent = message;
  elements.toast.classList.toggle("error", isError);
  elements.toast.classList.add("visible");
  window.clearTimeout(showToast.timeout);
  showToast.timeout = window.setTimeout(() => elements.toast.classList.remove("visible"), 3200);
}

async function refreshModelStatus() {
  const status = await invoke("model_status", { modelName: elements.modelName.value });
  elements.modelStatus.textContent = status.installed
    ? `Modelo ${status.modelName} instalado`
    : `Falta el modelo ${status.modelName}`;
  elements.modelStatus.classList.toggle("ok", status.installed);
  elements.modelPath.textContent = status.path;
  elements.downloadButton.textContent = status.installed ? "Volver a descargar" : "Descargar modelo";
  return status.installed;
}

async function setGlobalHotkey(hotkey) {
  if (currentHotkey && (await isRegistered(currentHotkey))) {
    await unregister(currentHotkey);
  }
  await register(hotkey, async (event) => {
    if (event.state === "Released") await toggleRecording();
  });
  currentHotkey = hotkey;
  elements.shortcutHint.textContent = `Atajo: ${hotkey}`;
}

async function toggleRecording() {
  if (busy) return;
  busy = true;
  try {
    if (!recording) {
      const installed = await refreshModelStatus();
      if (!installed) {
        document.querySelector(".settings-panel").open = true;
        throw new Error("Descarga primero el modelo de transcripción.");
      }
      const info = await invoke("start_recording");
      recording = true;
      setStatus("recording", "Grabando…", `${info.deviceName} · ${info.sampleRate} Hz`);
    } else {
      recording = false;
      setStatus("processing", "Transcribiendo…", "El audio se procesa localmente en este equipo.");
      const result = await invoke("stop_and_transcribe");
      lastTranscript = result.text;
      elements.transcript.textContent = result.text;
      elements.transcript.classList.remove("empty");
      elements.copyButton.disabled = false;
      const pasteMessage = result.pasted ? " y pegado" : "";
      setStatus("ready", "Transcripción lista", `Procesado en ${result.elapsedMs} ms${pasteMessage}.`);
      if (result.warning) showToast(result.warning, true);
    }
  } catch (error) {
    recording = false;
    setStatus("error", "No se pudo completar", String(error));
    showToast(String(error), true);
  } finally {
    busy = false;
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
elements.modelName.addEventListener("change", refreshModelStatus);

elements.copyButton.addEventListener("click", async () => {
  try {
    await invoke("copy_text", { text: lastTranscript });
    showToast("Texto copiado al portapapeles.");
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
    await refreshModelStatus();
    showToast("Modelo descargado correctamente.");
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

loadSettings().catch((error) => {
  setStatus("error", "Error de inicio", String(error));
  showToast(String(error), true);
});
