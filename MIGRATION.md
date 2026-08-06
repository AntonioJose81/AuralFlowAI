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
| API key en texto plano | Whisper local sin clave o clave Groq en el llavero seguro del sistema |
| PyInstaller sólo Windows | Instaladores Tauri para macOS y Windows |

## Interacción compacta

La interfaz 0.2 usa un dock flotante de 400 × 96, colocado inicialmente sobre la barra de tareas, movible y con icono en la bandeja del sistema. Durante la grabación procesa vistas de los últimos segundos y muestra texto parcial; al detenerse vuelve a transcribir el audio completo para obtener el resultado definitivo.

Whisper Tiny sigue siendo el motor local predeterminado. El usuario puede activar Groq `whisper-large-v3-turbo` para conseguir menor latencia mediante una API compatible con Whisper. Esa elección es explícita porque implica enviar el audio al servicio online.

En macOS, Enigo está fijado temporalmente al commit `c041408b4f9f0b96bf2c10d79f782ede146b5634`. La versión publicada 0.6.1 consultaba la distribución del teclado fuera del hilo principal y podía cerrar AuralFlow al ejecutar `⌘V`; el commit fijado deriva esa consulta al hilo principal.

## Datos que no se migran

- La antigua API key de Gemini no se copia ni se lee.
- `output.wav` no se utiliza.
- Los nombres de modelos Gemini obsoletos se descartan.
- El prompt que mezclaba transcripción y corrección no se aplica, para evitar alteraciones del dictado.

## Posibles extensiones

Una fase posterior puede añadir corrección opcional como un paso separado, detrás de una interfaz `TextEnhancer`. Debe estar desactivada por defecto y distinguir claramente procesamiento local y procesamiento en la nube.
