// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).

// `capture.rs` -- Capture CodeChat Editor Events
// ============================================================================
//
// This module provides a durable local FIFO spool and an HTTPS upload worker for
// CaptureWebService. CodeChat never stores PostgreSQL credentials and never
// connects directly to the capture database. The VS Code extension stores the
// portal-issued bearer token in SecretStorage, then passes it to this worker in
// memory so pending capture events can upload through the public web service.

// Imports
// -------
//
// ### Standard library
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

// ### Third-party
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

static NEXT_CAPTURE_EVENT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_SPOOL_FILE_ID: AtomicU64 = AtomicU64::new(1);

const DEFAULT_CAPTURE_SCHEMA_VERSION: i32 = 2;
const MAX_CAPTURE_BATCH_EVENTS: usize = 100;
const MAX_CAPTURE_BATCH_BYTES: usize = 524_288;
const INITIAL_RETRY_DELAY_MS: u64 = 1_000;
const MAX_RETRY_DELAY_MS: u64 = 60_000;
const CAPTURE_SERVICE_HTTP_TIMEOUT_SECS: u64 = 10;

/// Canonical event types. Keep the serialized strings stable for analysis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureEventType {
    /// Edit to documentation/prose. In CodeChat files this means doc blocks;
    /// fenced or embedded code content is classified as `WriteCode`.
    WriteDoc,
    /// Edit to executable source code, including code inside CodeChat blocks.
    WriteCode,
    /// Editor activity moved between documentation and code contexts.
    SwitchPane,
    /// Duration summary for a documentation/prose activity interval.
    DocSession,
    /// File save observed by the editor.
    Save,
    /// Compile/build task started.
    Compile,
    /// Debug/run session started.
    Run,
    /// Capture or activity session started.
    SessionStart,
    /// Capture or activity session ended.
    SessionEnd,
    /// Consent or recording settings changed.
    CaptureSettingsChanged,
    /// Compile/build task ended.
    CompileEnd,
    /// Debug/run session ended.
    RunEnd,
    /// Study task started by an external study workflow.
    TaskStart,
    /// Study task submitted by an external study workflow.
    TaskSubmit,
    /// Debugging study task started by an external study workflow.
    DebugTaskStart,
    /// Debugging study task submitted by an external study workflow.
    DebugTaskSubmit,
    /// Collaboration handoff interval started.
    HandoffStart,
    /// Collaboration handoff interval ended.
    HandoffEnd,
    /// A built-in reflection prompt was inserted into the active editor.
    ReflectionPromptInserted,
}

impl CaptureEventType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WriteDoc => "write_doc",
            Self::WriteCode => "write_code",
            Self::SwitchPane => "switch_pane",
            Self::DocSession => "doc_session",
            Self::Save => "save",
            Self::Compile => "compile",
            Self::Run => "run",
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::CaptureSettingsChanged => "capture_settings_changed",
            Self::CompileEnd => "compile_end",
            Self::RunEnd => "run_end",
            Self::TaskStart => "task_start",
            Self::TaskSubmit => "task_submit",
            Self::DebugTaskStart => "debug_task_start",
            Self::DebugTaskSubmit => "debug_task_submit",
            Self::HandoffStart => "handoff_start",
            Self::HandoffEnd => "handoff_end",
            Self::ReflectionPromptInserted => "reflection_prompt_inserted",
        }
    }
}

impl std::fmt::Display for CaptureEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Hash a local file path before it enters capture storage. The hash is stable
/// enough to group edits to the same file while avoiding raw path collection.
#[must_use]
pub fn hash_capture_path(path: &str) -> String {
    hash_capture_text(path)
}

fn hash_capture_token(token: &str) -> String {
    hash_capture_text(token)
}

fn hash_capture_text(value: &str) -> String {
    use std::fmt::Write;
    Sha256::digest(value.as_bytes())
        .iter()
        .fold(String::new(), |mut acc, byte| {
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}

/// JSON payload received from local clients for capture events.
///
/// The server supplies the authoritative timestamp and hashes any raw local file
/// path before storage. Study metadata such as course, assignment, group,
/// condition, and task is inferred later from researcher-managed mappings keyed
/// by the pseudonymous `user_id` and event timestamps.
#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, optional_fields)]
pub struct CaptureEventWire {
    /// Client-generated unique event identifier. Unlike `sequence_number`, this
    /// is an opaque stable ID for correlation and possible future deduplication
    /// across capture transports or retries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// Event order within one `(session_id, event_source)` stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_number: Option<i64>,
    /// Capture payload schema version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i32>,
    /// Pseudonymous participant UUID from CaptureWebService token status.
    pub user_id: String,
    /// Logical capture session UUID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Source of this event, such as the VS Code extension or server translation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_source: Option<String>,
    /// VS Code language identifier for the active file, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_id: Option<String>,
    /// Raw local file path from a trusted local client. This value exists only
    /// long enough for the Rust server to hash it; it is never spooled or sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// SHA-256 hash of the local file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    /// Canonical capture event type.
    pub event_type: CaptureEventType,
    /// Optional client timezone offset in minutes (JS Date().getTimezoneOffset()).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_tz_offset_min: Option<i32>,
    /// Event-specific data. Do not store source text or raw local paths here.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "unknown")]
    pub data: Option<serde_json::Value>,
}

/// Participant and session metadata remembered from client capture events.
#[derive(Clone, Debug, Default)]
pub(crate) struct CaptureContext {
    active: bool,
    user_id: Option<String>,
    event_source: Option<String>,
    session_id: Option<String>,
    client_tz_offset_min: Option<i32>,
    schema_version: Option<i32>,
    server_sequence_number: i64,
}

impl CaptureContext {
    /// Refresh server-side capture identity and active/inactive state from an
    /// extension capture message.
    pub(crate) fn update_from_wire(&mut self, wire: &CaptureEventWire) {
        match wire.event_type {
            CaptureEventType::SessionStart => self.active = true,
            CaptureEventType::SessionEnd => self.active = false,
            _ => {}
        }
        if !wire.user_id.trim().is_empty() {
            self.user_id = Some(wire.user_id.clone());
        }
        if let Some(event_source) = &wire.event_source {
            self.event_source = Some(event_source.clone());
        }
        if let Some(session_id) = &wire.session_id {
            if self.session_id.as_ref() != Some(session_id) {
                self.server_sequence_number = 0;
            }
            self.session_id = Some(session_id.clone());
        }
        if let Some(schema_version) = wire.schema_version {
            self.schema_version = Some(schema_version);
        }
        if let Some(client_tz_offset_min) = wire.client_tz_offset_min {
            self.client_tz_offset_min = Some(client_tz_offset_min);
        }
        if let Some(serde_json::Value::Object(data)) = &wire.data
            && let Some(active) = data
                .get("capture_active")
                .and_then(serde_json::Value::as_bool)
        {
            self.active = active;
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn capture_event(
        &mut self,
        event_type: CaptureEventType,
        file_path: Option<String>,
        data: serde_json::Value,
    ) -> Option<CaptureEventWire> {
        if !self.active {
            return None;
        }
        let mut data = match data {
            serde_json::Value::Object(map) => map,
            other => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), other);
                map
            }
        };
        data.entry("source".to_string())
            .or_insert_with(|| serde_json::json!("server_translation"));

        self.server_sequence_number += 1;

        Some(CaptureEventWire {
            event_id: None,
            sequence_number: Some(self.server_sequence_number),
            schema_version: self.schema_version,
            user_id: self.user_id.clone()?,
            session_id: self.session_id.clone(),
            event_source: Some("server_translation".to_string()),
            language_id: None,
            file_path,
            file_hash: None,
            event_type,
            client_tz_offset_min: self.client_tz_offset_min,
            data: Some(serde_json::Value::Object(data)),
        })
    }
}

/// True for a capture message that should update `CaptureContext` only.
pub(crate) fn capture_control_only(wire: &CaptureEventWire) -> bool {
    matches!(
        &wire.data,
        Some(serde_json::Value::Object(data))
            if data
                .get("capture_control_only")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
    )
}

/// Runtime service configuration supplied by the VS Code extension. The bearer
/// token is stored in VS Code SecretStorage and held only in memory here.
#[derive(Debug, Clone, Default)]
pub struct CaptureServiceConfig {
    pub base_url: Option<String>,
    token: Option<String>,
    pub participant_id: Option<String>,
    pub instance_id: Option<String>,
    generation: u64,
}

impl CaptureServiceConfig {
    fn configured(base_url: &str, token: Option<String>) -> Result<Self, String> {
        let base_url = normalize_service_base_url(base_url)?;
        Ok(Self {
            base_url: Some(base_url),
            token: token.filter(|token| !token.trim().is_empty()),
            participant_id: None,
            instance_id: None,
            generation: 0,
        })
    }

    fn token(&self) -> Option<&str> {
        self.token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
    }

    fn status_url(&self) -> Option<String> {
        self.base_url
            .as_deref()
            .map(|base_url| format!("{base_url}/v1/capture/status"))
    }

    fn events_url(&self) -> Option<String> {
        self.base_url
            .as_deref()
            .map(|base_url| format!("{base_url}/v1/capture/events"))
    }

    fn token_hash(&self) -> Option<String> {
        self.token().map(hash_capture_token)
    }

    fn spool_identity(&self) -> Option<SpoolIdentity> {
        if self.token().is_none() && self.base_url.is_none() {
            return None;
        }
        Some(SpoolIdentity {
            token_hash: self.token_hash(),
            service_base_url: self.base_url.clone(),
            participant_id: self.participant_id.clone(),
            instance_id: self.instance_id.clone(),
        })
    }

    fn matches_request_snapshot(&self, snapshot: &Self) -> bool {
        self.generation == snapshot.generation
            && self.base_url == snapshot.base_url
            && self.token_hash() == snapshot.token_hash()
    }
}

fn normalize_service_base_url(value: &str) -> Result<String, String> {
    let mut url = value.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return Err("capture service URL must not be empty".to_string());
    }
    for suffix in ["/v1/capture/events", "/v1/capture/status", "/v1/health"] {
        if let Some(base) = url.strip_suffix(suffix) {
            url = base.trim_end_matches('/').to_string();
            break;
        }
    }

    let mut parsed =
        url::Url::parse(&url).map_err(|_| "capture service URL must be absolute".to_string())?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("capture service URL must not include credentials".to_string());
    }
    let local_http = parsed.scheme() == "http"
        && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if parsed.scheme() != "https" && !local_http {
        return Err("capture service URL must use https:// except for localhost".to_string());
    }

    parsed.set_query(None);
    parsed.set_fragment(None);
    let trimmed_path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&trimmed_path);
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

/// Known capture worker states reported to the VS Code status UI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureState {
    /// Capture worker is not available.
    Disabled,
    /// Capture worker is starting.
    Starting,
    /// Events are being written to the local FIFO spool.
    Spooling,
    /// Events are being uploaded to CaptureWebService.
    Uploading,
    /// The local spool is empty and the remote service is reachable.
    Remote,
    /// The token was rejected by the service.
    AuthFailed,
    /// The service knows the token, but capture is currently not allowed.
    CaptureDisabled,
    /// The service or network is temporarily unavailable.
    ServiceUnavailable,
}

/// Non-secret token state shown in the capture UI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureTokenStatus {
    Missing,
    Unverified,
    Accepted,
    Rejected,
    CaptureDisabled,
}

/// Capture worker health exposed to the VS Code status item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export)]
pub struct CaptureStatus {
    pub enabled: bool,
    pub state: CaptureState,
    pub token_status: CaptureTokenStatus,
    pub queued_events: u64,
    pub spooled_events: u64,
    pub uploaded_events: u64,
    pub failed_events: u64,
    pub quarantined_events: u64,
    pub last_error: Option<String>,
    pub spool_path: Option<PathBuf>,
    pub service_base_url: Option<String>,
    pub participant_id: Option<String>,
    pub instance_id: Option<String>,
    pub capture_enabled: Option<bool>,
    pub participant_status: Option<String>,
    pub consent_status: Option<String>,
    pub instance_status: Option<String>,
    pub token_expires_at: Option<String>,
    pub service_version: Option<String>,
    pub last_status_check_at: Option<String>,
    pub last_upload_at: Option<String>,
}

impl CaptureStatus {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            state: CaptureState::Disabled,
            token_status: CaptureTokenStatus::Missing,
            queued_events: 0,
            spooled_events: 0,
            uploaded_events: 0,
            failed_events: 0,
            quarantined_events: 0,
            last_error: None,
            spool_path: None,
            service_base_url: None,
            participant_id: None,
            instance_id: None,
            capture_enabled: None,
            participant_status: None,
            consent_status: None,
            instance_status: None,
            token_expires_at: None,
            service_version: None,
            last_status_check_at: None,
            last_upload_at: None,
        }
    }

    fn starting(spool_path: PathBuf) -> Self {
        Self {
            enabled: true,
            state: CaptureState::Starting,
            token_status: CaptureTokenStatus::Missing,
            spool_path: Some(spool_path),
            ..Self::disabled()
        }
    }
}

/// The in-memory representation of a single capture event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureEvent {
    pub event_id: Option<String>,
    pub sequence_number: Option<i64>,
    pub schema_version: Option<i32>,
    pub user_id: String,
    pub session_id: Option<String>,
    pub event_source: Option<String>,
    pub language_id: Option<String>,
    pub file_hash: Option<String>,
    pub event_type: CaptureEventType,
    pub timestamp: DateTime<Utc>,
    pub client_tz_offset_min: Option<i32>,
    pub data: serde_json::Value,
}

impl CaptureEvent {
    #[must_use]
    pub fn new(
        user_id: String,
        file_hash: Option<String>,
        event_type: CaptureEventType,
        timestamp: DateTime<Utc>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            event_id: None,
            sequence_number: None,
            schema_version: None,
            user_id,
            session_id: None,
            event_source: None,
            language_id: None,
            file_hash,
            event_type,
            timestamp,
            client_tz_offset_min: None,
            data,
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn with_columns(
        event_id: Option<String>,
        sequence_number: Option<i64>,
        schema_version: Option<i32>,
        user_id: String,
        session_id: Option<String>,
        event_source: Option<String>,
        language_id: Option<String>,
        file_hash: Option<String>,
        event_type: CaptureEventType,
        timestamp: DateTime<Utc>,
        client_tz_offset_min: Option<i32>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            event_id,
            sequence_number,
            schema_version,
            user_id,
            session_id,
            event_source,
            language_id,
            file_hash,
            event_type,
            timestamp,
            client_tz_offset_min,
            data,
        }
    }

    #[must_use]
    pub fn now(
        user_id: String,
        file_hash: Option<String>,
        event_type: CaptureEventType,
        data: serde_json::Value,
    ) -> Self {
        Self::new(user_id, file_hash, event_type, Utc::now(), data)
    }
}

/// Generate a server-side event ID for events classified after the original
/// extension message has been processed.
pub fn generate_capture_event_id(prefix: &str) -> String {
    let counter = NEXT_CAPTURE_EVENT_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "{prefix}-{}-{}-{counter}",
        process::id(),
        Utc::now().timestamp_micros()
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SpoolIdentity {
    token_hash: Option<String>,
    service_base_url: Option<String>,
    participant_id: Option<String>,
    instance_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpoolRecord {
    spooled_at: DateTime<Utc>,
    #[serde(default)]
    identity: Option<SpoolIdentity>,
    event: CaptureEvent,
}

#[derive(Debug, Serialize)]
struct CaptureBatchRequest {
    schema_version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    participant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    client_sent_at: String,
    events: Vec<CaptureServiceEvent>,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureServiceEvent {
    event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<i32>,
    user_id: String,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_hash: Option<String>,
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_tz_offset_min: Option<i32>,
    client_event_time: String,
    data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureServiceStatusResponse {
    pub participant_id: String,
    pub instance_id: String,
    pub study_id: String,
    pub capture_enabled: bool,
    pub participant_status: String,
    pub consent_status: String,
    pub instance_status: String,
    pub token_expires_at: Option<String>,
    pub server_time: String,
    pub service_version: String,
}

#[derive(Debug, Deserialize)]
struct CaptureBatchAcceptedResponse {
    batch_id: String,
    accepted: u64,
    server_time: String,
}

#[derive(Debug)]
struct SpoolBatch {
    files: Vec<PathBuf>,
    body: Vec<u8>,
    event_count: u64,
}

#[derive(Debug)]
enum NextBatch {
    Batch(SpoolBatch),
    Empty,
    NoMatchingIdentity,
}

#[derive(Debug)]
struct CaptureHttpError {
    status_code: Option<i32>,
    message: String,
}

impl CaptureHttpError {
    fn transport(message: impl Into<String>) -> Self {
        Self {
            status_code: None,
            message: message.into(),
        }
    }

    fn response(status_code: i32, message: impl Into<String>) -> Self {
        Self {
            status_code: Some(status_code),
            message: message.into(),
        }
    }

    fn is_transient(&self) -> bool {
        matches!(self.status_code, None | Some(429 | 500 | 503))
            || matches!(self.status_code, Some(code) if code >= 500)
    }
}

enum WorkerMsg {
    Flush,
}

/// Handle used by the rest of the server to record events.
#[derive(Clone)]
pub struct EventCapture {
    tx: Sender<WorkerMsg>,
    status: Arc<Mutex<CaptureStatus>>,
    config: Arc<Mutex<CaptureServiceConfig>>,
    spool_path: PathBuf,
}

impl EventCapture {
    pub fn new(spool_path: PathBuf) -> Result<Self, io::Error> {
        fs::create_dir_all(&spool_path)?;
        fs::create_dir_all(quarantine_path(&spool_path))?;

        let status = Arc::new(Mutex::new(CaptureStatus::starting(spool_path.clone())));
        update_spool_count(&spool_path, &status);

        let config = Arc::new(Mutex::new(CaptureServiceConfig::default()));
        let (tx, rx) = mpsc::channel::<WorkerMsg>();
        let status_worker = status.clone();
        let config_worker = config.clone();
        let spool_worker = spool_path.clone();

        thread::Builder::new()
            .name("codechat-capture-upload".to_string())
            .spawn(move || upload_worker(&rx, &config_worker, &status_worker, &spool_worker))
            .map_err(|err| {
                io::Error::other(format!("Capture: failed to start upload worker: {err}"))
            })?;

        Ok(Self {
            tx,
            status,
            config,
            spool_path,
        })
    }

    pub fn configure_service(&self, base_url: &str, token: Option<String>) -> Result<(), String> {
        let mut new_config = CaptureServiceConfig::configured(base_url, token)?;
        let token_status = if new_config.token().is_some() {
            CaptureTokenStatus::Unverified
        } else {
            CaptureTokenStatus::Missing
        };
        {
            let mut config = self
                .config
                .lock()
                .map_err(|_| "capture service config lock is poisoned".to_string())?;
            new_config.generation = config.generation.saturating_add(1);
            *config = new_config.clone();
        }
        update_status(&self.status, |status| {
            status.enabled = true;
            status.state = CaptureState::Spooling;
            status.token_status = token_status;
            status.service_base_url.clone_from(&new_config.base_url);
            status.participant_id = None;
            status.instance_id = None;
            status.capture_enabled = None;
            status.participant_status = None;
            status.consent_status = None;
            status.instance_status = None;
            status.token_expires_at = None;
            status.service_version = None;
            status.last_error = None;
        });
        update_spool_count(&self.spool_path, &self.status);
        self.signal_flush();
        Ok(())
    }

    pub fn clear_token(&self) {
        if let Ok(mut config) = self.config.lock() {
            config.generation = config.generation.saturating_add(1);
            config.token = None;
            config.participant_id = None;
            config.instance_id = None;
        }
        update_status(&self.status, |status| {
            status.state = CaptureState::Spooling;
            status.token_status = CaptureTokenStatus::Missing;
            status.participant_id = None;
            status.instance_id = None;
            status.capture_enabled = None;
            status.participant_status = None;
            status.consent_status = None;
            status.instance_status = None;
            status.token_expires_at = None;
            status.service_version = None;
            status.last_error = Some("Capture token is not configured".to_string());
        });
        update_spool_count(&self.spool_path, &self.status);
    }

    pub fn check_service_status(&self) -> Result<CaptureServiceStatusResponse, String> {
        let cfg = self
            .config
            .lock()
            .map_err(|_| "capture service config lock is poisoned".to_string())?
            .clone();
        let response = request_capture_status(&cfg).map_err(|err| {
            apply_http_error_if_current(&self.config, &self.status, &cfg, &err);
            err.message
        })?;
        if !apply_capture_service_status_if_current(
            &self.config,
            &self.status,
            &cfg,
            response.clone(),
        ) {
            return Err("Capture service status response was stale".to_string());
        }
        Ok(response)
    }

    /// Durably append an event to the local FIFO spool, then ask the worker to
    /// upload as soon as service access permits.
    pub fn log(&self, event: &CaptureEvent) {
        debug!(
            "Capture: spooling event: type={}, user_id={}, file_hash={:?}",
            event.event_type, event.user_id, event.file_hash
        );

        let spool_identity = match self.config.lock() {
            Ok(config) => config.spool_identity(),
            Err(err) => {
                error!("Capture: FAILED to read capture config before spooling: {err}");
                update_status(&self.status, |status| {
                    status.failed_events += 1;
                    status.last_error = Some(format!(
                        "Failed to read capture config before spooling: {err}"
                    ));
                });
                return;
            }
        };

        match append_spool_event(&self.spool_path, event, spool_identity) {
            Ok(()) => {
                update_status(&self.status, |status| {
                    if matches!(
                        status.state,
                        CaptureState::Starting | CaptureState::Disabled
                    ) {
                        status.state = CaptureState::Spooling;
                    }
                    status.last_error = None;
                });
                update_spool_count(&self.spool_path, &self.status);
                self.signal_flush();
            }
            Err(err) => {
                error!("Capture: FAILED to append event to spool: {err}");
                update_status(&self.status, |status| {
                    status.failed_events += 1;
                    status.last_error =
                        Some(format!("Failed to append capture event to spool: {err}"));
                });
            }
        }
    }

    fn signal_flush(&self) {
        if let Err(err) = self.tx.send(WorkerMsg::Flush) {
            error!("Capture: FAILED to notify upload worker: {err}");
            update_status(&self.status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Failed to notify upload worker: {err}"));
            });
        }
    }

    #[must_use]
    pub fn status(&self) -> CaptureStatus {
        self.status.lock().map_or_else(
            |_| {
                let mut status = CaptureStatus::disabled();
                status.last_error = Some("Capture status lock is poisoned".to_string());
                status
            },
            |status| status.clone(),
        )
    }
}

fn update_status(status: &Arc<Mutex<CaptureStatus>>, f: impl FnOnce(&mut CaptureStatus)) {
    match status.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(err) => error!("Capture: unable to update status: {err}"),
    }
}

fn upload_worker(
    rx: &mpsc::Receiver<WorkerMsg>,
    config: &Arc<Mutex<CaptureServiceConfig>>,
    status: &Arc<Mutex<CaptureStatus>>,
    spool_path: &Path,
) {
    info!(
        "Capture: upload worker started with spool at {}.",
        spool_path.display()
    );
    let mut retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);
    loop {
        match upload_next_batch(spool_path, config, status) {
            UploadOutcome::UploadedBatch => {
                retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);
                continue;
            }
            UploadOutcome::NoEvents => {
                retry_delay = Duration::from_millis(INITIAL_RETRY_DELAY_MS);
            }
            UploadOutcome::NotConfigured | UploadOutcome::Paused => {}
            UploadOutcome::TransientFailure => {
                warn!(
                    "Capture: transient upload failure; retrying in {} ms.",
                    retry_delay.as_millis()
                );
            }
        }

        let wait_for = if matches!(
            capture_status_state(status),
            CaptureState::ServiceUnavailable
        ) {
            retry_delay
        } else {
            Duration::from_secs(30)
        };

        match rx.recv_timeout(wait_for) {
            Ok(WorkerMsg::Flush) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                warn!("Capture: upload worker channel closed; worker exiting.");
                break;
            }
        }

        if matches!(
            capture_status_state(status),
            CaptureState::ServiceUnavailable
        ) {
            retry_delay = retry_delay
                .saturating_mul(2)
                .min(Duration::from_millis(MAX_RETRY_DELAY_MS));
        }
    }
}

fn capture_status_state(status: &Arc<Mutex<CaptureStatus>>) -> CaptureState {
    status
        .lock()
        .map_or(CaptureState::Disabled, |status| status.state)
}

#[derive(Debug, PartialEq, Eq)]
enum UploadOutcome {
    UploadedBatch,
    NoEvents,
    NotConfigured,
    Paused,
    TransientFailure,
}

fn upload_next_batch(
    spool_path: &Path,
    config: &Arc<Mutex<CaptureServiceConfig>>,
    status: &Arc<Mutex<CaptureStatus>>,
) -> UploadOutcome {
    update_spool_count(spool_path, status);

    let cfg = match config.lock() {
        Ok(config) => config.clone(),
        Err(err) => {
            update_status(status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Capture config lock failed: {err}"));
            });
            return UploadOutcome::Paused;
        }
    };

    let Some(token) = cfg.token().map(str::to_string) else {
        update_status(status, |status| {
            status.state = CaptureState::Spooling;
            status.token_status = CaptureTokenStatus::Missing;
            status.last_error = Some("Capture token is not configured".to_string());
        });
        return UploadOutcome::NotConfigured;
    };
    let Some(events_url) = cfg.events_url() else {
        update_status(status, |status| {
            status.state = CaptureState::Spooling;
            status.last_error = Some("Capture service URL is not configured".to_string());
        });
        return UploadOutcome::NotConfigured;
    };

    if cfg.participant_id.is_none() || cfg.instance_id.is_none() {
        match request_capture_status(&cfg) {
            Ok(service_status) => {
                if !apply_capture_service_status_if_current(config, status, &cfg, service_status) {
                    return UploadOutcome::Paused;
                }
            }
            Err(err) => {
                apply_http_error_if_current(config, status, &cfg, &err);
                return if err.is_transient() {
                    UploadOutcome::TransientFailure
                } else {
                    UploadOutcome::Paused
                };
            }
        }
    }

    let cfg = match config.lock() {
        Ok(config) => config.clone(),
        Err(err) => {
            update_status(status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Capture config lock failed: {err}"));
            });
            return UploadOutcome::Paused;
        }
    };

    if !matches!(
        capture_status_state(status),
        CaptureState::Remote | CaptureState::Spooling | CaptureState::Uploading
    ) {
        return UploadOutcome::Paused;
    }

    let batch = match build_next_batch(spool_path, &cfg, status) {
        NextBatch::Batch(batch) => batch,
        NextBatch::Empty => {
            update_status(status, |status| {
                if !matches!(
                    status.state,
                    CaptureState::AuthFailed | CaptureState::CaptureDisabled
                ) {
                    status.state = CaptureState::Remote;
                }
            });
            return UploadOutcome::NoEvents;
        }
        NextBatch::NoMatchingIdentity => {
            update_status(status, |status| {
                status.state = CaptureState::Spooling;
                status.last_error =
                    Some("Pending capture events belong to a different capture token".to_string());
            });
            return UploadOutcome::NoEvents;
        }
    };

    if !run_if_capture_config_snapshot_is_current(config, &cfg, || {
        update_status(status, |status| {
            status.state = CaptureState::Uploading;
            status.last_error = None;
        });
    }) {
        return UploadOutcome::Paused;
    }

    match post_capture_batch(&events_url, &token, &batch.body) {
        Ok(accepted) => {
            if !run_if_capture_config_snapshot_is_current(config, &cfg, || {
                for file in &batch.files {
                    if let Err(err) = fs::remove_file(file) {
                        warn!(
                            "Capture: unable to remove uploaded spool file {}: {err}",
                            file.display()
                        );
                    }
                }
                update_status(status, |status| {
                    status.state = CaptureState::Remote;
                    status.uploaded_events += accepted.accepted;
                    status.spooled_events = status.spooled_events.saturating_sub(batch.event_count);
                    status.last_upload_at = Some(accepted.server_time.clone());
                    status.last_error = None;
                });
            }) {
                return UploadOutcome::Paused;
            }
            update_spool_count(spool_path, status);
            debug!(
                "Capture: uploaded batch {} with {} event(s).",
                accepted.batch_id, accepted.accepted
            );
            UploadOutcome::UploadedBatch
        }
        Err(err) => match err.status_code {
            Some(401 | 403) => {
                apply_http_error_if_current(config, status, &cfg, &err);
                UploadOutcome::Paused
            }
            Some(400 | 413) => {
                if !run_if_capture_config_snapshot_is_current(config, &cfg, || {
                    quarantine_files(
                        spool_path,
                        &batch.files,
                        &format!("Capture service rejected batch: {}", err.message),
                        status,
                    );
                }) {
                    return UploadOutcome::Paused;
                }
                update_spool_count(spool_path, status);
                UploadOutcome::UploadedBatch
            }
            _ if err.is_transient() => {
                apply_http_error_if_current(config, status, &cfg, &err);
                UploadOutcome::TransientFailure
            }
            _ => {
                apply_http_error_if_current(config, status, &cfg, &err);
                UploadOutcome::Paused
            }
        },
    }
}

fn append_spool_event(
    spool_path: &Path,
    event: &CaptureEvent,
    identity: Option<SpoolIdentity>,
) -> io::Result<()> {
    fs::create_dir_all(spool_path)?;
    let counter = NEXT_SPOOL_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = Utc::now().format("%Y%m%d%H%M%S%6f");
    let path = spool_path.join(format!(
        "{}-{}-{:020}.json",
        timestamp,
        process::id(),
        counter
    ));
    let tmp_path = path.with_extension("tmp");
    let mut event = event.clone();
    event.data = sanitize_capture_data(event.data).map_err(io::Error::other)?;
    let record = SpoolRecord {
        spooled_at: Utc::now(),
        identity,
        event,
    };

    {
        let mut file = File::create(&tmp_path)?;
        serde_json::to_writer(&mut file, &record)
            .map_err(|err| io::Error::other(err.to_string()))?;
        writeln!(file)?;
        file.sync_all()?;
    }
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn pending_spool_files(spool_path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !spool_path.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(spool_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

fn update_spool_count(spool_path: &Path, status: &Arc<Mutex<CaptureStatus>>) {
    match pending_spool_files(spool_path) {
        Ok(files) => update_status(status, |status| {
            let count = files.len() as u64;
            status.queued_events = count;
            status.spooled_events = count;
        }),
        Err(err) => update_status(status, |status| {
            status.failed_events += 1;
            status.last_error = Some(format!("Unable to inspect capture spool: {err}"));
        }),
    }
}

fn build_next_batch(
    spool_path: &Path,
    cfg: &CaptureServiceConfig,
    status: &Arc<Mutex<CaptureStatus>>,
) -> NextBatch {
    let files = match pending_spool_files(spool_path) {
        Ok(files) => files,
        Err(err) => {
            update_status(status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Unable to read capture spool: {err}"));
            });
            return NextBatch::Empty;
        }
    };
    if files.is_empty() {
        return NextBatch::Empty;
    }

    let mut selected_files = Vec::new();
    let mut events = Vec::new();
    let mut body = Vec::new();
    let mut skipped_identity_mismatch = false;

    for file in files {
        if selected_files.len() >= MAX_CAPTURE_BATCH_EVENTS {
            break;
        }

        let record = match read_spool_record(&file) {
            Ok(record) => record,
            Err(err) => {
                quarantine_files(
                    spool_path,
                    std::slice::from_ref(&file),
                    &format!("Invalid local spool record: {err}"),
                    status,
                );
                continue;
            }
        };

        if !spool_record_matches_current_identity(&record, cfg) {
            skipped_identity_mismatch = true;
            continue;
        }

        if let Some(participant_id) = cfg.participant_id.as_deref()
            && record.event.user_id != participant_id
        {
            quarantine_files(
                spool_path,
                std::slice::from_ref(&file),
                "Capture event user_id does not match current capture token",
                status,
            );
            continue;
        }

        let event = match service_event_from_capture_event(record.event) {
            Ok(event) => event,
            Err(err) => {
                quarantine_files(
                    spool_path,
                    std::slice::from_ref(&file),
                    &format!("Invalid capture event for service upload: {err}"),
                    status,
                );
                continue;
            }
        };

        let mut candidate_events = events.clone();
        candidate_events.push(event);
        let candidate_batch = CaptureBatchRequest {
            schema_version: DEFAULT_CAPTURE_SCHEMA_VERSION,
            participant_id: cfg.participant_id.clone(),
            instance_id: cfg.instance_id.clone(),
            client_sent_at: Utc::now().to_rfc3339(),
            events: candidate_events,
        };

        let candidate_body = match serde_json::to_vec(&candidate_batch) {
            Ok(body) => body,
            Err(err) => {
                quarantine_files(
                    spool_path,
                    std::slice::from_ref(&file),
                    &format!("Unable to serialize capture batch: {err}"),
                    status,
                );
                continue;
            }
        };

        if candidate_body.len() > MAX_CAPTURE_BATCH_BYTES {
            if selected_files.is_empty() {
                quarantine_files(
                    spool_path,
                    std::slice::from_ref(&file),
                    "Single capture event exceeds service payload size limit",
                    status,
                );
                continue;
            }
            break;
        }

        events = candidate_batch.events;
        body = candidate_body;
        selected_files.push(file);
    }

    if selected_files.is_empty() {
        if skipped_identity_mismatch {
            NextBatch::NoMatchingIdentity
        } else {
            NextBatch::Empty
        }
    } else {
        NextBatch::Batch(SpoolBatch {
            event_count: events.len() as u64,
            files: selected_files,
            body,
        })
    }
}

fn spool_record_matches_current_identity(record: &SpoolRecord, cfg: &CaptureServiceConfig) -> bool {
    if let Some(identity) = &record.identity {
        return identity.token_hash == cfg.token_hash()
            && identity.service_base_url == cfg.base_url;
    }

    cfg.participant_id
        .as_deref()
        .is_some_and(|participant_id| record.event.user_id == participant_id)
}

fn read_spool_record(path: &Path) -> Result<SpoolRecord, String> {
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn service_event_from_capture_event(
    mut event: CaptureEvent,
) -> Result<CaptureServiceEvent, String> {
    let event_id = event
        .event_id
        .take()
        .filter(|event_id| !event_id.trim().is_empty())
        .ok_or_else(|| "event_id is required".to_string())?;
    if event_id.len() > 128 {
        return Err("event_id exceeds 128 characters".to_string());
    }
    let session_id = event
        .session_id
        .take()
        .filter(|session_id| !session_id.trim().is_empty())
        .ok_or_else(|| "session_id is required".to_string())?;
    if session_id.len() > 128 {
        return Err("session_id exceeds 128 characters".to_string());
    }

    let data = sanitize_capture_data(event.data)?;
    Ok(CaptureServiceEvent {
        event_id,
        sequence_number: event.sequence_number,
        schema_version: event.schema_version,
        user_id: event.user_id,
        session_id,
        event_source: event.event_source,
        language_id: event.language_id,
        file_hash: event.file_hash,
        event_type: event.event_type.as_str().to_string(),
        client_tz_offset_min: event.client_tz_offset_min,
        client_event_time: event.timestamp.to_rfc3339(),
        data,
    })
}

fn sanitize_capture_data(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let serde_json::Value::Object(map) = value else {
        return Err("capture event data must be a JSON object".to_string());
    };
    Ok(serde_json::Value::Object(sanitize_capture_object(map)))
}

fn sanitize_capture_object(
    map: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    const FORBIDDEN_KEYS: &[&str] = &["file_path", "path", "absolute_path", "workspace_path"];
    map.into_iter()
        .filter_map(|(key, value)| {
            if FORBIDDEN_KEYS.contains(&key.as_str()) {
                return None;
            }
            Some((key, sanitize_capture_value(value)))
        })
        .collect()
}

fn sanitize_capture_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(sanitize_capture_object(map)),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(sanitize_capture_value).collect())
        }
        other => other,
    }
}

fn post_capture_batch(
    events_url: &str,
    token: &str,
    body: &[u8],
) -> Result<CaptureBatchAcceptedResponse, CaptureHttpError> {
    post_capture_batch_with_timeout(events_url, token, body, CAPTURE_SERVICE_HTTP_TIMEOUT_SECS)
}

fn post_capture_batch_with_timeout(
    events_url: &str,
    token: &str,
    body: &[u8],
    timeout_secs: u64,
) -> Result<CaptureBatchAcceptedResponse, CaptureHttpError> {
    let response = minreq::post(events_url)
        .with_header("Authorization", format!("Bearer {token}"))
        .with_header("Content-Type", "application/json")
        .with_body(body.to_vec())
        .with_timeout(timeout_secs)
        .send()
        .map_err(|err| CaptureHttpError::transport(err.to_string()))?;

    if response.status_code != 202 {
        return Err(CaptureHttpError::response(
            i32::from(response.status_code),
            response
                .as_str()
                .unwrap_or(response.reason_phrase.as_str())
                .to_string(),
        ));
    }
    serde_json::from_slice(response.as_bytes()).map_err(|err| {
        CaptureHttpError::response(202, format!("Invalid capture accepted response: {err}"))
    })
}

fn request_capture_status(
    cfg: &CaptureServiceConfig,
) -> Result<CaptureServiceStatusResponse, CaptureHttpError> {
    request_capture_status_with_timeout(cfg, CAPTURE_SERVICE_HTTP_TIMEOUT_SECS)
}

fn request_capture_status_with_timeout(
    cfg: &CaptureServiceConfig,
    timeout_secs: u64,
) -> Result<CaptureServiceStatusResponse, CaptureHttpError> {
    let token = cfg
        .token()
        .ok_or_else(|| CaptureHttpError::response(401, "Capture token is not configured"))?;
    let status_url = cfg
        .status_url()
        .ok_or_else(|| CaptureHttpError::transport("Capture service URL is not configured"))?;
    let response = minreq::get(status_url)
        .with_header("Authorization", format!("Bearer {token}"))
        .with_timeout(timeout_secs)
        .send()
        .map_err(|err| CaptureHttpError::transport(err.to_string()))?;

    if response.status_code != 200 {
        return Err(CaptureHttpError::response(
            i32::from(response.status_code),
            response
                .as_str()
                .unwrap_or(response.reason_phrase.as_str())
                .to_string(),
        ));
    }
    serde_json::from_slice(response.as_bytes()).map_err(|err| {
        CaptureHttpError::response(200, format!("Invalid capture status response: {err}"))
    })
}

fn run_if_capture_config_snapshot_is_current(
    config: &Arc<Mutex<CaptureServiceConfig>>,
    snapshot: &CaptureServiceConfig,
    action: impl FnOnce(),
) -> bool {
    let Ok(current) = config.lock() else {
        return false;
    };
    if !current.matches_request_snapshot(snapshot) {
        return false;
    }
    action();
    true
}

fn apply_capture_service_status_if_current(
    config: &Arc<Mutex<CaptureServiceConfig>>,
    status: &Arc<Mutex<CaptureStatus>>,
    snapshot: &CaptureServiceConfig,
    response: CaptureServiceStatusResponse,
) -> bool {
    if let Ok(mut config) = config.lock() {
        if !config.matches_request_snapshot(snapshot) {
            return false;
        }
        config.participant_id = Some(response.participant_id.clone());
        config.instance_id = Some(response.instance_id.clone());
        update_status(status, |status| {
            status.token_status = if response.capture_enabled {
                CaptureTokenStatus::Accepted
            } else {
                CaptureTokenStatus::CaptureDisabled
            };
            status.state = if response.capture_enabled {
                CaptureState::Remote
            } else {
                CaptureState::CaptureDisabled
            };
            status.participant_id = Some(response.participant_id);
            status.instance_id = Some(response.instance_id);
            status.capture_enabled = Some(response.capture_enabled);
            status.participant_status = Some(response.participant_status);
            status.consent_status = Some(response.consent_status);
            status.instance_status = Some(response.instance_status);
            status.token_expires_at = response.token_expires_at;
            status.service_version = Some(response.service_version);
            status.last_status_check_at = Some(Utc::now().to_rfc3339());
            status.last_error = if response.capture_enabled {
                None
            } else {
                Some("Capture is disabled by the portal/service".to_string())
            };
        });
    } else {
        return false;
    }
    true
}

fn apply_http_error(status: &Arc<Mutex<CaptureStatus>>, err: &CaptureHttpError) {
    update_status(status, |status| {
        status.failed_events += 1;
        match err.status_code {
            Some(401) => {
                status.state = CaptureState::AuthFailed;
                status.token_status = CaptureTokenStatus::Rejected;
            }
            Some(403) => {
                status.state = CaptureState::CaptureDisabled;
                status.token_status = CaptureTokenStatus::CaptureDisabled;
            }
            _ if err.is_transient() => {
                status.state = CaptureState::ServiceUnavailable;
            }
            _ => {}
        }
        status.last_error = Some(err.message.clone());
    });
}

fn apply_http_error_if_current(
    config: &Arc<Mutex<CaptureServiceConfig>>,
    status: &Arc<Mutex<CaptureStatus>>,
    snapshot: &CaptureServiceConfig,
    err: &CaptureHttpError,
) -> bool {
    run_if_capture_config_snapshot_is_current(config, snapshot, || {
        apply_http_error(status, err);
    })
}

fn quarantine_path(spool_path: &Path) -> PathBuf {
    spool_path.join("quarantine")
}

fn quarantine_files(
    spool_path: &Path,
    files: &[PathBuf],
    reason: &str,
    status: &Arc<Mutex<CaptureStatus>>,
) {
    let quarantine = quarantine_path(spool_path);
    if let Err(err) = fs::create_dir_all(&quarantine) {
        update_status(status, |status| {
            status.failed_events += 1;
            status.last_error = Some(format!("Unable to create quarantine directory: {err}"));
        });
        return;
    }

    for file in files {
        let file_name = file.file_name().map_or_else(
            || "capture-event.json".into(),
            std::borrow::ToOwned::to_owned,
        );
        let target = quarantine.join(file_name);
        if let Err(err) = fs::rename(file, &target).or_else(|_| {
            fs::copy(file, &target)?;
            fs::remove_file(file)
        }) {
            update_status(status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Unable to quarantine {}: {err}", file.display()));
            });
            continue;
        }
        let reason_path = target.with_extension("reason.txt");
        if let Err(err) = fs::write(&reason_path, reason) {
            warn!(
                "Capture: unable to write quarantine reason \"{}\": {err}",
                reason_path.display()
            );
        }
        update_status(status, |status| {
            status.quarantined_events += 1;
            status.last_error = Some(reason.to_string());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    fn temp_spool_path(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "codechat-capture-{test_name}-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn capture_test_event(user_id: &str, event_id: &str) -> CaptureEvent {
        CaptureEvent::with_columns(
            Some(event_id.to_string()),
            Some(1),
            Some(2),
            user_id.to_string(),
            Some("session-1".to_string()),
            Some("vscode_extension".to_string()),
            Some("rust".to_string()),
            Some("file-hash".to_string()),
            CaptureEventType::Save,
            Utc::now(),
            Some(360),
            json!({ "reason": "unit_test" }),
        )
    }

    fn capture_service_config(token: &str, generation: u64) -> CaptureServiceConfig {
        let mut cfg = CaptureServiceConfig::configured(
            "https://capture.example/dev",
            Some(token.to_string()),
        )
        .expect("capture service config should parse");
        cfg.generation = generation;
        cfg
    }

    fn capture_service_config_with_base_url(
        base_url: &str,
        token: &str,
        generation: u64,
        participant_id: &str,
    ) -> CaptureServiceConfig {
        let mut cfg = CaptureServiceConfig::configured(base_url, Some(token.to_string()))
            .expect("capture service config should parse");
        cfg.generation = generation;
        cfg.participant_id = Some(participant_id.to_string());
        cfg.instance_id = Some(format!("{participant_id}-instance"));
        cfg
    }

    fn capture_service_status_response(participant_id: &str) -> CaptureServiceStatusResponse {
        CaptureServiceStatusResponse {
            participant_id: participant_id.to_string(),
            instance_id: format!("{participant_id}-instance"),
            study_id: "study-2026".to_string(),
            capture_enabled: true,
            participant_status: "active".to_string(),
            consent_status: "consented".to_string(),
            instance_status: "active".to_string(),
            token_expires_at: None,
            server_time: "2026-07-12T16:10:04Z".to_string(),
            service_version: "0.1.0".to_string(),
        }
    }

    fn start_delayed_capture_events_server(
        status_code: u16,
        body: &'static str,
    ) -> (
        String,
        std::sync::mpsc::Receiver<()>,
        std::sync::mpsc::Sender<()>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (request_seen_tx, request_seen_rx) = std::sync::mpsc::channel();
        let (respond_tx, respond_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _addr) = listener.accept().expect("request should arrive");
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("request should read");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            request_seen_tx
                .send(())
                .expect("request notification should send");
            respond_rx.recv().expect("response release should arrive");
            let reason = match status_code {
                202 => "Accepted",
                400 => "Bad Request",
                413 => "Payload Too Large",
                _ => "Response",
            };
            write!(
                stream,
                "HTTP/1.1 {status_code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("response should write");
        });
        (base_url, request_seen_rx, respond_tx, handle)
    }

    fn mark_config_replaced_during_upload(
        config: &Arc<Mutex<CaptureServiceConfig>>,
        status: &Arc<Mutex<CaptureStatus>>,
        base_url: &str,
    ) {
        *config.lock().expect("config lock should not be poisoned") =
            capture_service_config_with_base_url(base_url, "new-token", 2, "new-user");
        update_status(status, |status| {
            status.state = CaptureState::Spooling;
            status.token_status = CaptureTokenStatus::Unverified;
            status.participant_id = None;
            status.instance_id = None;
            status.uploaded_events = 0;
            status.quarantined_events = 0;
            status.last_upload_at = None;
            status.last_error = None;
        });
    }

    #[test]
    fn capture_event_type_uses_stable_serialized_strings() {
        assert_eq!(
            serde_json::to_value(CaptureEventType::WriteDoc).unwrap(),
            json!("write_doc")
        );
        assert_eq!(
            serde_json::from_value::<CaptureEventType>(json!("compile_end")).unwrap(),
            CaptureEventType::CompileEnd
        );
        assert_eq!(
            serde_json::to_value(CaptureEventType::CaptureSettingsChanged).unwrap(),
            json!("capture_settings_changed")
        );
        assert!(serde_json::from_value::<CaptureEventType>(json!("random")).is_err());
    }

    #[test]
    fn capture_event_new_sets_all_fields() {
        let ts = Utc::now();

        let ev = CaptureEvent::new(
            "user123".to_string(),
            Some("hashed-path".to_string()),
            CaptureEventType::WriteDoc,
            ts,
            json!({ "chars_typed": 42 }),
        );

        assert_eq!(ev.user_id, "user123");
        assert_eq!(ev.file_hash.as_deref(), Some("hashed-path"));
        assert_eq!(ev.event_type, CaptureEventType::WriteDoc);
        assert_eq!(ev.timestamp, ts);
        assert!(ev.event_id.is_none());
        assert_eq!(ev.data, json!({ "chars_typed": 42 }));
    }

    #[test]
    fn capture_event_now_uses_current_time_and_fields() {
        let before = Utc::now();
        let ev = CaptureEvent::now(
            "user123".to_string(),
            None,
            CaptureEventType::Save,
            json!({ "reason": "manual" }),
        );
        let after = Utc::now();

        assert_eq!(ev.user_id, "user123");
        assert!(ev.file_hash.is_none());
        assert_eq!(ev.event_type, CaptureEventType::Save);
        assert_eq!(ev.data, json!({ "reason": "manual" }));
        assert!(ev.timestamp >= before);
        assert!(ev.timestamp <= after);
    }

    #[test]
    fn capture_spool_writes_fifo_json_records() {
        let spool_path = temp_spool_path("spool-test");
        let _ = fs::remove_dir_all(&spool_path);

        let capture = EventCapture::new(spool_path.clone()).expect("capture worker should start");
        capture.log(&CaptureEvent::with_columns(
            Some("event-1".to_string()),
            Some(1),
            Some(2),
            "participant".to_string(),
            Some("session".to_string()),
            Some("test".to_string()),
            Some("rust".to_string()),
            Some("file-hash".to_string()),
            CaptureEventType::Save,
            Utc::now(),
            Some(360),
            json!({ "reason": "unit_test" }),
        ));

        let mut text = String::new();
        for _ in 0..20 {
            if let Ok(files) = pending_spool_files(&spool_path)
                && let Some(path) = files.first()
                && let Ok(contents) = fs::read_to_string(path)
            {
                text = contents;
                if text.contains("\"event_id\":\"event-1\"") {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(text.contains("\"event_type\":\"save\""));
        assert!(text.contains("\"spooled_at\""));
        assert_eq!(capture.status().state, CaptureState::Spooling);
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn capture_spool_sanitizes_forbidden_path_keys_before_disk() {
        let spool_path = temp_spool_path("spool-sanitize-test");
        let _ = fs::remove_dir_all(&spool_path);
        let mut event = capture_test_event("participant", "event-sanitized");
        event.data = json!({
            "file_path": "C:/secret.rs",
            "nested": { "path": "/secret" },
            "items": [
                { "absolute_path": "/secret2", "ok": true },
                [[{ "workspace_path": "/secret3", "nested_ok": true }]]
            ]
        });

        append_spool_event(&spool_path, &event, None).expect("event should spool");
        let files = pending_spool_files(&spool_path).expect("spool should list");
        let record = read_spool_record(&files[0]).expect("spool record should parse");

        assert!(record.event.data.get("file_path").is_none());
        assert!(record.event.data.pointer("/nested/path").is_none());
        assert!(
            record
                .event
                .data
                .pointer("/items/0/absolute_path")
                .is_none()
        );
        assert_eq!(record.event.data.pointer("/items/0/ok"), Some(&json!(true)));
        assert!(
            record
                .event
                .data
                .pointer("/items/1/0/0/workspace_path")
                .is_none()
        );
        assert_eq!(
            record.event.data.pointer("/items/1/0/0/nested_ok"),
            Some(&json!(true))
        );
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn capture_event_with_columns_sets_analysis_columns() {
        let ts = Utc::now();

        let ev = CaptureEvent::with_columns(
            Some("abc-123".to_string()),
            Some(42),
            Some(2),
            "user123".to_string(),
            Some("session-1".to_string()),
            Some("vscode_extension".to_string()),
            Some("rust".to_string()),
            Some("hash".to_string()),
            CaptureEventType::WriteCode,
            ts,
            Some(-360),
            json!({ "chars_typed": 42 }),
        );

        assert_eq!(ev.event_id.as_deref(), Some("abc-123"));
        assert_eq!(ev.sequence_number, Some(42));
        assert_eq!(ev.schema_version, Some(2));
        assert_eq!(ev.session_id.as_deref(), Some("session-1"));
        assert_eq!(ev.event_source.as_deref(), Some("vscode_extension"));
        assert_eq!(ev.language_id.as_deref(), Some("rust"));
        assert_eq!(ev.file_hash.as_deref(), Some("hash"));
        assert_eq!(ev.client_tz_offset_min, Some(-360));
        assert_eq!(ev.data, json!({ "chars_typed": 42 }));
    }

    #[test]
    fn capture_service_payload_sanitizes_forbidden_path_keys() {
        let ev = CaptureEvent::with_columns(
            Some("event-1".to_string()),
            Some(1),
            Some(2),
            "user123".to_string(),
            Some("session-1".to_string()),
            Some("vscode_extension".to_string()),
            Some("rust".to_string()),
            Some(hash_capture_path("src/lib.rs")),
            CaptureEventType::Save,
            Utc::now(),
            Some(360),
            json!({
                "reason": "manual",
                "file_path": "C:/secret.rs",
                "nested": { "path": "/secret" },
                "items": [
                    { "absolute_path": "/secret2", "ok": true },
                    [[{ "workspace_path": "/secret3", "nested_ok": true }]]
                ]
            }),
        );
        let service_event = service_event_from_capture_event(ev).expect("event should convert");

        assert_eq!(service_event.event_type, "save");
        assert_eq!(service_event.session_id, "session-1");
        assert!(service_event.data.get("file_path").is_none());
        assert!(service_event.data.pointer("/nested/path").is_none());
        assert!(
            service_event
                .data
                .pointer("/items/0/absolute_path")
                .is_none()
        );
        assert_eq!(
            service_event.data.pointer("/items/0/ok"),
            Some(&json!(true))
        );
        assert!(
            service_event
                .data
                .pointer("/items/1/0/0/workspace_path")
                .is_none()
        );
        assert_eq!(
            service_event.data.pointer("/items/1/0/0/nested_ok"),
            Some(&json!(true))
        );
    }

    #[test]
    fn stale_capture_status_response_does_not_overwrite_current_token_identity() {
        let old_snapshot = capture_service_config("old-token", 1);
        let new_config = capture_service_config("new-token", 2);
        let config = Arc::new(Mutex::new(new_config.clone()));
        let status = Arc::new(Mutex::new(CaptureStatus::starting(temp_spool_path(
            "stale-status-test",
        ))));

        assert!(!apply_capture_service_status_if_current(
            &config,
            &status,
            &old_snapshot,
            capture_service_status_response("old-user"),
        ));

        let current = config.lock().expect("config lock should not be poisoned");
        assert_eq!(current.token_hash(), new_config.token_hash());
        assert_eq!(current.participant_id, None);
        assert_eq!(current.instance_id, None);
        drop(current);

        let status = status.lock().expect("status lock should not be poisoned");
        assert_eq!(status.participant_id, None);
        assert_ne!(status.token_status, CaptureTokenStatus::Accepted);
    }

    #[test]
    fn stale_capture_http_error_does_not_mark_current_token_rejected() {
        let old_snapshot = capture_service_config("old-token", 1);
        let new_config = capture_service_config("new-token", 2);
        let config = Arc::new(Mutex::new(new_config));
        let status = Arc::new(Mutex::new(CaptureStatus::starting(temp_spool_path(
            "stale-error-test",
        ))));
        let err = CaptureHttpError::response(401, "old token rejected");

        assert!(!apply_http_error_if_current(
            &config,
            &status,
            &old_snapshot,
            &err,
        ));

        let status = status.lock().expect("status lock should not be poisoned");
        assert_ne!(status.state, CaptureState::AuthFailed);
        assert_ne!(status.token_status, CaptureTokenStatus::Rejected);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn stale_capture_post_success_does_not_delete_spooled_events() {
        let spool_path = temp_spool_path("stale-post-success-test");
        let _ = fs::remove_dir_all(&spool_path);
        let (base_url, request_seen_rx, respond_tx, server_handle) =
            start_delayed_capture_events_server(
                202,
                r#"{"batch_id":"batch-1","accepted":1,"server_time":"2026-07-12T16:10:05Z"}"#,
            );
        let old_cfg = capture_service_config_with_base_url(&base_url, "old-token", 1, "old-user");
        append_spool_event(
            &spool_path,
            &capture_test_event("old-user", "old-post-success-event"),
            old_cfg.spool_identity(),
        )
        .expect("old event should spool");
        let config = Arc::new(Mutex::new(old_cfg));
        let status = Arc::new(Mutex::new(CaptureStatus::starting(spool_path.clone())));
        update_status(&status, |status| {
            status.state = CaptureState::Spooling;
        });
        let upload_config = config.clone();
        let upload_status = status.clone();
        let upload_spool_path = spool_path.clone();
        let upload_handle = thread::spawn(move || {
            upload_next_batch(&upload_spool_path, &upload_config, &upload_status)
        });

        request_seen_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upload request should start");
        mark_config_replaced_during_upload(&config, &status, &base_url);
        respond_tx.send(()).expect("response should release");

        assert_eq!(
            upload_handle.join().expect("upload thread should finish"),
            UploadOutcome::Paused,
        );
        server_handle.join().expect("server thread should finish");
        assert_eq!(
            pending_spool_files(&spool_path)
                .expect("spool should list")
                .len(),
            1
        );
        let status = status.lock().expect("status lock should not be poisoned");
        assert_eq!(status.state, CaptureState::Spooling);
        assert_eq!(status.uploaded_events, 0);
        assert_eq!(status.last_upload_at, None);
        drop(status);
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn stale_capture_post_validation_failure_does_not_quarantine_spooled_events() {
        let spool_path = temp_spool_path("stale-post-validation-test");
        let _ = fs::remove_dir_all(&spool_path);
        let (base_url, request_seen_rx, respond_tx, server_handle) =
            start_delayed_capture_events_server(400, r#"{"error":{"message":"invalid"}}"#);
        let old_cfg = capture_service_config_with_base_url(&base_url, "old-token", 1, "old-user");
        append_spool_event(
            &spool_path,
            &capture_test_event("old-user", "old-post-validation-event"),
            old_cfg.spool_identity(),
        )
        .expect("old event should spool");
        let config = Arc::new(Mutex::new(old_cfg));
        let status = Arc::new(Mutex::new(CaptureStatus::starting(spool_path.clone())));
        update_status(&status, |status| {
            status.state = CaptureState::Spooling;
        });
        let upload_config = config.clone();
        let upload_status = status.clone();
        let upload_spool_path = spool_path.clone();
        let upload_handle = thread::spawn(move || {
            upload_next_batch(&upload_spool_path, &upload_config, &upload_status)
        });

        request_seen_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("upload request should start");
        mark_config_replaced_during_upload(&config, &status, &base_url);
        respond_tx.send(()).expect("response should release");

        assert_eq!(
            upload_handle.join().expect("upload thread should finish"),
            UploadOutcome::Paused,
        );
        server_handle.join().expect("server thread should finish");
        assert_eq!(
            pending_spool_files(&spool_path)
                .expect("spool should list")
                .len(),
            1
        );
        assert_eq!(
            pending_spool_files(&quarantine_path(&spool_path))
                .expect("quarantine should list")
                .len(),
            0
        );
        let status = status.lock().expect("status lock should not be poisoned");
        assert_eq!(status.state, CaptureState::Spooling);
        assert_eq!(status.quarantined_events, 0);
        assert_eq!(status.last_error, None);
        drop(status);
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn capture_batch_skips_spool_records_for_other_tokens() {
        let spool_path = temp_spool_path("spool-token-test");
        let _ = fs::remove_dir_all(&spool_path);

        let mut old_cfg = CaptureServiceConfig::configured(
            "https://capture.example/dev",
            Some("old-token".to_string()),
        )
        .expect("old config should parse");
        old_cfg.participant_id = Some("old-user".to_string());
        old_cfg.instance_id = Some("old-instance".to_string());
        let mut new_cfg = CaptureServiceConfig::configured(
            "https://capture.example/dev",
            Some("new-token".to_string()),
        )
        .expect("new config should parse");
        new_cfg.participant_id = Some("new-user".to_string());
        new_cfg.instance_id = Some("new-instance".to_string());

        append_spool_event(
            &spool_path,
            &capture_test_event("old-user", "old-event"),
            old_cfg.spool_identity(),
        )
        .expect("old event should spool");
        append_spool_event(
            &spool_path,
            &capture_test_event("new-user", "new-event"),
            new_cfg.spool_identity(),
        )
        .expect("new event should spool");

        let status = Arc::new(Mutex::new(CaptureStatus::starting(spool_path.clone())));
        let batch = match build_next_batch(&spool_path, &new_cfg, &status) {
            NextBatch::Batch(batch) => batch,
            other => panic!("expected matching batch, got {other:?}"),
        };
        let body: serde_json::Value =
            serde_json::from_slice(&batch.body).expect("batch body should parse");

        assert_eq!(batch.files.len(), 1);
        assert_eq!(
            body.pointer("/events/0/event_id"),
            Some(&json!("new-event"))
        );
        assert_eq!(body.pointer("/events/0/user_id"), Some(&json!("new-user")));
        assert_eq!(body.pointer("/events/1"), None);
        assert_eq!(
            pending_spool_files(&spool_path)
                .expect("spool should list")
                .len(),
            2
        );
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn capture_batch_reports_when_only_other_token_records_are_pending() {
        let spool_path = temp_spool_path("spool-token-mismatch-test");
        let _ = fs::remove_dir_all(&spool_path);

        let old_cfg = CaptureServiceConfig::configured(
            "https://capture.example/dev",
            Some("old-token".to_string()),
        )
        .expect("old config should parse");
        let mut new_cfg = CaptureServiceConfig::configured(
            "https://capture.example/dev",
            Some("new-token".to_string()),
        )
        .expect("new config should parse");
        new_cfg.participant_id = Some("new-user".to_string());

        append_spool_event(
            &spool_path,
            &capture_test_event("old-user", "old-event"),
            old_cfg.spool_identity(),
        )
        .expect("old event should spool");

        let status = Arc::new(Mutex::new(CaptureStatus::starting(spool_path.clone())));
        assert!(matches!(
            build_next_batch(&spool_path, &new_cfg, &status),
            NextBatch::NoMatchingIdentity
        ));
        assert_eq!(
            pending_spool_files(&spool_path)
                .expect("spool should list")
                .len(),
            1
        );
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn capture_batch_does_not_upload_legacy_records_before_identity_known() {
        let spool_path = temp_spool_path("spool-legacy-unknown-identity-test");
        let _ = fs::remove_dir_all(&spool_path);

        let cfg = CaptureServiceConfig::configured(
            "https://capture.example/dev",
            Some("token".to_string()),
        )
        .expect("config should parse");

        append_spool_event(
            &spool_path,
            &capture_test_event("participant", "legacy-event"),
            None,
        )
        .expect("legacy event should spool");

        let status = Arc::new(Mutex::new(CaptureStatus::starting(spool_path.clone())));
        assert!(matches!(
            build_next_batch(&spool_path, &cfg, &status),
            NextBatch::NoMatchingIdentity
        ));
        assert_eq!(
            pending_spool_files(&spool_path)
                .expect("spool should list")
                .len(),
            1
        );
        let _ = fs::remove_dir_all(&spool_path);
    }

    #[test]
    fn capture_http_upload_uses_request_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("listener should have address");
        thread::spawn(move || {
            if let Ok((_stream, _addr)) = listener.accept() {
                thread::sleep(Duration::from_secs(3));
            }
        });

        let started = Instant::now();
        let err = post_capture_batch_with_timeout(
            &format!("http://{addr}/v1/capture/events"),
            "token",
            br#"{"events":[]}"#,
            1,
        )
        .expect_err("silent local server should time out");

        assert!(err.status_code.is_none());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn service_url_normalization_accepts_dev_base_and_routes() {
        assert_eq!(
            normalize_service_base_url(
                "https://9m2nbv2rvc.execute-api.us-east-2.amazonaws.com/dev/v1/capture/events"
            )
            .unwrap(),
            "https://9m2nbv2rvc.execute-api.us-east-2.amazonaws.com/dev"
        );
        assert_eq!(
            normalize_service_base_url("http://localhost:8787/v1/capture/status").unwrap(),
            "http://localhost:8787"
        );
        assert_eq!(
            normalize_service_base_url("http://127.0.0.1:8787/dev/").unwrap(),
            "http://127.0.0.1:8787/dev"
        );
        assert!(normalize_service_base_url("postgres://example").is_err());
        assert!(normalize_service_base_url("http://capture.example/dev").is_err());
        assert!(normalize_service_base_url("http://localhost.evil/dev").is_err());
        assert!(normalize_service_base_url("https://user:pass@example.com/dev").is_err());
    }
}
