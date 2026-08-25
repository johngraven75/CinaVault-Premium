use keyring::Entry;

const SERVICE_NAME: &str = "CinaVault Premium";

pub fn set(provider: &str, secret: &str) -> Result<(), String> {
    Entry::new(SERVICE_NAME, provider)
        .map_err(|error| error.to_string())?
        .set_password(secret)
        .map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    #[test]
    fn service_name_is_not_a_portable_config_path() {
        assert!(!super::SERVICE_NAME.contains(".json"));
    }
}

pub(crate) const SECURE_STORE_MARKER: &str = "__secure_store__";
