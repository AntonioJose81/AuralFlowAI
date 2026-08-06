# AuralFlow

AuralFlow convierte voz en texto de forma local y pega el resultado en la aplicación activa. Esta versión reemplaza el prototipo Python/Gemini por una aplicación de escritorio Tauri 2 con núcleo Rust y `whisper.cpp`.

## Qué cambia en la versión 0.2

- La transcripción se ejecuta en el dispositivo: no requiere API key ni sube el audio.
- Funciona con modelos Whisper multilingües cuantizados.
- Captura el micrófono en memoria, mezcla canales y remuestrea a 16 kHz.
- Usa un atajo global configurable.
- Muestra texto parcial mientras se habla y reutiliza el modelo cargado en memoria.
- Funciona como una barra flotante compacta con acceso desde la bandeja del sistema.
- Guarda configuración y modelos en los directorios de usuario del sistema.
- Genera `.app`/`.dmg` en macOS y `.msi`/NSIS en Windows.
- Separa interfaz, audio, modelos, configuración, transcripción y pegado.

## Modelos disponibles

| Modelo | Uso recomendado |
| --- | --- |
| `tiny-q5_1` | Opción predeterminada; máxima velocidad y sólo unos 32 MB |
| `base-q5_1` | Buen equilibrio entre velocidad y precisión |
| `small-q5_1` | Más precisión, con mayor latencia |
| `large-v3-turbo-q5_0` | Equipos potentes; mayor precisión |

Los modelos se descargan bajo demanda desde el repositorio oficial usado por `whisper.cpp`. AuralFlow nunca incluye modelos dentro del repositorio Git.

## Requisitos de desarrollo

- Rust estable 1.85 o posterior.
- Node.js 20 o posterior.
- Dependencias de Tauri 2 para el sistema operativo.
- En macOS: Xcode Command Line Tools.
- En Windows: Microsoft C++ Build Tools y WebView2.

Consulta los [prerrequisitos oficiales de Tauri](https://v2.tauri.app/start/prerequisites/).

## Ejecutar en desarrollo

```bash
npm install
npm run tauri dev
```

La primera vez, abre **Preferencias** desde el engranaje y descarga un modelo. `tiny-q5_1` es la opción recomendada para dictado en vivo; `base-q5_1` mejora la precisión si el equipo mantiene una latencia aceptable.

## Pruebas y comprobaciones

```bash
npm run build
cd src-tauri
cargo fmt --check
cargo check
cargo test
```

## Crear instaladores

Los artefactos deben compilarse en su sistema de destino:

```bash
npm run tauri build
```

- Windows produce instaladores MSI y NSIS.
- macOS produce un paquete `.app` y un DMG.
- El workflow `build-desktop.yml` construye ambos sistemas desde GitHub Actions.

## Permisos

### macOS

AuralFlow solicita acceso al micrófono. Para pegar automáticamente en otras aplicaciones también necesita permiso en **Ajustes del Sistema → Privacidad y seguridad → Accesibilidad**. Una distribución pública debe firmarse y notarizarse.

### Windows

Windows puede impedir el pegado en programas ejecutados como administrador cuando AuralFlow no está elevado. Ésta es una protección del sistema, no un error de transcripción.

## Privacidad

- El audio sólo existe en memoria durante la grabación y transcripción.
- No se crean archivos WAV temporales.
- No hay telemetría ni llamadas a servicios de IA.
- La única descarga de red es el modelo solicitado por el usuario.

## Estructura

```text
src/                         Interfaz web local
src-tauri/src/audio.rs       Captura y remuestreo
src-tauri/src/model.rs       Descarga y almacenamiento de modelos
src-tauri/src/transcribe.rs  Inferencia Whisper
src-tauri/src/paste.rs       Portapapeles y pegado
src-tauri/src/config.rs      Preferencias por usuario
.github/workflows/           Verificación y builds de escritorio
```

## Estado conocido

Ésta es la primera base de la migración nativa. Antes de publicar una release deben validarse físicamente el micrófono, el permiso de Accesibilidad y el pegado en un Mac y un PC. También debe decidirse y corregirse la licencia del repositorio: la licencia Creative Commons heredada no se modifica automáticamente en esta migración.
