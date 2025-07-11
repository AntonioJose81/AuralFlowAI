# app.py (Versión Final con Solución de Portapapeles)
import customtkinter as ctk
import pyaudio
import wave
import numpy as np
import threading
import google.generativeai as genai
import pyautogui
import configparser
import os
import sys
import tkinter.messagebox as messagebox
import certifi
import logging
import keyboard
import queue
import webbrowser
import time
import pyperclip # <-- LIBRERÍA NUEVA Y CLAVE

# --- CONFIGURACIÓN DE PATHS Y LOGGING ---
if getattr(sys, 'frozen', False):
    application_path = os.path.dirname(sys.executable)
    os.environ['SSL_CERT_FILE'] = os.path.join(sys._MEIPASS, 'certifi', 'cacert.pem')
else:
    application_path = os.path.dirname(os.path.abspath(__file__))

log_file_path = os.path.join(application_path, 'app.log')
config_file_path = os.path.join(application_path, 'config.ini')
logging.basicConfig(filename=log_file_path, level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s', encoding='utf-8')

class SettingsWindow(ctk.CTkToplevel):
    # --- SIN CAMBIOS EN LA VENTANA DE CONFIGURACIÓN ---
    def __init__(self, master):
        super().__init__(master)
        # ... (Pega aquí la clase SettingsWindow completa de la respuesta anterior) ...
        self.master = master; self.title("Configuración"); self.geometry("600x550")
        self.transient(master); self.attributes("-topmost", True); self.config = master.config
        self.api_key_label = ctk.CTkLabel(self, text="Clave de API de Google Gemini (requerido):"); self.api_key_label.pack(pady=(10, 5))
        self.api_key_entry = ctk.CTkEntry(self, width=460); self.api_key_entry.insert(0, self.config.get('GEMINI_API', 'api_key', fallback='')); self.api_key_entry.pack(pady=5, padx=20)
        self.link_label = ctk.CTkLabel(self, text="¿No tienes una clave? Consíguela aquí", text_color="#60a5fa", cursor="hand2"); self.link_label.pack()
        self.link_label.bind("<Button-1>", lambda e: webbrowser.open_new("https://aistudio.google.com/"))
        self.model_label = ctk.CTkLabel(self, text="Modelo de Gemini:"); self.model_label.pack(pady=(20, 5))
        available_models = self.config.get('MODELS', 'available_models', fallback='').split(',')
        self.model_menu = ctk.CTkOptionMenu(self, values=[m.strip() for m in available_models if m.strip()]); self.model_menu.set(self.config.get('GEMINI_API', 'selected_model')); self.model_menu.pack(pady=5, padx=20)
        self.prompt_label = ctk.CTkLabel(self, text="Prompt de Transcripción y Mejora:"); self.prompt_label.pack(pady=(20, 5))
        self.prompt_textbox = ctk.CTkTextbox(self, height=150, width=560); self.prompt_textbox.pack(pady=5, padx=20, fill="x", expand=True); self.prompt_textbox.insert("1.0", self.config.get('PROMPTS', 'enhancer_prompt'))
        self.hotkey_label = ctk.CTkLabel(self, text="Atajo Iniciar/Detener:"); self.hotkey_label.pack(pady=(10, 5))
        self.hotkey_entry = ctk.CTkEntry(self, width=200); self.hotkey_entry.insert(0, self.config.get('HOTKEYS', 'start_stop_hotkey')); self.hotkey_entry.pack(pady=5)
        self.save_button = ctk.CTkButton(self, text="Guardar y Cerrar", command=self.save_and_close); self.save_button.pack(pady=20)

    def save_and_close(self):
        api_key = self.api_key_entry.get().strip()
        if not api_key: messagebox.showerror("Error", "La clave API es obligatoria.", parent=self); return
        self.config.set('GEMINI_API', 'api_key', api_key); self.config.set('GEMINI_API', 'selected_model', self.model_menu.get())
        self.config.set('PROMPTS', 'enhancer_prompt', self.prompt_textbox.get("1.0", "end-1c").strip())
        self.config.set('HOTKEYS', 'start_stop_hotkey', self.hotkey_entry.get())
        with open(config_file_path, 'w', encoding='utf-8') as configfile: self.config.write(configfile)
        self.master.reload_config(); self.destroy()

class AuralFlowApp(ctk.CTk):
    def __init__(self, config):
        super().__init__()
        self.config = config
        self.pyaudio_instance = pyaudio.PyAudio()
        self.reload_config()
        self.title("AuralFlow"); self.geometry("300x180"); self.attributes("-topmost", True)
        ctk.set_appearance_mode("dark")
        self.is_recording = False
        self.audio_queue = queue.Queue(maxsize=1); self.result_queue = queue.Queue()
        self.status_label = ctk.CTkLabel(self, text="", font=("Arial", 14)); self.status_label.pack(pady=10)
        self.update_status_label()
        self.canvas = ctk.CTkCanvas(self, height=80, bg="#2B2B2B", highlightthickness=0); self.canvas.pack(fill="x", padx=10)
        self.settings_button = ctk.CTkButton(self, text="Configuración ⚙️", command=self.open_settings_window); self.settings_button.pack(pady=(10, 10), side="bottom")
        self.protocol("WM_DELETE_WINDOW", self.on_closing); self.check_result_queue()

    def on_closing(self):
        self.is_recording = False; time.sleep(0.1)
        keyboard.unhook_all(); self.pyaudio_instance.terminate(); self.destroy()

    def check_result_queue(self):
        try:
            result = self.result_queue.get_nowait()
            if isinstance(result, Exception):
                messagebox.showerror("Error", f"{result}")
            else:
                threading.Thread(target=self.write_text_thread, args=(result,), daemon=True).start()
        except queue.Empty: pass
        finally:
            self.after(100, self.check_result_queue)

    def write_text_thread(self, text_to_write):
        """Usa el portapapeles para pegar el texto de forma segura y fiable."""
        try:
            # Guardar contenido original del portapapeles para no molestar al usuario
            original_clipboard = pyperclip.paste()
        except pyperclip.PyperclipException:
            original_clipboard = ""

        pyperclip.copy(text_to_write)
        time.sleep(0.1) # Pequeña pausa para asegurar que el SO ha copiado el texto
        pyautogui.hotkey('ctrl', 'v')
        time.sleep(0.1)

        # Restaurar el portapapeles original
        pyperclip.copy(original_clipboard)
        
        # Pedir a la GUI que se resetee de forma segura
        self.after(0, self.reset_ui)

    # --- El resto de funciones no cambian ---
    def update_status_label(self):
        if not self.api_key: self.status_label.configure(text="Configuración Requerida", text_color="yellow")
        else: self.status_label.configure(text="Presiona el atajo para grabar", text_color="white")
    def setup_hotkeys(self):
        keyboard.unhook_all()
        hotkey = self.config.get('HOTKEYS', 'start_stop_hotkey', fallback='ctrl+alt+r')
        try: keyboard.add_hotkey(hotkey, self.toggle_recording)
        except: messagebox.showerror("Error de Atajo", f"No se pudo registrar '{hotkey}'.\nEjecuta como administrador.")
    def reload_config(self):
        self.config.read(config_file_path, encoding='utf-8')
        self.api_key = self.config.get('GEMINI_API', 'api_key', fallback='')
        selected_model = self.config.get('GEMINI_API', 'selected_model')
        self.master_prompt = self.config.get('PROMPTS', 'enhancer_prompt')
        self.audio_filename = os.path.join(application_path, self.config['AUDIO']['filename'])
        self.samplerate = int(self.config['AUDIO']['samplerate'])
        self.CHUNK = 1024; self.FORMAT = pyaudio.paInt16; self.CHANNELS = 1
        if self.api_key: genai.configure(api_key=self.api_key); self.model = genai.GenerativeModel(selected_model)
        self.setup_hotkeys()
        if hasattr(self, 'status_label'): self.update_status_label()
    def open_settings_window(self):
        if not (hasattr(self, 'settings_win') and self.settings_win.winfo_exists()): self.settings_win = SettingsWindow(self)
        self.settings_win.focus()
    def toggle_recording(self):
        if not self.api_key: messagebox.showerror("Configuración Requerida", "Añade tu clave API en Configuración."); return
        self.is_recording = not self.is_recording
        if self.is_recording:
            self.status_label.configure(text="🔴 Grabando...", text_color="red")
            threading.Thread(target=self.recording_worker, daemon=True).start()
        else:
            self.status_label.configure(text="Procesando...", text_color="yellow")
    def recording_worker(self):
        self.frames = []
        stream = self.pyaudio_instance.open(format=self.FORMAT, channels=self.CHANNELS, rate=self.samplerate, input=True, frames_per_buffer=self.CHUNK)
        while self.is_recording:
            data = stream.read(self.CHUNK); self.frames.append(data)
            numpy_data = np.frombuffer(data, dtype=np.int16)
            self.after(0, self.update_waveform, numpy_data)
        stream.stop_stream(); stream.close()
        self.process_audio()
    def update_waveform(self, audio_data):
        if not self.is_recording: self.canvas.delete("all"); return
        self.canvas.delete("all")
        normalized_data = audio_data / 32768.0; h, w = self.canvas.winfo_height(), self.canvas.winfo_width()
        if h <= 1 or w <= 1: return
        points = normalized_data[::5]; scaled = (points * (h/2)) + (h/2)
        coords = [i for p in zip(np.linspace(0, w, len(scaled)), scaled) for i in p]
        if len(coords) > 2: self.canvas.create_line(coords, fill="#1F6AA5", width=1.5)
    def process_audio(self):
        if not self.frames: self.after(0, self.reset_ui); return
        with wave.open(self.audio_filename, 'wb') as wf:
            wf.setnchannels(self.CHANNELS); wf.setsampwidth(self.pyaudio_instance.get_sample_size(self.FORMAT))
            wf.setframerate(self.samplerate); wf.writeframes(b''.join(self.frames))
        threading.Thread(target=self.api_call_thread, daemon=True).start()
    def api_call_thread(self):
        audio_file = None
        try:
            audio_file = genai.upload_file(path=self.audio_filename)
            response = self.model.generate_content([self.master_prompt, audio_file], request_options={'timeout': 600})
            final_text = response.text.strip()
            if not final_text: raise Exception("La IA devolvió una respuesta vacía.")
            self.result_queue.put(final_text)
        except Exception as e: self.result_queue.put(e)
        finally:
            if audio_file:
                try: genai.delete_file(audio_file.name)
                except Exception: pass
    def reset_ui(self): self.update_status_label()

class FirstTimeSetupApp(ctk.CTk):
    # ... (Sin cambios en esta clase) ...
    def __init__(self, config):
        super().__init__()
        self.config = config; self.title("Configuración Inicial de AuralFlow"); self.geometry("500x250")
        self.label = ctk.CTkLabel(self, text="¡Bienvenido! Introduce tu clave de API de Google Gemini para empezar.", wraplength=480); self.label.pack(pady=20, padx=20)
        self.api_entry = ctk.CTkEntry(self, placeholder_text="Pega tu clave API aquí...", width=460); self.api_entry.pack(pady=10)
        self.link_label = ctk.CTkLabel(self, text="¿No tienes una clave? Consíguela aquí (es gratis)", text_color="#60a5fa", cursor="hand2"); self.link_label.pack()
        self.link_label.bind("<Button-1>", lambda e: webbrowser.open_new("https://aistudio.google.com/"))
        self.save_button = ctk.CTkButton(self, text="Guardar y Lanzar AuralFlow", command=self.save_and_launch); self.save_button.pack(pady=20)
        self.protocol("WM_DELETE_WINDOW", self.on_closing)
    def save_and_launch(self):
        api_key = self.api_entry.get().strip()
        if not api_key: messagebox.showerror("Error", "El campo no puede estar vacío."); return
        self.config.set('GEMINI_API', 'api_key', api_key)
        with open(config_file_path, 'w', encoding='utf-8') as configfile: self.config.write(configfile)
        self.destroy(); os.startfile(sys.executable)
    def on_closing(self):
        if messagebox.askokcancel("Salir", "¿Seguro que quieres salir?"): self.destroy()

if __name__ == "__main__":
    config = configparser.ConfigParser()
    if os.path.exists(config_file_path): config.read(config_file_path, encoding='utf-8')
    else: config.add_section('GEMINI_API')
    
    api_key = config.get('GEMINI_API', 'api_key', fallback='').strip()
    if not api_key:
        FirstTimeSetupApp(config).mainloop()
    else:
        AuralFlowApp(config).mainloop()