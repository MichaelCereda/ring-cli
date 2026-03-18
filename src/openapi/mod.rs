pub mod http_tool;
pub mod parser;
pub mod transform;

use crate::models::Configuration;

/// Process an OpenAPI config source (path or URL, after stripping the `openapi:` prefix).
///
/// Returns `(Configuration, raw_source_content_for_hashing)`.
///
/// The `tool` parameter must be `"curl"` or `"wget"`.
/// When `yes` is `false` and the source is a remote URL the user is prompted
/// to confirm the download before proceeding.
/// When `verbose` is `true` every per-operation warning is printed to stderr;
/// a summary is always printed when warnings are non-empty.
///
/// # Errors
///
/// Returns an error when:
/// - the user declines the remote download prompt
/// - the remote fetch fails
/// - the local file cannot be read
/// - spec parsing or version validation fails
/// - the spec contains no usable operations
pub fn process_openapi_source(
    source: &str,
    tool: &str,
    yes: bool,
    verbose: bool,
) -> Result<(Configuration, String), anyhow::Error> {
    let is_remote = source.starts_with("http://") || source.starts_with("https://");

    let raw_content = if is_remote {
        if !yes {
            eprint!(
                "Warning: ring-cli will use '{}' to download {}\nContinue? [Y/n] ",
                tool, source
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_lowercase();
            if trimmed == "n" || trimmed == "no" {
                anyhow::bail!("Download cancelled by user");
            }
        }
        eprintln!("Downloading OpenAPI spec...");
        http_tool::fetch_remote(tool, source)?
    } else {
        std::fs::read_to_string(source)
            .map_err(|e| anyhow::anyhow!("Failed to read OpenAPI spec at {source}: {e}"))?
    };

    let spec = parser::parse_spec(&raw_content)?;
    parser::validate_version(&spec)?;

    let fallback_name = std::path::Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "api".to_string());

    let mut warnings = Vec::new();
    let config =
        transform::transform_spec_with_warnings(&spec, tool, &fallback_name, &mut warnings)?;

    if verbose {
        for w in &warnings {
            eprintln!("{w}");
        }
    }
    let summary = transform::summarize_warnings(&warnings);
    if !summary.is_empty() {
        eprintln!("{summary}");
    }

    Ok((config, raw_content))
}
