//! HuggingFace Hub publishing — create dataset repos and upload training data.
//!
//! Security model (adopted from Localmotive):
//! - Token resolved on backend only (never crosses IPC from frontend)
//! - Token format validated (must start with "hf_")
//! - License validated against allowlist (prevents YAML injection in dataset card)
//! - Retry logic with Retry-After header support for rate limiting

use crate::error::{ClueditError, Result};
use crate::models::ExportFormat;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const HF_BASE_URL: &str = "https://huggingface.co";
const USER_AGENT: &str = "cluedit/0.1.0";
const MAX_RETRIES: u32 = 3;

const ALLOWED_LICENSES: &[&str] = &[
    "mit",
    "apache-2.0",
    "cc-by-4.0",
    "cc-by-sa-4.0",
    "cc-by-nc-4.0",
    "cc0-1.0",
    "openrail",
    "other",
];

// ============================================================================
// Public types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiResponse {
    pub name: String,
    pub fullname: Option<String>,
    #[serde(default)]
    pub orgs: Vec<OrgInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgInfo {
    pub name: String,
}

/// Config received from frontend — NO token (resolved on backend from store).
#[derive(Debug, Clone, Deserialize)]
pub struct PublishConfig {
    pub repo_name: String,
    pub namespace: Option<String>,
    pub private: bool,
    pub license: String,
    pub format: ExportFormat,
    pub project_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishResult {
    pub repo_url: String,
    pub commit_url: String,
    pub files_uploaded: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "step")]
#[allow(dead_code)]
pub enum PublishProgress {
    ValidatingToken,
    ExportingData,
    CreatingRepo,
    GeneratingCard,
    Uploading { current: usize, total: usize },
    Committing,
    Done,
}

// ============================================================================
// HF API request/response types (private)
// ============================================================================

#[derive(Serialize)]
struct CreateRepoRequest {
    name: String,
    #[serde(rename = "type")]
    repo_type: String,
    private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
}

#[derive(Serialize)]
struct CommitRequest {
    summary: String,
    files: Vec<CommitFile>,
}

#[derive(Serialize)]
struct CommitFile {
    path: String,
    content: String,
    encoding: String,
}

#[derive(Deserialize)]
struct CommitResponse {
    #[serde(rename = "commitUrl")]
    commit_url: String,
}

// ============================================================================
// HTTP client with retry
// ============================================================================

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| ClueditError::HfApi(format!("Failed to build HTTP client: {}", e)))
}

/// Send a request with retry on 429 (rate limit).
async fn send_with_retry(
    request_builder: impl Fn() -> reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    for attempt in 0..MAX_RETRIES {
        let resp = request_builder().send().await?;

        if resp.status().as_u16() != 429 {
            return Ok(resp);
        }

        // Rate limited — back off
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(2u64.pow(attempt).min(30));

        log::warn!(
            "Rate limited (attempt {}/{}), retrying in {}s",
            attempt + 1,
            MAX_RETRIES,
            retry_after
        );
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
    }

    // Final attempt
    Ok(request_builder().send().await?)
}

// ============================================================================
// Token persistence + validation
// ============================================================================

/// Validate token format (must start with "hf_").
fn validate_token_format(token: &str) -> Result<()> {
    let trimmed = token.trim();
    if !trimmed.starts_with("hf_") {
        return Err(ClueditError::HfAuth(
            "Invalid token format. HuggingFace tokens start with 'hf_'".to_string(),
        ));
    }
    if trimmed.len() < 10 {
        return Err(ClueditError::HfAuth("Token too short".to_string()));
    }
    Ok(())
}

/// Read HF token: env var first, then saved file.
pub fn read_saved_token(data_dir: &Path) -> Option<String> {
    if let Ok(token) = std::env::var("HF_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }

    let token_path = data_dir.join("hf_token.json");
    if let Ok(content) = std::fs::read_to_string(&token_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            return v.get("token").and_then(|t| t.as_str()).map(String::from);
        }
    }

    None
}

/// Save HF token to app data directory.
pub fn save_token(data_dir: &Path, token: &str) -> Result<()> {
    let token_path = data_dir.join("hf_token.json");
    let content = serde_json::json!({ "token": token });
    std::fs::write(&token_path, serde_json::to_string_pretty(&content)?)?;
    Ok(())
}

/// Delete saved HF token.
pub fn delete_token(data_dir: &Path) {
    let token_path = data_dir.join("hf_token.json");
    let _ = std::fs::remove_file(&token_path);
}

// ============================================================================
// HF API operations
// ============================================================================

/// Validate a HuggingFace token and return user info.
pub async fn validate_token(token: &str) -> Result<WhoamiResponse> {
    validate_token_format(token)?;
    let client = build_client()?;

    let resp = send_with_retry(|| {
        client
            .get(format!("{}/api/whoami-v2", HF_BASE_URL))
            .bearer_auth(token)
    })
    .await?;

    match resp.status().as_u16() {
        200 => Ok(resp.json::<WhoamiResponse>().await?),
        401 => Err(ClueditError::HfAuth("Invalid or expired token".to_string())),
        403 => Err(ClueditError::HfAuth(
            "Token needs write scope. Create a new token at huggingface.co/settings/tokens"
                .to_string(),
        )),
        status => Err(ClueditError::HfApi(format!(
            "Unexpected response: HTTP {}",
            status
        ))),
    }
}

/// Publish training data to a HuggingFace dataset repository.
/// Token is resolved from data_dir (env var or saved file), NOT from config.
pub async fn publish_dataset(
    config: &PublishConfig,
    token: &str,
    content: &str,
    sample_count: usize,
    app_handle: &AppHandle,
) -> Result<PublishResult> {
    // Validate all inputs before any network calls
    validate_token_format(token)?;
    validate_license(&config.license)?;
    validate_hf_name(&config.repo_name, "Repository name")?;
    if let Some(ns) = &config.namespace {
        validate_hf_name(ns, "Namespace")?;
    }
    if content.len() > MAX_INLINE_CONTENT_BYTES {
        return Err(ClueditError::HfApi(format!(
            "Dataset too large for direct upload ({:.1} MB). Export to disk and use `huggingface-cli upload` instead.",
            content.len() as f64 / 1_048_576.0
        )));
    }

    let client = build_client()?;

    // Step 1: Validate token and get namespace
    emit_progress(app_handle, PublishProgress::ValidatingToken);
    let whoami = validate_token(token).await?;
    let namespace = config.namespace.as_deref().unwrap_or(&whoami.name);

    // Step 2: Create repo
    emit_progress(app_handle, PublishProgress::CreatingRepo);
    let repo_url = create_repo(
        &client,
        token,
        &config.repo_name,
        namespace,
        config.private,
        &config.license,
    )
    .await?;

    // Step 3: Generate dataset card
    emit_progress(app_handle, PublishProgress::GeneratingCard);
    let card = generate_dataset_card(config, namespace, sample_count);

    // Step 4: Commit files
    let format_name = format_filename(&config.format);
    let data_filename = format!("train_{}.jsonl", format_name);

    emit_progress(
        app_handle,
        PublishProgress::Uploading {
            current: 1,
            total: 2,
        },
    );

    emit_progress(app_handle, PublishProgress::Committing);
    let commit_resp = commit_files(
        &client,
        token,
        namespace,
        &config.repo_name,
        "Upload training data from CluEdit",
        vec![
            CommitFile {
                path: "README.md".to_string(),
                content: card,
                encoding: "utf-8".to_string(),
            },
            CommitFile {
                path: data_filename,
                content: content.to_string(),
                encoding: "utf-8".to_string(),
            },
        ],
    )
    .await?;

    emit_progress(app_handle, PublishProgress::Done);

    Ok(PublishResult {
        repo_url,
        commit_url: commit_resp.commit_url,
        files_uploaded: 2,
    })
}

// ============================================================================
// Private helpers
// ============================================================================

/// Validate a HF repo name or namespace (prevents URL path injection).
fn validate_hf_name(name: &str, label: &str) -> Result<()> {
    if name.is_empty() || name.len() > 96 {
        return Err(ClueditError::HfApi(format!(
            "{} must be 1-96 characters",
            label
        )));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(ClueditError::HfApi(format!(
            "{} must contain only alphanumeric characters, hyphens, underscores, or dots",
            label
        )));
    }
    if name.starts_with('.') || name.starts_with('-') {
        return Err(ClueditError::HfApi(format!(
            "{} must start with an alphanumeric character",
            label
        )));
    }
    Ok(())
}

/// Sanitize an API error body for display (truncate + strip control chars).
fn sanitize_api_error(body: &str, max_len: usize) -> String {
    let truncated = if body.len() > max_len {
        &body[..body
            .char_indices()
            .take(max_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(body.len())]
    } else {
        body
    };
    truncated.replace(|c: char| c.is_control(), "")
}

/// Escape a string for safe interpolation into YAML.
fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', "")
}

/// Maximum inline content size (5MB). Beyond this, a clear error is returned.
const MAX_INLINE_CONTENT_BYTES: usize = 5 * 1024 * 1024;

fn validate_license(license: &str) -> Result<()> {
    if !ALLOWED_LICENSES.contains(&license) {
        return Err(ClueditError::HfApi(format!(
            "Invalid license '{}'. Allowed: {}",
            license,
            ALLOWED_LICENSES.join(", ")
        )));
    }
    Ok(())
}

fn emit_progress(app_handle: &AppHandle, progress: PublishProgress) {
    if let Err(e) = app_handle.emit("publish-progress", &progress) {
        log::warn!("Failed to emit publish progress: {}", e);
    }
}

async fn create_repo(
    client: &reqwest::Client,
    token: &str,
    name: &str,
    namespace: &str,
    private: bool,
    license: &str,
) -> Result<String> {
    let resp = send_with_retry(|| {
        client
            .post(format!("{}/api/repos/create", HF_BASE_URL))
            .bearer_auth(token)
            .json(&CreateRepoRequest {
                name: format!("{}/{}", namespace, name),
                repo_type: "dataset".to_string(),
                private,
                license: Some(license.to_string()),
            })
    })
    .await?;

    match resp.status().as_u16() {
        200 | 201 => Ok(format!("{}/datasets/{}/{}", HF_BASE_URL, namespace, name)),
        409 => Ok(format!("{}/datasets/{}/{}", HF_BASE_URL, namespace, name)),
        401 => Err(ClueditError::HfAuth("Token expired or invalid".to_string())),
        403 => Err(ClueditError::HfAuth(
            "Token lacks write permissions".to_string(),
        )),
        422 => {
            let text = resp.text().await.unwrap_or_default();
            Err(ClueditError::HfApi(format!(
                "Invalid repository name: {}",
                sanitize_api_error(&text, 200)
            )))
        }
        status => {
            let text = resp.text().await.unwrap_or_default();
            Err(ClueditError::HfApi(format!(
                "Failed to create repo: HTTP {} - {}",
                status,
                sanitize_api_error(&text, 200)
            )))
        }
    }
}

async fn commit_files(
    client: &reqwest::Client,
    token: &str,
    namespace: &str,
    repo: &str,
    summary: &str,
    files: Vec<CommitFile>,
) -> Result<CommitResponse> {
    // Pre-serialize to avoid cloning file content on retries
    let body = serde_json::to_vec(&CommitRequest {
        summary: summary.to_string(),
        files,
    })?;
    let url = format!(
        "{}/api/datasets/{}/{}/commit/main",
        HF_BASE_URL, namespace, repo
    );

    let resp = send_with_retry(|| {
        client
            .post(&url)
            .bearer_auth(token)
            .header("content-type", "application/json")
            .body(body.clone())
    })
    .await?;

    match resp.status().as_u16() {
        200 => Ok(resp.json::<CommitResponse>().await?),
        401 => Err(ClueditError::HfAuth(
            "Token expired during upload".to_string(),
        )),
        status => {
            let text = resp.text().await.unwrap_or_default();
            Err(ClueditError::HfApi(format!(
                "Commit failed: HTTP {} - {}",
                status,
                sanitize_api_error(&text, 200)
            )))
        }
    }
}

fn format_filename(format: &ExportFormat) -> &'static str {
    match format {
        ExportFormat::ChatML => "chatml",
        ExportFormat::ChatMLTools => "chatml_tools",
        ExportFormat::ShareGPT => "sharegpt",
        ExportFormat::Alpaca => "alpaca",
        _ => "data",
    }
}

// ============================================================================
// Dataset card generation
// ============================================================================

fn generate_dataset_card(config: &PublishConfig, namespace: &str, sample_count: usize) -> String {
    let size_category = match sample_count {
        0..=999 => "n<1K",
        1000..=9999 => "1K<n<10K",
        10000..=99999 => "10K<n<100K",
        100000..=999999 => "100K<n<1M",
        _ => "1M<n<10M",
    };

    let format_name = match &config.format {
        ExportFormat::ChatML => "ChatML (OpenAI)",
        ExportFormat::ChatMLTools => "ChatML with Tools",
        ExportFormat::ShareGPT => "ShareGPT",
        ExportFormat::Alpaca => "Alpaca",
        _ => "JSONL",
    };

    let features_yaml = match &config.format {
        ExportFormat::ChatML | ExportFormat::ChatMLTools => {
            "  features:\n    - name: messages\n      list:\n        - name: role\n          dtype: string\n        - name: content\n          dtype: string"
        }
        ExportFormat::ShareGPT => {
            "  features:\n    - name: conversations\n      list:\n        - name: from\n          dtype: string\n        - name: value\n          dtype: string"
        }
        ExportFormat::Alpaca => {
            "  features:\n    - name: instruction\n      dtype: string\n    - name: input\n      dtype: string\n    - name: output\n      dtype: string"
        }
        _ => "  features:\n    - name: text\n      dtype: string",
    };

    let data_file = format!("train_{}.jsonl", format_filename(&config.format));
    let timestamp = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let safe_name = escape_yaml(&config.repo_name);

    format!(
        r#"---
license: {license}
language:
  - en
pretty_name: "{safe_name}"
size_categories:
  - "{size_category}"
tags:
  - code
  - synthetic
  - cluedit
  - fine-tuning
task_categories:
  - text-generation
configs:
  - config_name: default
    data_files:
      - split: train
        path: "{data_file}"
dataset_info:
{features_yaml}
---

# {repo_name}

LLM fine-tuning training data exported from AI coding assistant conversations by [CluEdit](https://github.com/nickpaterno/cluedit).

## Dataset Details

- **Source:** {namespace}/{repo_name}
- **Generated:** {timestamp}
- **Total samples:** {sample_count}
- **Format:** {format_name}

## Format

{format_name} format — each line is a complete training example.

## Usage

```python
from datasets import load_dataset
ds = load_dataset("{namespace}/{repo_name}")
```

## Generation

Exported with CluEdit from Claude Code and Codex CLI conversation history.
"#,
        license = config.license,
        repo_name = config.repo_name,
        size_category = size_category,
        data_file = data_file,
        features_yaml = features_yaml,
        namespace = namespace,
        timestamp = timestamp,
        sample_count = sample_count,
        format_name = format_name,
    )
}
