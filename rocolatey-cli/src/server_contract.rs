pub fn validate_schema_version(actual: u32, endpoint: &str) -> Result<(), String> {
    if actual == rocolatey_lib::server::ROCO_SERVER_SCHEMA_VERSION {
        return Ok(());
    }

    Err(format!(
        "Unsupported server schema version {} for {} (expected {})",
        actual,
        endpoint,
        rocolatey_lib::server::ROCO_SERVER_SCHEMA_VERSION
    ))
}

#[cfg(test)]
mod tests {
    use super::validate_schema_version;

    #[test]
    fn accepts_current_schema_version() {
        assert!(validate_schema_version(1, "/test").is_ok());
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let err = validate_schema_version(999, "/test").expect_err("schema should fail");
        assert!(err.contains("Unsupported server schema version"));
    }
}