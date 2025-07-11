# AuralFlowAI
 Una aplicación de escritorio ligera para Windows que transcribe y formatea tu voz a texto en cualquier aplicación, potenciada por Google Gemini. Tu asistente de dictado personal y personalizable.

 ---

## 🛠️ Guía de Instalación y Compilación (Para Desarrolladores y Entusiastas)

¿Quieres modificar el código, añadir nuevas funciones o simplemente compilar tu propia versión de AuralFlow? ¡Genial! Sigue estos pasos.

### **Requisitos Previos**

-   **Python:** Asegúrate de tener instalado Python 3.10 o una versión superior. Puedes descargarlo desde [python.org](https://www.python.org/downloads/).
-   **Git:** Necesario para clonar el repositorio. Puedes descargarlo desde [git-scm.com](https://git-scm.com/downloads).

---

### **Paso 1: Obtener el Código Fuente**

Abre una terminal o `CMD` y clona este repositorio en tu ordenador:

```bash
git clone https://github.com/TU_USUARIO/AuralFlow.git
```

Luego, navega a la carpeta del proyecto:

```bash
cd AuralFlow
```

---

### **Paso 2: Crear un Entorno Virtual (Práctica Recomendada)**

Es muy recomendable trabajar en un entorno virtual para no instalar las librerías en tu sistema global.

1.  **Crea el entorno:**
    ```bash
    python -m venv venv
    ```

2.  **Activa el entorno:**
    *   En Windows (CMD/PowerShell):
        ```bash
        venv\Scripts\activate
        ```
    *   En macOS/Linux:
        ```bash
        source venv/bin/activate
        ```
    Verás `(venv)` al principio de la línea de tu terminal, indicando que el entorno está activo.

---

### **Paso 3: Instalar las Dependencias**

Con el entorno virtual activado, instala todas las librerías que AuralFlow necesita con un solo comando:

```bash
pip install -r requirements.txt
```
*(Este comando lee el archivo `requirements.txt` e instala automáticamente `customtkinter`, `pyaudio`, `keyboard`, etc.)*

---

### **Paso 4: Configurar tu API Key**

1.  Abre el archivo `config.ini` con un editor de texto.
2.  Busca la línea `api_key =`.
3.  Pega tu clave de API de Google Gemini después del signo `=`.
    > 💡 Si no tienes una, puedes obtenerla gratis en [Google AI Studio](https://aistudio.google.com/). Recuerda que necesitas habilitar la facturación en tu proyecto de Google Cloud para que la API funcione.

---

### **Paso 5: Probar el Script**

Antes de compilar, asegúrate de que todo funciona correctamente ejecutando el script de Python:

```bash
python app.py
```
La aplicación debería abrirse y ser completamente funcional.

---

### **Paso 6: Compilar tu Propio Archivo `.exe`**

Para crear un archivo ejecutable autocontenido (`.exe`) que puedas compartir o usar sin necesidad de tener Python instalado, usaremos la herramienta PyInstaller.

1.  **Instala PyInstaller** (si aún no lo tienes):
    ```bash
    pip install pyinstaller
    ```

2.  **Ejecuta el comando de compilación:**
    Este comando está optimizado para incluir todos los archivos necesarios y evitar errores comunes.
    ```bash
    pyinstaller --onefile --windowed --add-data "config.ini;." --collect-data certifi app.py
    ```

    *   `--onefile`: Empaqueta todo en un único archivo `.exe`.
    *   `--windowed`: Oculta la ventana de la terminal al ejecutar la aplicación.
    *   `--add-data "config.ini;."`: Asegura que tu archivo de configuración se incluya en el paquete.
    *   `--collect-data certifi`: Soluciona el problema de los certificados SSL para las llamadas a la API.

3.  **¡Listo! Encuentra tu `.exe`**
    Una vez que el proceso termine, se habrá creado una carpeta llamada `dist`. Dentro de ella, encontrarás tu `app.exe`, listo para ser usado y distribuido.

---
