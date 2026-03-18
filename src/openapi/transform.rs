//! OpenAPI 3.0 specification to ring-cli `Configuration` transformation.
//!
//! # Design
//!
//! Path segments become the subcommand hierarchy:
//! - `/pets` becomes the top-level command `pets`
//! - `/pets/{petId}` adds item-level verbs (`get`, `delete`) under `pets`
//! - `/pets/{petId}/toys` adds `pets > toys` with verbs (`list`, `create`)
//!
//! HTTP methods map to verbs via [`method_to_verb`].
//!
//! Path parameters (`{petId}`) and query parameters (`?limit`) become CLI
//! flags on the leaf command.  Request body fields are flattened recursively
//! with dot-separated prefixes so that `owner.address.city` becomes the flag
//! `--owner.address.city`.  clap 4 accepts dots in long flag names.
//!
//! Intermediate hierarchy nodes carry only `subcommands`; leaf nodes carry
//! only `cmd`.

use std::collections::HashMap;

use openapiv3::{APIKeyLocation, OpenAPI, Parameter, ReferenceOr, Schema, SchemaKind, SecurityScheme, Type};

use crate::models::{CmdType, Command, Configuration, Flag};
use crate::openapi::http_tool::{generate_curl_command, generate_wget_command};
use crate::openapi::parser::extract_base_url;

// ---------------------------------------------------------------------------
// Security helpers — public so callers and tests can use them directly.
// ---------------------------------------------------------------------------

/// Convert a camelCase or PascalCase security scheme name to UPPER_SNAKE_CASE
/// and append `_TOKEN`.
///
/// # Examples
///
/// ```
/// use ring_cli::openapi::transform::security_scheme_to_env_var;
/// assert_eq!(security_scheme_to_env_var("bearerAuth"), "BEARER_AUTH_TOKEN");
/// assert_eq!(security_scheme_to_env_var("apiKey"), "API_KEY_TOKEN");
/// ```
#[must_use]
pub fn security_scheme_to_env_var(scheme_name: &str) -> String {
    let mut out = String::with_capacity(scheme_name.len() + 8);
    let chars: Vec<char> = scheme_name.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '-' || ch == '_' {
            out.push('_');
        } else if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push(ch.to_uppercase().next().unwrap_or(ch));
        }
    }
    // Collapse consecutive underscores and strip leading/trailing.
    let snake: String = out
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    format!("{snake}_TOKEN")
}

/// Extract auth headers from the spec's global security requirements and
/// component security scheme definitions.
///
/// Returns `(header_name, header_value)` tuples ready to be passed to the
/// curl/wget command generators.
fn extract_auth_headers(spec: &OpenAPI) -> Vec<(String, String)> {
    let security_reqs = match &spec.security {
        None => return vec![],
        Some(reqs) => reqs,
    };
    let components = match &spec.components {
        None => return vec![],
        Some(c) => c,
    };

    // Collect the set of scheme names referenced in the global security list.
    let referenced: Vec<&str> = security_reqs
        .iter()
        .flat_map(|req| req.keys().map(|k| k.as_str()))
        .collect();

    let mut headers: Vec<(String, String)> = Vec::new();

    for scheme_name in referenced {
        let scheme_ref = match components.security_schemes.get(scheme_name) {
            None => continue,
            Some(s) => s,
        };
        let scheme = match scheme_ref {
            ReferenceOr::Item(s) => s,
            ReferenceOr::Reference { .. } => continue, // $ref schemes not resolved
        };
        let env_var = security_scheme_to_env_var(scheme_name);
        match scheme {
            SecurityScheme::HTTP { scheme, .. } if scheme.eq_ignore_ascii_case("bearer") => {
                headers.push((
                    "Authorization".to_string(),
                    format!("Bearer ${{{{env.{env_var}}}}}"),
                ));
            }
            SecurityScheme::APIKey {
                location: APIKeyLocation::Header,
                name,
                ..
            } => {
                headers.push((name.clone(), format!("${{{{env.{env_var}}}}}")));
            }
            _ => {} // OAuth2, OpenIDConnect, query/cookie API keys — unsupported, skip silently.
        }
    }

    headers
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Transform an OpenAPI spec into a ring-cli [`Configuration`].
///
/// `tool` must be either `"curl"` or `"wget"`.
/// `fallback_name` is used when the spec title is empty.
///
/// # Errors
///
/// Returns an error when the spec contains no usable operations.
#[allow(dead_code)]
pub fn transform_spec(
    spec: &OpenAPI,
    tool: &str,
    fallback_name: &str,
) -> Result<Configuration, anyhow::Error> {
    let mut warnings = Vec::new();
    transform_spec_with_warnings(spec, tool, fallback_name, &mut warnings)
}

/// Transform an OpenAPI spec into a ring-cli [`Configuration`], collecting
/// non-fatal diagnostic messages into `warnings`.
///
/// # Errors
///
/// Returns an error when the spec contains no usable operations.
pub fn transform_spec_with_warnings(
    spec: &OpenAPI,
    tool: &str,
    fallback_name: &str,
    warnings: &mut Vec<String>,
) -> Result<Configuration, anyhow::Error> {
    let name = crate::openapi::parser::derive_config_name(spec, fallback_name);
    let description = spec
        .info
        .description
        .clone()
        .unwrap_or_else(|| format!("Generated from OpenAPI spec: {}", spec.info.title));
    let base_url = extract_base_url(spec);

    // Auth headers derived from the spec's global security requirements.
    let auth_headers = extract_auth_headers(spec);

    // Collect all operations as (path_string, method, &operation).
    let mut operations: Vec<(String, String, &openapiv3::Operation)> = Vec::new();

    for (path_str, path_ref) in spec.paths.iter() {
        let path_item = match path_ref {
            ReferenceOr::Item(item) => item,
            ReferenceOr::Reference { reference } => {
                warnings.push(format!(
                    "Skipping path {path_str}: $ref paths ({reference}) are not supported"
                ));
                continue;
            }
        };
        for (method, operation) in path_item.iter() {
            operations.push((path_str.clone(), method.to_string(), operation));
        }
    }

    // Build a tree keyed on non-param path segments.
    let mut top_nodes: HashMap<String, TreeNode> = HashMap::new();

    for (path_str, method, operation) in &operations {
        let raw_segments: Vec<&str> = path_str
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if raw_segments.is_empty() {
            warnings.push(format!("Skipping root path '{path_str}' ({method})"));
            continue;
        }

        // An item operation is one where the LAST raw segment is a `{param}`.
        let is_item = raw_segments
            .last()
            .map(|s| is_path_param(s))
            .unwrap_or(false);

        // Command hierarchy keys: drop path-parameter segments.
        let key_segments: Vec<&str> = raw_segments
            .iter()
            .copied()
            .filter(|s| !is_path_param(s))
            .collect();

        if key_segments.is_empty() {
            warnings.push(format!(
                "Skipping path '{path_str}' ({method}): all segments are path parameters"
            ));
            continue;
        }

        let verb = method_to_verb(method, is_item);

        let description_str = operation
            .summary
            .clone()
            .or_else(|| operation.description.clone())
            .unwrap_or_else(|| format!("{} {}", method.to_uppercase(), path_str));

        // Build flags from path and query parameters.
        let mut flags: Vec<Flag> = Vec::new();
        for param_ref in &operation.parameters {
            match param_ref {
                ReferenceOr::Item(param) => match param {
                    Parameter::Path { parameter_data, .. } => {
                        flags.push(Flag {
                            name: param_to_flag_name(&parameter_data.name),
                            short: None,
                            description: parameter_data
                                .description
                                .clone()
                                .unwrap_or_else(|| {
                                    format!("Path parameter: {}", parameter_data.name)
                                }),
                        });
                    }
                    Parameter::Query { parameter_data, .. } => {
                        flags.push(Flag {
                            name: param_to_flag_name(&parameter_data.name),
                            short: None,
                            description: parameter_data
                                .description
                                .clone()
                                .unwrap_or_else(|| {
                                    format!("Query parameter: {}", parameter_data.name)
                                }),
                        });
                    }
                    _ => {} // Header / Cookie — skip
                },
                ReferenceOr::Reference { reference } => {
                    warnings.push(format!(
                        "Skipping $ref parameter ({reference}) in {path_str} ({method})"
                    ));
                }
            }
        }

        // Body flags (JSON schema only).
        let body_flags = collect_body_flags(&operation.request_body, path_str, method, warnings);
        flags.extend(body_flags.clone());

        // Build URL template: `{param}` → `${{flag-name}}`.
        let url_template = build_url_template(&base_url, path_str);

        // Identify which flags came from query parameters so we can append them.
        let query_flag_names: Vec<String> = operation
            .parameters
            .iter()
            .filter_map(|p| {
                if let ReferenceOr::Item(Parameter::Query { parameter_data, .. }) = p {
                    Some(param_to_flag_name(&parameter_data.name))
                } else {
                    None
                }
            })
            .collect();

        let query_flags: Vec<&Flag> = flags
            .iter()
            .filter(|f| query_flag_names.contains(&f.name))
            .collect();

        let url_with_query = build_url_with_query(&url_template, &query_flags);

        let has_body = !body_flags.is_empty();
        // Start with any spec-level auth headers, then add content-type when needed.
        let mut headers: Vec<(String, String)> = auth_headers.clone();
        if has_body {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }
        let body_json = if has_body {
            Some(build_json_template(&body_flags))
        } else {
            None
        };

        let run_cmd = match tool {
            "wget" => generate_wget_command(
                &method.to_uppercase(),
                &url_with_query,
                &headers,
                body_json.as_deref(),
            ),
            _ => generate_curl_command(
                &method.to_uppercase(),
                &url_with_query,
                &headers,
                body_json.as_deref(),
            ),
        };

        let leaf = LeafCommand {
            verb: verb.to_string(),
            description: description_str,
            flags,
            cmd: CmdType { run: vec![run_cmd] },
        };

        insert_into_tree(&mut top_nodes, &key_segments, leaf);
    }

    if top_nodes.is_empty() {
        anyhow::bail!("No operations found in the OpenAPI spec");
    }

    let mut commands: HashMap<String, Command> = top_nodes
        .into_iter()
        .map(|(k, node)| {
            let mut cmd = node.into_command();
            if cmd.description.is_empty() {
                cmd.description = k.clone();
            }
            (k, cmd)
        })
        .collect();

    // Recursively fill empty descriptions.
    fill_descriptions(&mut commands);

    Ok(Configuration {
        version: "2.0".to_string(),
        name,
        description,
        base_dir: None,
        banner: None,
        commands,
    })
}

/// Produce a human-readable summary of transformation warnings.
///
/// The summary breaks warnings down into:
/// - Skipped operations (prefixed with `"Skipped:"`)
/// - Best-effort approximations (prefixed with `"Best-effort:"`)
/// - Other diagnostics
///
/// # Examples
///
/// ```
/// use ring_cli::openapi::transform::summarize_warnings;
/// let warnings = vec![
///     "Skipped: /xml-only GET - no JSON content type".to_string(),
///     "Best-effort: /upload POST - multipart/form-data".to_string(),
/// ];
/// let summary = summarize_warnings(&warnings);
/// assert!(summary.contains("Skipped"));
/// ```
#[must_use]
pub fn summarize_warnings(warnings: &[String]) -> String {
    if warnings.is_empty() {
        return String::new();
    }

    let skipped: Vec<&str> = warnings
        .iter()
        .filter(|w| w.starts_with("Skipped:"))
        .map(|w| w.as_str())
        .collect();
    let best_effort: Vec<&str> = warnings
        .iter()
        .filter(|w| w.starts_with("Best-effort:"))
        .map(|w| w.as_str())
        .collect();
    let other: Vec<&str> = warnings
        .iter()
        .filter(|w| !w.starts_with("Skipped:") && !w.starts_with("Best-effort:"))
        .map(|w| w.as_str())
        .collect();

    let mut out = String::new();

    if !skipped.is_empty() {
        out.push_str(&format!(
            "Skipped {} operation(s) (unsupported content types / path refs)\n",
            skipped.len()
        ));
        for w in &skipped {
            out.push_str(&format!("  - {w}\n"));
        }
    }
    if !best_effort.is_empty() {
        out.push_str(&format!(
            "{} operation(s) use best-effort approximations (use --verbose for details)\n",
            best_effort.len()
        ));
        for w in &best_effort {
            out.push_str(&format!("  - {w}\n"));
        }
    }
    if !other.is_empty() {
        out.push_str(&format!("{} other warning(s):\n", other.len()));
        for w in &other {
            out.push_str(&format!("  - {w}\n"));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Conversion helpers — public so callers and tests can use them directly.
// ---------------------------------------------------------------------------

/// Map an HTTP method and item context to a ring-cli verb name.
///
/// `is_item` is `true` when the last raw path segment is a `{param}`.
pub fn method_to_verb(method: &str, is_item: bool) -> &'static str {
    match (method, is_item) {
        ("get", false) => "list",
        ("get", true) => "get",
        ("post", _) => "create",
        ("put", _) => "update",
        ("patch", _) => "patch",
        ("delete", _) => "delete",
        ("options", _) => "options",
        ("head", _) => "head",
        ("trace", _) => "trace",
        _ => "run",
    }
}

/// Convert a camelCase or `X-Header-Name` style parameter name to kebab-case.
///
/// Examples:
/// - `petId`      → `pet-id`
/// - `limit`      → `limit`
/// - `X-Request-Id` → `x-request-id`
pub fn param_to_flag_name(param_name: &str) -> String {
    let mut out = String::with_capacity(param_name.len() + 4);
    let chars: Vec<char> = param_name.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '_' || ch == '-' {
            out.push('-');
        } else if ch.is_uppercase() {
            let prev_is_lower_or_digit = i > 0
                && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let next_is_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
            let at_start = i == 0;
            if !at_start && (prev_is_lower_or_digit || next_is_lower) {
                out.push('-');
            }
            out.push(ch.to_lowercase().next().unwrap());
        } else {
            out.push(ch);
        }
    }
    // Collapse consecutive hyphens and strip leading/trailing.
    out.split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Recursively flatten an OpenAPI [`Schema`] into a list of [`Flag`]s.
///
/// Nested objects produce dot-separated names: `owner.address.city`.
pub fn flatten_schema_to_flags(schema: &Schema, prefix: &str, flags: &mut Vec<Flag>) {
    match &schema.schema_kind {
        SchemaKind::Type(Type::Object(obj)) => {
            for (prop_name, prop_ref) in &obj.properties {
                let full_name = dot_join(prefix, prop_name);
                if let ReferenceOr::Item(boxed) = prop_ref {
                    flatten_schema_to_flags(boxed, &full_name, flags);
                }
                // Skip $ref properties silently — they are resolved separately.
            }
        }
        SchemaKind::Any(any) if !any.properties.is_empty() => {
            for (prop_name, prop_ref) in &any.properties {
                let full_name = dot_join(prefix, prop_name);
                if let ReferenceOr::Item(boxed) = prop_ref {
                    flatten_schema_to_flags(boxed, &full_name, flags);
                }
            }
        }
        _ => {
            // Leaf (string, integer, boolean, …) — emit one flag.
            if !prefix.is_empty() {
                let description = schema
                    .schema_data
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Field: {prefix}"));
                flags.push(Flag {
                    name: prefix.to_string(),
                    short: None,
                    description,
                });
            }
        }
    }
}

/// Build a JSON body template from a flat list of (possibly dot-path) flags.
///
/// `owner.address.city` with flag name `owner.address.city` becomes:
/// `{"owner":{"address":{"city":"${{owner.address.city}}"}}}`.
#[must_use]
pub fn build_json_template(flags: &[Flag]) -> String {
    let mut root = serde_json::Map::new();
    for flag in flags {
        insert_json_path(&mut root, &flag.name, &flag.name);
    }
    serde_json::to_string(&serde_json::Value::Object(root))
        .unwrap_or_else(|_| "{}".to_string())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn is_path_param(segment: &str) -> bool {
    segment.starts_with('{') && segment.ends_with('}')
}

fn dot_join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

/// Recursively insert a dot-path key into a JSON map as nested objects.
/// The leaf value is the `${{flag_name}}` placeholder string.
fn insert_json_path(
    map: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    flag_name: &str,
) {
    if let Some(dot_pos) = path.find('.') {
        let key = &path[..dot_pos];
        let rest = &path[dot_pos + 1..];
        let entry = map
            .entry(key.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(child) = entry {
            insert_json_path(child, rest, flag_name);
        }
    } else {
        map.insert(
            path.to_string(),
            serde_json::Value::String(format!("${{{{{flag_name}}}}}")),
        );
    }
}

/// Build the URL string for a path, replacing `{param}` with `${{flag-name}}`.
fn build_url_template(base_url: &str, path: &str) -> String {
    let substituted: String = path
        .split('/')
        .map(|seg| {
            if is_path_param(seg) {
                let inner = &seg[1..seg.len() - 1];
                format!("${{{{{}}}}}", param_to_flag_name(inner))
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("{base_url}{substituted}")
}

/// Append query parameter placeholders to the URL template.
fn build_url_with_query(url_template: &str, query_flags: &[&Flag]) -> String {
    if query_flags.is_empty() {
        return url_template.to_string();
    }
    let qs: String = query_flags
        .iter()
        .map(|f| format!("{}=${{{{{}}}}}", f.name, f.name))
        .collect::<Vec<_>>()
        .join("&");
    format!("{url_template}?{qs}")
}

/// Extract body flags from an optional request body (JSON schema only).
///
/// Non-JSON content types produce warnings; `multipart/form-data` generates a
/// best-effort `--file` flag. Operations with *only* non-JSON content types
/// are indicated by a `"Skipped:"` prefix on the warning.
fn collect_body_flags(
    request_body: &Option<ReferenceOr<openapiv3::RequestBody>>,
    path_str: &str,
    method: &str,
    warnings: &mut Vec<String>,
) -> Vec<Flag> {
    let rb = match request_body {
        None => return vec![],
        Some(ReferenceOr::Reference { reference }) => {
            warnings.push(format!(
                "Skipping $ref requestBody ({reference}): not supported"
            ));
            return vec![];
        }
        Some(ReferenceOr::Item(rb)) => rb,
    };

    // If the request body has multipart/form-data but no application/json,
    // emit a best-effort `--file` flag and record a warning.
    let has_json = rb.content.contains_key("application/json");
    let has_multipart = rb.content.contains_key("multipart/form-data");

    if !has_json {
        // Check whether there are any content types at all.
        if rb.content.is_empty() {
            return vec![];
        }

        if has_multipart {
            warnings.push(format!(
                "Best-effort: {path_str} {} - multipart/form-data; generated --file flag",
                method.to_uppercase()
            ));
            return vec![Flag {
                name: "file".to_string(),
                short: None,
                description: "File to upload".to_string(),
            }];
        }

        // No JSON, not multipart — record as skipped.
        let types: Vec<&str> = rb.content.keys().map(|k| k.as_str()).collect();
        warnings.push(format!(
            "Skipped: {path_str} {} - no JSON content type (found: {})",
            method.to_uppercase(),
            types.join(", ")
        ));
        return vec![];
    }

    let mt = rb.content.get("application/json").unwrap();

    match &mt.schema {
        None => vec![],
        Some(ReferenceOr::Item(schema)) => {
            let mut flags = Vec::new();
            flatten_schema_to_flags(schema, "", &mut flags);
            flags
        }
        Some(ReferenceOr::Reference { reference }) => {
            warnings.push(format!(
                "Skipping $ref schema ({reference}) in requestBody: not supported"
            ));
            vec![]
        }
    }
}

/// Recursively fill in empty `description` fields with the command's key name.
fn fill_descriptions(commands: &mut HashMap<String, Command>) {
    for (name, cmd) in commands.iter_mut() {
        if cmd.description.is_empty() {
            cmd.description = name.clone();
        }
        if let Some(subs) = &mut cmd.subcommands {
            fill_descriptions(subs);
        }
    }
}

// ---------------------------------------------------------------------------
// Tree data structure for building nested commands
// ---------------------------------------------------------------------------

/// A fully-resolved leaf operation (one HTTP method on one path).
struct LeafCommand {
    verb: String,
    description: String,
    flags: Vec<Flag>,
    cmd: CmdType,
}

/// A node in the command-hierarchy tree.
///
/// - `verbs` holds leaf commands that belong at this level (e.g. `list`, `create`).
/// - `children` holds deeper sub-trees (the next non-param path segment).
struct TreeNode {
    verbs: HashMap<String, LeafCommand>,
    children: HashMap<String, TreeNode>,
}

impl TreeNode {
    fn new() -> Self {
        Self {
            verbs: HashMap::new(),
            children: HashMap::new(),
        }
    }

    /// Materialise this tree node into a ring-cli [`Command`].
    fn into_command(self) -> Command {
        let has_children = !self.children.is_empty();
        let has_verbs = !self.verbs.is_empty();

        if !has_children && has_verbs {
            // All verbs become subcommands of this node.
            let subs: HashMap<String, Command> = self
                .verbs
                .into_iter()
                .map(|(verb, leaf)| {
                    (
                        verb,
                        Command {
                            description: leaf.description,
                            flags: leaf.flags,
                            cmd: Some(leaf.cmd),
                            subcommands: None,
                        },
                    )
                })
                .collect();
            Command {
                description: String::new(),
                flags: vec![],
                cmd: None,
                subcommands: Some(subs),
            }
        } else if has_children && !has_verbs {
            let subs: HashMap<String, Command> = self
                .children
                .into_iter()
                .map(|(k, child)| (k, child.into_command()))
                .collect();
            Command {
                description: String::new(),
                flags: vec![],
                cmd: None,
                subcommands: Some(subs),
            }
        } else if has_children && has_verbs {
            // Merge both verb commands and child sub-trees into subcommands.
            let mut subs: HashMap<String, Command> = self
                .children
                .into_iter()
                .map(|(k, child)| (k, child.into_command()))
                .collect();
            for (verb, leaf) in self.verbs {
                subs.insert(
                    verb,
                    Command {
                        description: leaf.description,
                        flags: leaf.flags,
                        cmd: Some(leaf.cmd),
                        subcommands: None,
                    },
                );
            }
            Command {
                description: String::new(),
                flags: vec![],
                cmd: None,
                subcommands: Some(subs),
            }
        } else {
            // Empty node — should not occur; emit a no-op placeholder.
            Command {
                description: "empty".to_string(),
                flags: vec![],
                cmd: Some(CmdType {
                    run: vec!["true".to_string()],
                }),
                subcommands: None,
            }
        }
    }
}

/// Insert a [`LeafCommand`] into the top-level tree map at `segments`.
fn insert_into_tree(
    top: &mut HashMap<String, TreeNode>,
    segments: &[&str],
    leaf: LeafCommand,
) {
    if segments.is_empty() {
        return;
    }
    let root_key = segments[0];
    let node = top
        .entry(root_key.to_string())
        .or_insert_with(TreeNode::new);

    if segments.len() == 1 {
        node.verbs.insert(leaf.verb.clone(), leaf);
    } else {
        insert_into_tree_node(node, &segments[1..], leaf);
    }
}

fn insert_into_tree_node(node: &mut TreeNode, segments: &[&str], leaf: LeafCommand) {
    if segments.is_empty() {
        return;
    }
    let key = segments[0];
    let child = node
        .children
        .entry(key.to_string())
        .or_insert_with(TreeNode::new);

    if segments.len() == 1 {
        child.verbs.insert(leaf.verb.clone(), leaf);
    } else {
        insert_into_tree_node(child, &segments[1..], leaf);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clap_accepts_dots_in_long_flag() {
        let cmd = clap::Command::new("test")
            .arg(clap::Arg::new("owner.email").long("owner.email"));
        let matches = cmd.try_get_matches_from(["test", "--owner.email", "foo"]);
        assert!(matches.is_ok(), "clap does not accept dots in long flags");
    }

    #[test]
    fn test_method_to_verb() {
        assert_eq!(method_to_verb("get", false), "list");
        assert_eq!(method_to_verb("get", true), "get");
        assert_eq!(method_to_verb("post", false), "create");
        assert_eq!(method_to_verb("delete", true), "delete");
        assert_eq!(method_to_verb("put", true), "update");
        assert_eq!(method_to_verb("patch", true), "patch");
    }

    #[test]
    fn test_param_to_flag_name() {
        assert_eq!(param_to_flag_name("petId"), "pet-id");
        assert_eq!(param_to_flag_name("limit"), "limit");
        assert_eq!(param_to_flag_name("X-Request-Id"), "x-request-id");
    }

    #[test]
    fn test_transform_petstore_basic() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();
        assert_eq!(config.name, "petstore");
        assert!(config.commands.contains_key("pets"));
    }

    #[test]
    fn test_collection_vs_item_operations() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let subs = pets.subcommands.as_ref().unwrap();
        assert!(subs.contains_key("list"), "missing 'list'");
        assert!(subs.contains_key("create"), "missing 'create'");
        assert!(subs.contains_key("get"), "missing 'get'");
        assert!(subs.contains_key("delete"), "missing 'delete'");
    }

    #[test]
    fn test_path_params_become_flags() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let get = pets.subcommands.as_ref().unwrap().get("get").unwrap();
        let flag_names: Vec<&str> = get.flags.iter().map(|f| f.name.as_str()).collect();
        assert!(
            flag_names.contains(&"pet-id"),
            "missing --pet-id: {flag_names:?}"
        );
    }

    #[test]
    fn test_query_params_become_flags() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let flag_names: Vec<&str> = list.flags.iter().map(|f| f.name.as_str()).collect();
        assert!(
            flag_names.contains(&"limit"),
            "missing --limit: {flag_names:?}"
        );
    }

    #[test]
    fn test_generated_curl_command() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let cmd = list.cmd.as_ref().unwrap();
        let run_cmd = &cmd.run[0];
        assert!(run_cmd.contains("curl"), "expected curl: {run_cmd}");
        assert!(run_cmd.contains("GET"), "expected GET: {run_cmd}");
        assert!(
            run_cmd.contains("petstore.example.com"),
            "expected base URL: {run_cmd}"
        );
    }

    #[test]
    fn test_generated_wget_command() {
        let content = std::fs::read_to_string("tests/fixtures/petstore.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "wget", "petstore").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let cmd = list.cmd.as_ref().unwrap();
        assert!(cmd.run[0].contains("wget"), "expected wget");
    }

    #[test]
    fn test_nested_path_hierarchy() {
        let content =
            std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let subs = pets.subcommands.as_ref().unwrap();
        assert!(
            subs.contains_key("toys"),
            "missing 'toys' under pets: {subs:?}",
            subs = subs.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_body_flags_generated() {
        let content =
            std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let create = pets.subcommands.as_ref().unwrap().get("create").unwrap();
        let flag_names: Vec<&str> =
            create.flags.iter().map(|f| f.name.as_str()).collect();
        assert!(
            flag_names.contains(&"name"),
            "missing --name: {flag_names:?}"
        );
        assert!(
            flag_names.contains(&"tag"),
            "missing --tag: {flag_names:?}"
        );
    }

    #[test]
    fn test_deep_nested_body_flags() {
        let content =
            std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let create = pets.subcommands.as_ref().unwrap().get("create").unwrap();
        let flag_names: Vec<&str> =
            create.flags.iter().map(|f| f.name.as_str()).collect();
        let has_city = flag_names
            .iter()
            .any(|n| n.contains("owner") && n.contains("address") && n.contains("city"));
        assert!(has_city, "missing deeply nested city flag: {flag_names:?}");
    }

    #[test]
    fn test_create_command_has_body_in_curl() {
        let content =
            std::fs::read_to_string("tests/fixtures/petstore_nested.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-nested").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let create = pets.subcommands.as_ref().unwrap().get("create").unwrap();
        let cmd_str = &create.cmd.as_ref().unwrap().run[0];
        assert!(
            cmd_str.contains("-d "),
            "POST should have body data: {cmd_str}"
        );
        assert!(
            cmd_str.contains("name"),
            "body should contain name: {cmd_str}"
        );
    }

    // -----------------------------------------------------------------------
    // Part A: Security scheme tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_security_scheme_to_env_var() {
        assert_eq!(security_scheme_to_env_var("bearerAuth"), "BEARER_AUTH_TOKEN");
        assert_eq!(security_scheme_to_env_var("apiKey"), "API_KEY_TOKEN");
    }

    #[test]
    fn test_bearer_auth_in_generated_command() {
        let content =
            std::fs::read_to_string("tests/fixtures/petstore_auth.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let config = transform_spec(&spec, "curl", "petstore-auth").unwrap();
        let pets = config.commands.get("pets").unwrap();
        let list = pets.subcommands.as_ref().unwrap().get("list").unwrap();
        let cmd_str = &list.cmd.as_ref().unwrap().run[0];
        assert!(
            cmd_str.contains("Authorization: Bearer"),
            "missing auth header: {cmd_str}"
        );
        assert!(
            cmd_str.contains("${{env.BEARER_AUTH_TOKEN}}"),
            "missing env var: {cmd_str}"
        );
    }

    // -----------------------------------------------------------------------
    // Part B: Unsupported features / warnings tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_unsupported_with_warnings() {
        let content =
            std::fs::read_to_string("tests/fixtures/openapi_mixed.json").unwrap();
        let spec = crate::openapi::parser::parse_spec(&content).unwrap();
        let mut warnings = Vec::new();
        let config =
            transform_spec_with_warnings(&spec, "curl", "mixed", &mut warnings).unwrap();
        // Normal endpoint should be present.
        assert!(
            config.commands.contains_key("normal"),
            "missing normal endpoint"
        );
        // Should have some warnings for the unsupported endpoints.
        assert!(!warnings.is_empty(), "expected warnings for unsupported features");
    }

    #[test]
    fn test_summarize_warnings() {
        let warnings = vec![
            "Skipped: /xml-only GET - no JSON content type".to_string(),
            "Best-effort: /upload POST - multipart/form-data".to_string(),
        ];
        let summary = summarize_warnings(&warnings);
        assert!(
            summary.contains("Skipped"),
            "summary should mention skipped: {summary}"
        );
    }
}
