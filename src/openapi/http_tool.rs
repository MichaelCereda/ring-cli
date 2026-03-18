use std::process::Command;

/// Detect available HTTP tool. Returns "curl" or "wget".
pub fn detect_http_tool() -> Result<String, anyhow::Error> {
    if Command::new("curl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok("curl".to_string());
    }
    if Command::new("wget")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Ok("wget".to_string());
    }
    anyhow::bail!("curl or wget is required for OpenAPI support. Install one and try again.")
}

/// Build the shell command string to fetch a remote URL.
pub fn build_fetch_command(tool: &str, url: &str) -> String {
    match tool {
        "curl" => format!("curl -s -f -L '{url}'"),
        "wget" => format!("wget -q -O- '{url}'"),
        _ => unreachable!("unsupported tool: {tool}"),
    }
}

/// Fetch a remote URL using the detected tool. Returns the response body.
pub fn fetch_remote(tool: &str, url: &str) -> Result<String, anyhow::Error> {
    let cmd = build_fetch_command(tool, url);
    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to download {url}: {stderr}");
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Generate a curl command string for an API operation.
pub fn generate_curl_command(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body_json_template: Option<&str>,
) -> String {
    let mut parts = vec![format!("curl -s -X {method} '{url}'")];
    for (key, value) in headers {
        parts.push(format!("-H '{key}: {value}'"));
    }
    if let Some(body) = body_json_template {
        parts.push(format!("-d '{body}'"));
    }
    parts.join(" ")
}

/// Generate a wget command string for an API operation.
pub fn generate_wget_command(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body_json_template: Option<&str>,
) -> String {
    let mut parts = vec![format!("wget -q -O- --method={method} '{url}'")];
    for (key, value) in headers {
        parts.push(format!("--header='{key}: {value}'"));
    }
    if let Some(body) = body_json_template {
        parts.push(format!("--body-data='{body}'"));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http_tool_finds_curl_or_wget() {
        let tool = detect_http_tool();
        assert!(tool.is_ok(), "neither curl nor wget found");
        let name = tool.unwrap();
        assert!(
            name == "curl" || name == "wget",
            "unexpected tool: {name}"
        );
    }

    #[test]
    fn test_fetch_command_curl() {
        let cmd = build_fetch_command("curl", "https://example.com/spec.json");
        assert!(cmd.contains("curl"));
        assert!(cmd.contains("-s"));
        assert!(cmd.contains("-f"));
        assert!(cmd.contains("-L"));
        assert!(cmd.contains("https://example.com/spec.json"));
    }

    #[test]
    fn test_fetch_command_wget() {
        let cmd = build_fetch_command("wget", "https://example.com/spec.json");
        assert!(cmd.contains("wget"));
        assert!(cmd.contains("-q"));
        assert!(cmd.contains("-O-"));
        assert!(cmd.contains("https://example.com/spec.json"));
    }

    #[test]
    fn test_generate_curl_get() {
        let cmd = generate_curl_command("GET", "https://api.example.com/pets", &[], None);
        assert_eq!(cmd, "curl -s -X GET 'https://api.example.com/pets'");
    }

    #[test]
    fn test_generate_curl_post_with_body() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
        ];
        let cmd = generate_curl_command(
            "POST",
            "https://api.example.com/pets",
            &headers,
            Some(r#"{"name":"${{name}}"}"#),
        );
        assert!(cmd.contains("curl -s -X POST"));
        assert!(cmd.contains("-H 'Content-Type: application/json'"));
        assert!(cmd.contains(r#"-d '{"name":"${{name}}"}'"#));
    }

    #[test]
    fn test_generate_wget_get() {
        let cmd = generate_wget_command("GET", "https://api.example.com/pets", &[], None);
        assert_eq!(cmd, "wget -q -O- --method=GET 'https://api.example.com/pets'");
    }

    #[test]
    fn test_generate_wget_post_with_body() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
        ];
        let cmd = generate_wget_command(
            "POST",
            "https://api.example.com/pets",
            &headers,
            Some(r#"{"name":"${{name}}"}"#),
        );
        assert!(cmd.contains("wget -q -O- --method=POST"));
        assert!(cmd.contains("--header='Content-Type: application/json'"));
        assert!(cmd.contains(r#"--body-data='{"name":"${{name}}"}'"#));
    }
}
