use keyring::Entry;

const SERVICE_NAME: &str = "CinaVault Premium";

/// Saves a secret and reads it back through a fresh entry, so a store that
/// accepts the write but does not keep it (keyring's in-memory mock, a locked
/// or unavailable keychain) is reported instead of losing the key.
pub fn set(provider: &str, secret: &str) -> Result<(), String> {
    Entry::new(SERVICE_NAME, provider)
        .map_err(|error| error.to_string())?
        .set_password(secret)
        .map_err(|error| error.to_string())?;
    match get(provider)? {
        Some(stored) if stored == secret => Ok(()),
        _ => Err(format!(
            "The system keychain did not keep the {provider} key, so it was not saved"
        )),
    }
}

pub fn get(provider: &str) -> Result<Option<String>, String> {
    match Entry::new(SERVICE_NAME, provider)
        .map_err(|error| error.to_string())?
        .get_password()
    {
        Ok(secret) if !secret.trim().is_empty() => Ok(Some(secret)),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub fn delete(provider: &str) -> Result<(), String> {
    match Entry::new(SERVICE_NAME, provider)
        .map_err(|error| error.to_string())?
        .delete_credential()
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) const SECURE_STORE_MARKER: &str = "__secure_store__";

#[cfg(test)]
mod tests {
    #[test]
    fn service_name_is_not_a_portable_config_path() {
        assert!(!super::SERVICE_NAME.contains(".json"));
    }
}
