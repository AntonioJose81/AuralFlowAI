# Migración desde el prototipo Python

El archivo `app.py` original servía para validar la idea, pero no se utiliza en la arquitectura 0.2.

| Prototipo | Versión 0.2 |
| --- | --- |
| CustomTkinter | Tauri 2 + HTML/CSS |
| PyAudio y archivo WAV | CPAL y audio en memoria |
| Gemini 1.5 | Whisper local mediante `whisper-rs` |
| `keyboard` | Plugin global-shortcut de Tauri |
| `pyperclip`/PyAutoGUI | `arboard` + `enigo` |
| `config.ini` junto al ejecutable | JSON en el directorio de configuración del usuario |
| API key en texto plano | No se requiere API key |
| PyInstaller sólo Windows | Instaladores Tauri para macOS y Windows |

## Datos que no se migran

- La antigua API key de Gemini no se copia ni se lee.
- `output.wav` no se utiliza.
- Los nombres de modelos Gemini obsoletos se descartan.
- El prompt que mezclaba transcripción y corrección no se aplica, para evitar alteraciones del dictado.

## Posibles extensiones

Una fase posterior puede añadir corrección opcional como un paso separado, detrás de una interfaz `TextEnhancer`. Debe estar desactivada por defecto y distinguir claramente procesamiento local y procesamiento en la nube.
