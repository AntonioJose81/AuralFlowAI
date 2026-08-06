use keyring::{Entry, Error};

const SERVICE: &str = "com.antoniojose.auralflow";
const GROQ_ACCOUNT: &str = "groq-api-key";

fn groq_entry() -> Result<Entry, String> {
    Entry::new(SERVICE, GROQ_ACCOUNT)
        .map_err(|error| format!("no se pudo acceder al llavero del sistema: {error}"))
}

pub fn save_groq_key(api_key: &str) -> Result<(), String> {
    groq_entry()?
        .set_password(api_key)
        .map_err(|error| format!("no se pudo guardar la clave en el llavero: {error}"))
}

pub fn load_groq_key() -> Result<Option<String>, String> {
    match groq_entry()?.get_password() {
        Ok(api_key) => Ok(Some(api_key)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("no se pudo leer la clave del llavero: {error}")),
    }
}

pub fn delete_groq_key() -> Result<(), String> {
    match groq_entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("no se pudo borrar la clave del llavero: {error}")),
    }
}
