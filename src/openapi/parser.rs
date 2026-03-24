use openapiv3::OpenAPI;

/// Parse an OpenAPI spec from a string (JSON or YAML).
///
/// JSON is tried first; if that fails the content is re-parsed as YAML.
///
/// # Errors
///
/// Returns an error when neither JSON nor YAML parsing succeeds.
///
/// # Examples
///
/// ```no_run
/// let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
/// let spec = stampo::openapi::parser::parse_spec(&content).unwrap();
/// assert_eq!(spec.openapi, "3.0.0");
/// ```
pub fn parse_spec(content: &str) -> Result<OpenAPI, anyhow::Error> {
    // Try JSON first, then YAML.
    if let Ok(spec) = serde_json::from_str::<OpenAPI>(content) {
        return Ok(spec);
    }
    let spec: OpenAPI = serde_saphyr::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse OpenAPI spec: {e}"))?;
    Ok(spec)
}

/// Validate that the spec is OpenAPI 3.x (not Swagger 2.0).
///
/// # Errors
///
/// Returns an error when the `openapi` version field does not start with `"3."`.
pub fn validate_version(spec: &OpenAPI) -> Result<(), anyhow::Error> {
    if !spec.openapi.starts_with("3.") {
        anyhow::bail!(
            "Swagger 2.0 is not yet supported. Please use an OpenAPI 3.0+ spec. Found version: {}",
            spec.openapi
        );
    }
    Ok(())
}

/// Extract the base URL from the first server entry, or return a placeholder.
///
/// Trailing slashes are stripped from the URL so that path concatenation
/// produces well-formed URLs without double slashes.
#[must_use]
pub fn extract_base_url(spec: &OpenAPI) -> String {
    spec.servers
        .first()
        .map(|s| s.url.trim_end_matches('/').to_string())
        .unwrap_or_else(|| "http://localhost".to_string())
}

/// Derive a config name from the spec title (slugified).
///
/// Falls back to slugifying `fallback_filename` when the title is empty.
#[must_use]
pub fn derive_config_name(spec: &OpenAPI, fallback_filename: &str) -> String {
    let title = &spec.info.title;
    if title.is_empty() {
        return slugify(fallback_filename);
    }
    slugify(title)
}

/// Convert a string to a lowercase hyphen-separated slug.
///
/// Non-alphanumeric characters are replaced with hyphens, consecutive hyphens
/// are collapsed, and leading/trailing hyphens are removed.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|seg| !seg.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_petstore_json() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert_eq!(spec.openapi, "3.0.0");
        assert_eq!(spec.info.title, "Petstore");
        assert_eq!(spec.paths.paths.len(), 2);
    }

    #[test]
    fn test_validate_version_3() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert!(validate_version(&spec).is_ok());
    }

    #[test]
    fn test_extract_base_url() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert_eq!(extract_base_url(&spec), "https://petstore.example.com/v1");
    }

    #[test]
    fn test_derive_config_name() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = parse_spec(&content).unwrap();
        assert_eq!(derive_config_name(&spec, "fallback"), "petstore");
    }

    #[test]
    fn test_slugify_complex_title() {
        assert_eq!(slugify("My Cool API v2"), "my-cool-api-v2");
        assert_eq!(slugify("  spaces  "), "spaces");
    }

    #[test]
    fn test_reject_swagger_2() {
        let swagger2 = r#"{"swagger": "2.0", "info": {"title": "Old API", "version": "1.0"}, "paths": {}}"#;
        match parse_spec(swagger2) {
            Ok(spec) => {
                let result = validate_version(&spec);
                assert!(result.is_err(), "should reject Swagger 2.0");
                assert!(result.unwrap_err().to_string().contains("not yet supported"));
            }
            Err(_) => {} // Parsing failure is also acceptable for Swagger 2.0 input
        }
    }

    #[test]
    fn test_extract_base_url_missing_servers() {
        let no_servers =
            r#"{"openapi": "3.0.0", "info": {"title": "Test", "version": "1.0"}, "paths": {}}"#;
        let spec = parse_spec(no_servers).unwrap();
        assert_eq!(extract_base_url(&spec), "http://localhost");
    }
}
