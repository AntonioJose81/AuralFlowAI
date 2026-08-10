use std::{thread, time::Duration};

use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};

use crate::error::{AuralFlowError, Result};

pub fn copy_text(text: &str) -> Result<()> {
    let mut clipboard =
        Clipboard::new().map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))
}

pub fn paste_text(text: &str) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    copy_text(text)?;
    thread::sleep(Duration::from_millis(150));

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(target_os = "windows")]
    let modifier = Key::Control;

    enigo
        .key(modifier, Press)
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    thread::sleep(Duration::from_millis(20));
    let paste_result = enigo.key(Key::Unicode('v'), Click);
    thread::sleep(Duration::from_millis(20));
    let release_result = enigo.key(modifier, Release);
    paste_result
        .and(release_result)
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    Ok(())
}

pub fn accessibility_status(prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        let settings = Settings {
            open_prompt_to_get_permissions: prompt,
            ..Settings::default()
        };
        Enigo::new(&settings).is_ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = prompt;
        true
    }
}
