use std::{thread, time::Duration};

use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};

use crate::error::{AuralFlowError, Result};

pub fn copy_text(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new()
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))
}

pub fn paste_text(text: &str) -> Result<()> {
    copy_text(text)?;
    thread::sleep(Duration::from_millis(100));
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Command;
    #[cfg(target_os = "windows")]
    let modifier = Key::Control;

    enigo
        .key(modifier, Press)
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    let paste_result = enigo.key(Key::Unicode('v'), Click);
    let release_result = enigo.key(modifier, Release);
    paste_result
        .and(release_result)
        .map_err(|error| AuralFlowError::Clipboard(error.to_string()))?;
    Ok(())
}
