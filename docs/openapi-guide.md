# OpenAPI Support Guide

Stampo can transform OpenAPI 3.0+ specifications into ready-to-use command-line tools. This guide covers how to use OpenAPI specs with stampo, from fetching remote specs to mixing OpenAPI configs with regular YAML configs.

## What is OpenAPI Support

OpenAPI 3.0+ is a standard format for describing REST APIs. Stampo reads an OpenAPI spec and automatically generates CLI commands based on the API's paths, methods, parameters, and request bodies. This eliminates the need to manually write YAML configuration files for every API endpoint.

Key features:

- **Automatic command hierarchy** from API paths (e.g., `/pets/{id}` becomes `pets get --pet-id`)
- **Curl/wget integration** — commands execute via curl or wget with automatic header/body setup
- **Parameter mapping** — path, query, and body parameters become CLI flags
- **Authentication support** — Bearer tokens and API keys injected from environment variables
- **Local and remote specs** — use specs from your filesystem or fetch them from URLs
- **Spec refreshing** — update cached commands when the API spec changes

## Quick Start: Local Spec

To use a local OpenAPI spec file:

1. Obtain an OpenAPI spec (JSON or YAML). For example, save a spec to `./petstore.json`:

```bash
curl -s https://api.example.com/openapi.json -o petstore.json
```

2. Initialize an alias with the spec:

```bash
stampo init --alias myapi --config-path openapi:./petstore.json
```

Stampo parses the spec, generates commands, and caches them. Your alias is ready to use.

3. Use it:

```bash
# Tab completion works at every level
myapi <TAB>                         # shows: pets, orders, etc.
myapi pets <TAB>                    # shows: create, list, get, delete, etc.
myapi pets list --<TAB>             # shows: --limit, --skip, etc.

# Run a command
myapi pets list --limit 10
myapi pets get --pet-id 42
myapi pets create --name "Fluffy" --tag "dog"
```

## Quick Start: Remote Spec

To fetch and use a remote OpenAPI spec:

1. Initialize with a URL:

```bash
stampo init --alias github --config-path openapi:https://api.example.com/openapi.yml
```

2. Stampo detects curl or wget, prompts for download consent (to protect against accidental network access):

```
Warning: stampo will use 'curl' to download https://api.example.com/openapi.yml
Continue? [Y/n]
```

3. Type `y` or use `--yes` to skip the prompt (useful for CI/CD):

```bash
stampo init --alias github --config-path openapi:https://api.example.com/openapi.yml --yes
```

4. The spec is downloaded, parsed, transformed, and cached. Commands work the same as with a local spec.

## Path-to-Command Mapping

OpenAPI paths become a nested command hierarchy. The pattern is predictable:

| HTTP Method + Path | Command |
|---|---|
| `GET /pets` | `myapi pets list` |
| `POST /pets` | `myapi pets create` |
| `GET /pets/{petId}` | `myapi pets get --pet-id <value>` |
| `PUT /pets/{petId}` | `myapi pets update --pet-id <value>` |
| `DELETE /pets/{petId}` | `myapi pets delete --pet-id <value>` |
| `GET /pets/{petId}/toys` | `myapi pets toys list --pet-id <value>` |
| `POST /pets/{petId}/toys` | `myapi pets toys create --pet-id <value>` |

Rules:

- Path segments become subcommand levels (e.g., `/pets/toys/items` → `pets toys items`)
- Path parameters (e.g., `{petId}`) become flags (e.g., `--pet-id`) on the leaf command
- HTTP methods map to verbs: `GET` (collection) → `list`, `GET` (item) → `get`, `POST` → `create`, `PUT` → `update`, `PATCH` → `patch`, `DELETE` → `delete`
- Collection vs item is determined by the **last** path segment: if it's a `{param}`, it's an item operation; otherwise it's a collection operation

### Example

Given an API with:

```
GET    /pets
POST   /pets
GET    /pets/{petId}
DELETE /pets/{petId}
GET    /pets/{petId}/toys
```

Stampo generates:

```
myapi pets list
myapi pets create
myapi pets get --pet-id <value>
myapi pets delete --pet-id <value>
myapi pets toys list --pet-id <value>
```

## Request Bodies as Flags

When an operation accepts a JSON request body, stampo flattens the schema into flat flags with dot-notation. For example:

### Simple Nested Schema

OpenAPI schema:

```json
{
  "type": "object",
  "properties": {
    "name": { "type": "string", "description": "Pet name" },
    "owner": {
      "type": "object",
      "properties": {
        "email": { "type": "string", "description": "Owner email" }
      }
    }
  }
}
```

Generated flags:

```
--name           (Pet name)
--owner.email    (Owner email)
```

Usage:

```bash
myapi pets create --name "Fluffy" --owner.email "alice@example.com"
```

Stampo automatically constructs the JSON body:

```json
{
  "name": "Fluffy",
  "owner": {
    "email": "alice@example.com"
  }
}
```

### Deep Nesting

Nesting is unlimited. A 3-level-deep schema like:

```json
{
  "owner": {
    "address": {
      "city": { "type": "string" }
    }
  }
}
```

Generates the flag:

```
--owner.address.city
```

Usage:

```bash
myapi pets create --name "Spot" --owner.address.city "Portland"
```

## Authentication via Environment Variables

OpenAPI specs can define security schemes (Bearer tokens, API keys). Stampo extracts these and injects them as environment variables.

### Bearer Token

OpenAPI spec:

```yaml
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
security:
  - bearerAuth: []
```

Stampo converts the scheme name `bearerAuth` to the environment variable `BEARER_AUTH_TOKEN` and injects it:

Generated curl command:

```bash
curl -H 'Authorization: Bearer ${{env.BEARER_AUTH_TOKEN}}' https://api.example.com/pets
```

Before using the command, set the environment variable:

```bash
export BEARER_AUTH_TOKEN="your-api-token"
myapi pets list
```

### API Key Header

OpenAPI spec:

```yaml
components:
  securitySchemes:
    apiKey:
      type: apiKey
      in: header
      name: X-API-Key
security:
  - apiKey: []
```

Stampo converts to `API_KEY_TOKEN` and injects it:

```bash
curl -H 'X-API-Key: ${{env.API_KEY_TOKEN}}' https://api.example.com/pets
```

### Scheme Name Conversion

The environment variable name is derived from the security scheme name by converting camelCase to UPPER_SNAKE_CASE and appending `_TOKEN`:

| Scheme Name | Environment Variable |
|---|---|
| `bearerAuth` | `BEARER_AUTH_TOKEN` |
| `apiKey` | `API_KEY_TOKEN` |
| `github_token` | `GITHUB_TOKEN_TOKEN` |
| `MyCustomAuth` | `MY_CUSTOM_AUTH_TOKEN` |

## Refreshing OpenAPI Specs

When the API spec changes, refresh your cached commands:

```bash
myapi refresh-configuration
```

Stampo:

1. Fetches the spec again (if remote)
2. Compares hashes of the raw spec with the cached version
3. If changed, shows the diff and prompts you to trust the new version:

```
Config 'pets' has changed. Trust new version? [y/N]
```

4. Type `y` to accept or `n` to keep the old version

Skip the prompt with `--yes`:

```bash
myapi refresh-configuration --yes
```

### Handling Missing Specs

If the original spec URL or file is no longer available, refresh warns but keeps the cached copy working:

```
Warning: could not fetch https://api.example.com/openapi.yml
Using cached version from 2026-03-15
```

## Mixing OpenAPI and YAML Configs

You can combine OpenAPI-generated commands with custom YAML commands in a single alias. Each config becomes a top-level subcommand:

```bash
stampo init --alias infra \
  --config-path ./deploy.yml \
  --config-path openapi:./api-spec.yml
```

Usage:

```bash
infra deploy staging          # from deploy.yml
infra api pets list           # from openapi:./api-spec.yml (config name: "api")
infra api pets get --pet-id 5
```

Stampo generates the config name from the OpenAPI spec's `info.title` field (or falls back to the filename). To avoid conflicts, ensure config names are unique across all files.

### References File with OpenAPI

A references file can include both regular and OpenAPI configs:

```yaml
banner: "Welcome to Infrastructure CLI"
configs:
  - deploy.yml
  - monitoring.yml
  - openapi:./petstore.json
  - openapi:https://api.example.com/users.yml
```

## HTTP Tool Requirements

Stampo requires **curl** or **wget** to execute generated commands. At init time, stampo auto-detects which tool is available:

1. Check for `curl --version`
2. If not found, check for `wget --version`
3. If neither is found, error: "curl or wget is required for OpenAPI support"

Generated commands use whichever tool was detected. The tool is remembered in the alias metadata, so it doesn't need to be re-detected on every command run.

### Handling Missing Tools

If curl/wget becomes unavailable after init, stampo errors clearly:

```
Error: curl is not installed. Please install curl or wget to use this alias.
```

Install the missing tool and run `refresh-configuration`.

## Known Limitations

Stampo makes best-effort approximations for some OpenAPI features. Unsupported or partially-supported features:

| Feature | Behavior |
|---|---|
| **Webhook callbacks** | Skipped with warning |
| **XML-only content types** | Skipped; no JSON → no CLI command generated |
| **multipart/form-data (file uploads)** | Best-effort: generates `--file` flag for curl `-F` |
| **$ref schemas** | Not resolved; skipped with warning |
| **OAuth2, OpenIDConnect** | Skipped; only Bearer and API Key supported |
| **Cookie authentication** | Skipped |
| **Query/cookie API keys** | Skipped; only header API keys are supported |
| **anyOf / oneOf schemas** | Best-effort: generates flags for all possible fields |
| **allOf schemas** | Merged and flattened |
| **Swagger 2.0** | Rejected with error: "Use OpenAPI 3.0+" |
| **Server variables** | Ignored; first server URL used as-is |
| **Discriminators** | Ignored; all variants treated as possible fields |

When unsupported features are encountered, `stampo init` prints a summary:

```
Generated 12 commands from OpenAPI spec
Skipped 2 operations (unsupported content types)
1 operation uses best-effort approximations
```

Use `--verbose` during init to see detailed warnings:

```bash
stampo init --alias api --config-path openapi:./spec.yml --verbose
```

## Troubleshooting

### "curl or wget is required"

Stampo could not find curl or wget on your system.

**Solution:** Install one of them:

```bash
# macOS
brew install curl

# Linux
sudo apt-get install curl

# Windows (PowerShell)
choco install curl
```

Then run `stampo init` again.

### "Swagger 2.0 not yet supported"

Your OpenAPI spec is in Swagger 2.0 format.

**Solution:** Upgrade the spec to OpenAPI 3.0+ or use a tool like [Swagger Editor](https://editor.swagger.io) to convert it.

### "Invalid OpenAPI spec"

Stampo could not parse the spec as valid JSON or YAML.

**Troubleshooting:**

1. Validate the spec at [Swagger Editor](https://editor.swagger.io)
2. Check for JSON/YAML syntax errors
3. Ensure the spec includes required fields: `openapi`, `info.title`, `paths`

### Remote URL fails to download

The spec URL is unreachable or returns an error.

**Troubleshooting:**

```bash
# Test the URL manually
curl -I https://api.example.com/openapi.yml

# Use --yes to skip confirmation and see full error
stampo init --alias api --config-path openapi:https://api.example.com/openapi.yml --yes
```

### Generated commands are incomplete

Some API operations may be skipped due to unsupported content types.

**Solution:** Use `--verbose` to see details:

```bash
stampo init --alias api --config-path openapi:./spec.yml --verbose
```

Stampo will list all skipped operations and their reasons.

### Deeply nested flags are hard to type

For complex nested schemas, dot-notation flags can be long:

```bash
myapi pets create --owner.address.country.name "USA"
```

**Solution:** Use shell aliases or write a small wrapper script to provide shortcuts:

```bash
# In your shell config
alias my_create_pet='myapi pets create --owner.address.country.name'
```

## Examples

### Using Petstore API

Create an alias from the public Petstore API:

```bash
stampo init --alias petstore --config-path openapi:https://petstore.swagger.io/v2/swagger.json --yes
```

(Note: Swagger 2.0 will be rejected; see Known Limitations.)

For OpenAPI 3.0 example specs, visit [apis.guru](https://apis.guru).

### Creating a Config with Authentication

Example `api.yml` mixing OpenAPI with a manual deploy command:

```bash
stampo init --alias ops \
  --config-path ./deploy.yml \
  --config-path openapi:./user-api.json
```

Then use it:

```bash
# Set auth token
export API_KEY_TOKEN="secret123"

# Use the API
ops api users list
ops api users get --user-id 42

# Use regular commands
ops deploy staging
```

### Refreshing on Schedule

To keep specs fresh in CI/CD:

```bash
#!/bin/bash
# Fetch latest spec and refresh alias
curl -s https://api.example.com/openapi.json -o /tmp/latest.json
stampo init --alias api --config-path openapi:/tmp/latest.json --force --yes
```

## Architecture Details

For developers interested in how stampo transforms OpenAPI specs:

- Stampo parses the spec into an `openapiv3` structure
- Paths are organized into a command hierarchy tree based on non-parameter segments
- Path, query, and body parameters become flags
- Curl/wget commands are generated with proper headers, query strings, and JSON bodies
- The entire `Configuration` struct is cached in `~/.stampo/aliases/<name>/`
- On refresh, the raw spec hash is compared; if changed, you're prompted to trust the new version
- The cached copy continues working even if the original spec URL becomes unavailable
