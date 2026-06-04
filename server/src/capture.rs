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
// This module provides an asynchronous event capture facility backed by a
// PostgreSQL database. It is designed to support the dissertation study by
// recording process-level data such as:
//
// * Frequency and timing of writing entries
// * Edits to documentation and code
// * Switches between documentation and coding activity
// * Duration of engagement with reflective writing
// * Save, compile, and run events
//
// Events are sent from the client (browser and/or VS Code extension) to the
// server as JSON. The server enqueues events into an asynchronous worker which
// performs batched inserts into the `events` table.
//
// Database schema
// ----------------------------------------------------------------------------
//
// The canonical schema and migration DDL lives in
// `server/scripts/capture_events_schema.sql`. The important analysis columns
// are:
//
// ```sql
// event_id, sequence_number, schema_version,
// user_id, session_id, event_source, language_id, file_hash, event_type,
// timestamp, client_tz_offset_min, data
// ```
//
// * `user_id` – pseudonymous participant UUID. Course, group, assignment, and
//   study condition are intentionally joined later from researcher-managed
//   participant/date mappings instead of being configured by students.
// * `event_id` – opaque stable per-event ID for correlation and future
//   deduplication across capture transports or retries.
// * `sequence_number` – ordered event counter scoped by `session_id` and
//   `event_source` for reconstructing event order and detecting gaps.
// * `session_id`, `schema_version` – session grouping and payload versioning
//   metadata.
// * `file_hash` – privacy-preserving SHA-256 hash of the local file path.
// * `event_type` – coarse event type (see `CaptureEventType` below).
// * `timestamp` – server receive/record timestamp (in UTC).
// * `client_tz_offset_min` – browser/VS Code timezone offset used to derive
//   local time-of-day without storing location or full timezone identity.
// * `data` – JSONB payload with event-specific details.

// Imports
// -------
//
// ### Standard library
use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
    thread,
};

// ### Third-party
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_postgres::{Client, NoTls};
use ts_rs::TS;

static NEXT_CAPTURE_EVENT_ID: AtomicU64 = AtomicU64::new(1);

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
pub fn hash_capture_path(path: &str) -> String {
    Sha256::digest(path.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// JSON payload received from local clients for capture events.
///
/// The server supplies the authoritative timestamp and hashes any raw local file
/// path before storage. Study metadata such as course, assignment, group,
/// condition, and task is not part of this wire type: those values are inferred
/// later from researcher-managed mappings keyed by the pseudonymous `user_id`
/// and event timestamps.
#[derive(Debug, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, optional_fields)]
pub struct CaptureEventWire {
    /// Client-generated unique event identifier. Unlike `sequence_number`, this
    /// is an opaque stable ID for correlation and possible future deduplication
    /// across capture transports or retries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    /// Event order within one `(session_id, event_source)` stream. Unlike
    /// `event_id`, this is intentionally ordered so analysis can reconstruct
    /// event order and detect gaps within a stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_number: Option<i64>,
    /// Capture payload schema version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<i32>,
    /// Pseudonymous participant UUID. This is not the student's real identity.
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
    /// long enough for the Rust server to normalize hashing; it is never written
    /// to the capture database or fallback JSONL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// SHA-256 hash of the local file path. Clients should prefer `file_path`
    /// so the server owns hashing, but this remains for server-originated
    /// events and backward-compatible local callers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    /// Canonical capture event type.
    pub event_type: CaptureEventType,

    /// Optional client timezone offset in minutes (JS Date().getTimezoneOffset()).
    /// Combined with the server UTC timestamp, this allows local time-of-day
    /// analysis without storing the student's location or full timezone name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_tz_offset_min: Option<i32>,

    /// Event-specific data stored as JSON. Known keys include capture controls
    /// (`capture_active`, `capture_control_only`), activity/session details
    /// (`mode`, `closed_by`, `duration_ms`, `duration_seconds`, `from`, `to`),
    /// tool/run/build details (`reason`, `lineCount`, `sessionName`,
    /// `sessionType`, `taskName`, `taskSource`, `processId`, `exitCode`), write
    /// classification details (`source`, `classification_basis`, `diff`,
    /// `doc_block_diff`, `doc_block_count_before`,
    /// `doc_block_count_after`). Add future keys only when they support a
    /// specific analysis question and do not store source text or raw local
    /// paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "unknown")]
    pub data: Option<serde_json::Value>,
}

/// Participant and session metadata remembered from client capture events.
///
/// The translation layer generates `write_doc`/`write_code` events after it has
/// parsed CodeChat content. Those events should share the same pseudonymous
/// participant and capture session as extension-side events, but the server
/// should not ask students for course/group/assignment/task setup values.
#[derive(Clone, Debug, Default)]
pub(crate) struct CaptureContext {
    /// True only while capture is actively recording. The translation layer must
    /// not generate write events from a stale participant/session context after
    /// recording or consent is turned off.
    active: bool,
    /// Pseudonymous participant UUID from the latest client capture event.
    user_id: Option<String>,
    /// Origin of the client event stream, such as the VS Code extension.
    event_source: Option<String>,
    /// Extension session UUID carried on the capture wire payload.
    session_id: Option<String>,
    /// Client timezone offset in minutes, retained for generated write events.
    client_tz_offset_min: Option<i32>,
    /// Capture payload schema version from the extension.
    schema_version: Option<i32>,
    /// Server-local event order for translation-generated events in this
    /// capture context. Client events have their own extension-side sequence.
    server_sequence_number: i64,
}

impl CaptureContext {
    /// Refresh server-side capture identity and active/inactive state from an
    /// extension capture message. This context is used only for server-generated
    /// write classification events, not for deciding whether the original
    /// extension event itself is inserted.
    pub(crate) fn update_from_wire(&mut self, wire: &CaptureEventWire) {
        // Session start/end are the coarse lifecycle signals; the explicit
        // `capture_active` data field handles settings-change audit events that
        // should be inserted while also disabling later translated writes.
        match wire.event_type {
            CaptureEventType::SessionStart => self.active = true,
            CaptureEventType::SessionEnd => self.active = false,
            _ => {}
        }
        // Keep the most recent participant/session metadata so translated write
        // events can be joined to the same participant as extension events.
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
        if let Some(serde_json::Value::Object(data)) = &wire.data {
            // Settings-change audit events use this flag to tell the server
            // whether future translation-generated write events are allowed.
            if let Some(active) = data
                .get("capture_active")
                .and_then(serde_json::Value::as_bool)
            {
                self.active = active;
            }
        }
    }

    /// True when server-generated capture events should be logged for this
    /// participant/session context.
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn capture_event(
        &mut self,
        event_type: CaptureEventType,
        file_path: Option<String>,
        data: serde_json::Value,
    ) -> Option<CaptureEventWire> {
        // Do not generate server-side write_doc/write_code rows unless the
        // latest settings state says capture is actively recording.
        if !self.active {
            return None;
        }
        // Normalize arbitrary JSON payloads into objects so we can attach
        // server-translation metadata consistently.
        let mut data = match data {
            serde_json::Value::Object(map) => map,
            other => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), other);
                map
            }
        };
        // Preserve any existing source field, but default server-generated
        // events to `server_translation` for analysis.
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

/// True for a capture message that should update `CaptureContext` only. These
/// messages are used to stop server-side write classification after the user
/// turns off consent or recording, without adding a synthetic DB row.
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

/// Configuration used to construct the PostgreSQL connection string.
///
/// You can populate this from a JSON file or environment variables in
/// `main.rs`; this module stays agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    /// PostgreSQL host name or address.
    pub host: String,
    /// Optional PostgreSQL port. Uses libpq's default when omitted.
    #[serde(default)]
    pub port: Option<u16>,
    /// PostgreSQL user name.
    pub user: String,
    /// PostgreSQL password. Never included in redacted summaries.
    pub password: String,
    /// PostgreSQL database name.
    pub dbname: String,
    /// Optional: application-level identifier for this deployment (e.g., course
    /// code or semester). Not stored in the DB directly; callers can embed this
    /// in `data` if desired.
    #[serde(default)]
    pub app_id: Option<String>,
    /// Local JSONL file used when PostgreSQL is unavailable.
    #[serde(default)]
    pub fallback_path: Option<PathBuf>,
}

impl CaptureConfig {
    /// Validate capture configuration before starting the worker. This catches
    /// invalid setup early and avoids ambiguous "random port" behavior.
    pub fn validate(&self) -> Result<(), String> {
        if self.port == Some(0) {
            return Err("capture database port must be between 1 and 65535".to_string());
        }
        validate_conn_str_field("host", &self.host)?;
        validate_conn_str_field("user", &self.user)?;
        validate_conn_str_field("password", &self.password)?;
        validate_conn_str_field("dbname", &self.dbname)?;
        Ok(())
    }

    /// Build a libpq-style connection string.
    pub fn to_conn_str(&self) -> String {
        let mut parts = vec![
            format!("host={}", self.host),
            format!("user={}", self.user),
            format!("password={}", self.password),
            format!("dbname={}", self.dbname),
        ];
        if let Some(port) = self.port {
            parts.push(format!("port={port}"));
        }
        parts.join(" ")
    }

    /// Return a human-readable summary that never includes the password.
    pub fn redacted_summary(&self) -> String {
        format!(
            "host={}, port={:?}, user={}, dbname={}, app_id={:?}, fallback_path={:?}",
            self.host, self.port, self.user, self.dbname, self.app_id, self.fallback_path
        )
    }

    /// Build capture configuration from environment variables. If no capture
    /// host is configured, return `Ok(None)` so callers can fall back to a file.
    pub fn from_env() -> Result<Option<Self>, String> {
        let Some(host) = env_var_trimmed("CODECHAT_CAPTURE_HOST") else {
            return Ok(None);
        };

        let port = match env_var_trimmed("CODECHAT_CAPTURE_PORT") {
            Some(port) => Some(port.parse::<u16>().map_err(|err| {
                format!("CODECHAT_CAPTURE_PORT must be a valid port number: {err}")
            })?),
            None => None,
        };

        let cfg = Self {
            host,
            port,
            user: required_env_var("CODECHAT_CAPTURE_USER")?,
            password: required_env_var("CODECHAT_CAPTURE_PASSWORD")?,
            dbname: required_env_var("CODECHAT_CAPTURE_DBNAME")?,
            app_id: env_var_trimmed("CODECHAT_CAPTURE_APP_ID"),
            fallback_path: env_var_trimmed("CODECHAT_CAPTURE_FALLBACK_PATH").map(PathBuf::from),
        };
        cfg.validate()?;
        Ok(Some(cfg))
    }
}

fn validate_conn_str_field(field_name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("capture database {field_name} must not be empty"));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(format!(
            "capture database {field_name} must not contain whitespace"
        ));
    }
    Ok(())
}

/// Load capture configuration from environment variables or the repo/runtime
/// `capture_config.json`.
///
/// Environment variables take precedence so deployment can inject secrets
/// without writing them to disk. Local development and student-facing setup use
/// the single config file at `root_path/capture_config.json`.
pub fn load_capture_config(root_path: &Path) -> Option<CaptureConfig> {
    match CaptureConfig::from_env() {
        Ok(Some(cfg)) => return Some(with_default_capture_fallback_path(cfg, root_path)),
        Ok(None) => {}
        Err(err) => {
            warn!("Capture: invalid environment configuration: {err}");
            return None;
        }
    }

    let config_path = root_path.join("capture_config.json");

    match fs::read_to_string(&config_path) {
        Ok(json) => match serde_json::from_str::<CaptureConfig>(&json) {
            Ok(cfg) => match cfg.validate() {
                Ok(()) => Some(with_default_capture_fallback_path(cfg, root_path)),
                Err(err) => {
                    warn!("Capture: invalid configuration in {config_path:?}: {err}");
                    None
                }
            },
            Err(err) => {
                warn!("Capture: invalid JSON in {config_path:?}: {err}");
                None
            }
        },
        Err(err) => {
            info!(
                "Capture: disabled (no CODECHAT_CAPTURE_* env and no readable config at {config_path:?}: {err})"
            );
            None
        }
    }
}

/// Normalize the fallback JSONL path to the runtime root when a relative path
/// or no path is provided.
pub fn with_default_capture_fallback_path(
    mut cfg: CaptureConfig,
    root_path: &Path,
) -> CaptureConfig {
    match &cfg.fallback_path {
        Some(path) if path.is_relative() => {
            cfg.fallback_path = Some(root_path.join(path));
        }
        Some(_) => {}
        None => {
            cfg.fallback_path = Some(root_path.join("capture-events-fallback.jsonl"));
        }
    }
    cfg
}

fn env_var_trimmed(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_env_var(name: &str) -> Result<String, String> {
    env_var_trimmed(name).ok_or_else(|| format!("{name} is required when capture env is used"))
}

/// Known capture worker states reported to the VS Code status UI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureState {
    /// Capture is not configured or the worker is unavailable.
    Disabled,
    /// Capture worker is starting and attempting the first database connection.
    Starting,
    /// Events are being persisted to PostgreSQL.
    Database,
    /// Events are being written to local JSONL fallback storage.
    Fallback,
}

/// Capture worker health exposed to the VS Code status item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export)]
pub struct CaptureStatus {
    /// True when the capture worker is configured and accepting events.
    pub enabled: bool,
    /// Current worker state.
    pub state: CaptureState,
    /// Number of events accepted into the worker queue.
    pub queued_events: u64,
    /// Number of events inserted into PostgreSQL.
    pub persisted_events: u64,
    /// Number of events written to the local JSONL fallback.
    pub fallback_events: u64,
    /// Number of failed enqueue or fallback-write attempts.
    pub failed_events: u64,
    /// Most recent capture error, if one is known.
    pub last_error: Option<String>,
    /// Local JSONL fallback path when fallback capture is configured.
    pub fallback_path: Option<PathBuf>,
}

impl CaptureStatus {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            state: CaptureState::Disabled,
            queued_events: 0,
            persisted_events: 0,
            fallback_events: 0,
            failed_events: 0,
            last_error: None,
            fallback_path: None,
        }
    }

    fn starting(fallback_path: Option<PathBuf>) -> Self {
        Self {
            enabled: true,
            state: CaptureState::Starting,
            queued_events: 0,
            persisted_events: 0,
            fallback_events: 0,
            failed_events: 0,
            last_error: None,
            fallback_path,
        }
    }
}

/// The in-memory representation of a single capture event.
#[derive(Debug, Clone)]
pub struct CaptureEvent {
    /// Globally unique event identifier, generated by the client or server.
    ///
    /// This is an opaque stable ID for correlation and possible future
    /// deduplication. It is not ordered; use `sequence_number` for event order
    /// within one `(session_id, event_source)` stream.
    pub event_id: Option<String>,
    /// Event order within one `(session_id, event_source)` stream.
    ///
    /// This is intentionally ordered so analysis can reconstruct event order and
    /// detect missing events. It is not globally unique; use `event_id` for
    /// stable event identity.
    pub sequence_number: Option<i64>,
    /// Capture payload schema version.
    pub schema_version: Option<i32>,
    /// Pseudonymous participant UUID supplied by the extension.
    pub user_id: String,
    /// Logical capture session UUID.
    pub session_id: Option<String>,
    /// Origin of the event stream, such as the VS Code extension.
    pub event_source: Option<String>,
    /// VS Code language identifier for the active file, when known.
    pub language_id: Option<String>,
    /// Privacy-preserving SHA-256 hash of the local file path.
    pub file_hash: Option<String>,
    /// Canonical type of the captured event.
    pub event_type: CaptureEventType,
    /// Server receive/record timestamp, in UTC.
    pub timestamp: DateTime<Utc>,
    /// Client timezone offset in minutes.
    ///
    /// Combined with the server UTC timestamp, this supports local time-of-day
    /// analysis without collecting student location or a full timezone name.
    pub client_tz_offset_min: Option<i32>,
    /// Event-specific payload, stored as JSONB in the DB.
    ///
    /// Known keys include:
    ///
    /// * Capture/settings control: `capture_active`, `capture_control_only`,
    ///   `changed_by`, `changed_settings`, previous/new consent and recording
    ///   booleans.
    /// * Activity/session summaries: `mode`, `closed_by`, `duration_ms`,
    ///   `duration_seconds`, `from`, `to`.
    /// * Save/run/compile metadata: `reason`, `lineCount`, `sessionName`,
    ///   `sessionType`, `taskName`, `taskSource`, `processId`, `exitCode`.
    /// * Write classification: `source`, `classification_basis`, `diff`,
    ///   `doc_block_diff`, `doc_block_count_before`, `doc_block_count_after`.
    ///
    /// Future keys should be documented here, tied to an analysis question, and
    /// privacy-reviewed before capture. Do not store raw source text or raw
    /// local file paths in this payload.
    pub data: serde_json::Value,
}

impl CaptureEvent {
    /// Convenience constructor when the caller already has a timestamp.
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

    /// Constructor for callers that already have first-class capture columns.
    #[allow(clippy::too_many_arguments)]
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

    /// Convenience constructor which uses the current time.
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

/// Internal worker message. Identical to `CaptureEvent`, but separated in case
/// we later want to add batching / flush control signals.
type WorkerMsg = CaptureEvent;

/// Handle used by the rest of the server to record events.
///
/// Cloning this handle is cheap: it only clones an `mpsc::UnboundedSender`.
#[derive(Clone)]
pub struct EventCapture {
    tx: mpsc::UnboundedSender<WorkerMsg>,
    status: Arc<Mutex<CaptureStatus>>,
}

impl EventCapture {
    /// Create a capture worker that writes every event to local JSONL fallback.
    ///
    /// This mode is used for local debugging and for installations without a
    /// configured PostgreSQL capture database.
    pub fn fallback_only(fallback_path: PathBuf) -> Result<Self, io::Error> {
        let status = Arc::new(Mutex::new(CaptureStatus {
            enabled: true,
            state: CaptureState::Fallback,
            queued_events: 0,
            persisted_events: 0,
            fallback_events: 0,
            failed_events: 0,
            last_error: Some("PostgreSQL capture config unavailable".to_string()),
            fallback_path: Some(fallback_path.clone()),
        }));

        info!(
            "Capture: no PostgreSQL config available; writing events to fallback JSONL at {:?}.",
            fallback_path
        );

        let (tx, mut rx) = mpsc::unbounded_channel::<WorkerMsg>();
        let status_worker = status.clone();

        thread::Builder::new()
            .name("codechat-capture-fallback".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Capture: failed to build fallback Tokio runtime");

                runtime.block_on(async move {
                    while let Some(event) = rx.recv().await {
                        write_event_to_fallback(
                            &fallback_path,
                            &event,
                            &status_worker,
                            Some("PostgreSQL capture config unavailable".to_string()),
                        );
                    }
                    warn!("Capture: event channel closed; fallback-only worker exiting.");
                });
            })
            .map_err(|err| {
                io::Error::other(format!(
                    "Capture: failed to start fallback worker thread: {err}"
                ))
            })?;

        Ok(Self { tx, status })
    }

    /// Create a new `EventCapture` instance and spawn a background worker which
    /// consumes events and inserts them into PostgreSQL.
    ///
    /// This function is synchronous so it can be called from non-async server
    /// setup code. It spawns an async task internally which performs the
    /// database connection and event processing.
    pub fn new(mut config: CaptureConfig) -> Result<Self, io::Error> {
        let fallback_path = config
            .fallback_path
            .get_or_insert_with(|| PathBuf::from("capture-events-fallback.jsonl"))
            .clone();
        let conn_str = config.to_conn_str();
        let status = Arc::new(Mutex::new(CaptureStatus::starting(Some(
            fallback_path.clone(),
        ))));

        // High-level DB connection details (no password).
        info!(
            "Capture: preparing PostgreSQL connection ({})",
            config.redacted_summary()
        );

        let (tx, mut rx) = mpsc::unbounded_channel::<WorkerMsg>();
        let status_worker = status.clone();

        // Create a dedicated runtime so capture can be started from sync code
        // before the Actix/Tokio server runtime exists.
        thread::Builder::new()
            .name("codechat-capture".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("Capture: failed to build Tokio runtime");

                runtime.block_on(async move {
                    info!("Capture: attempting to connect to PostgreSQL.");

                    match tokio_postgres::connect(&conn_str, NoTls).await {
                        Ok((client, connection)) => {
                            info!("Capture: successfully connected to PostgreSQL.");
                            update_status(&status_worker, |status| {
                                status.state = CaptureState::Database;
                                status.last_error = None;
                            });

                            // Drive the connection in its own task.
                            let status_connection = status_worker.clone();
                            tokio::spawn(async move {
                                if let Err(err) = connection.await {
                                    error!("Capture PostgreSQL connection error: {err}");
                                    update_status(&status_connection, |status| {
                                        status.state = CaptureState::Fallback;
                                        status.last_error = Some(format!(
                                            "PostgreSQL connection error: {err}"
                                        ));
                                    });
                                }
                            });

                            // Main event loop: pull events off the channel and insert
                            // them into the database.
                            while let Some(event) = rx.recv().await {
                                debug!(
                                    "Capture: inserting event: type={}, user_id={}, file_hash={:?}",
                                    event.event_type, event.user_id, event.file_hash
                                );

                                if let Err(err) = insert_event(&client, &event).await {
                                    error!(
                                        "Capture: FAILED to insert event (type={}, user_id={}): {err}",
                                        event.event_type, event.user_id
                                    );
                                    update_status(&status_worker, |status| {
                                        status.state = CaptureState::Fallback;
                                        status.last_error = Some(format!(
                                            "PostgreSQL insert failed: {err}"
                                        ));
                                    });
                                    write_event_to_fallback(
                                        &fallback_path,
                                        &event,
                                        &status_worker,
                                        Some(format!("PostgreSQL insert failed: {err}")),
                                    );
                                } else {
                                    update_status(&status_worker, |status| {
                                        status.persisted_events += 1;
                                        if status.state != CaptureState::Database {
                                            status.state = CaptureState::Database;
                                        }
                                    });
                                    debug!("Capture: event insert successful.");
                                }
                            }

                            info!("Capture: event channel closed; background worker exiting.");
                        }

                        Err(err) => {
                            let ctx = format!(
                                "Capture: FAILED to connect to PostgreSQL (host={}, dbname={}, user={})",
                                config.host, config.dbname, config.user
                            );

                            log_pg_connect_error(&ctx, &err);

                            update_status(&status_worker, |status| {
                                status.state = CaptureState::Fallback;
                                status.last_error = Some(format!(
                                    "PostgreSQL connection failed: {err}"
                                ));
                            });

                            warn!(
                                "Capture: writing pending events to fallback JSONL at {:?}.",
                                fallback_path
                            );
                            while let Some(event) = rx.recv().await {
                                write_event_to_fallback(
                                    &fallback_path,
                                    &event,
                                    &status_worker,
                                    Some("PostgreSQL connection unavailable".to_string()),
                                );
                            }
                            warn!("Capture: event channel closed; fallback worker exiting.");
                        }
                    }
                });
            })
            .map_err(|err| {
                io::Error::other(format!("Capture: failed to start worker thread: {err}"))
            })?;

        Ok(Self { tx, status })
    }

    /// Enqueue an event for insertion. This is non-blocking.
    pub fn log(&self, event: CaptureEvent) {
        debug!(
            "Capture: queueing event: type={}, user_id={}, file_hash={:?}",
            event.event_type, event.user_id, event.file_hash
        );

        if let Err(err) = self.tx.send(event) {
            error!("Capture: FAILED to enqueue capture event: {err}");
            update_status(&self.status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Failed to enqueue capture event: {err}"));
            });
        } else {
            update_status(&self.status, |status| {
                status.queued_events += 1;
            });
        }
    }

    pub fn status(&self) -> CaptureStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| {
                let mut status = CaptureStatus::disabled();
                status.last_error = Some("Capture status lock is poisoned".to_string());
                status
            })
    }
}

fn update_status(status: &Arc<Mutex<CaptureStatus>>, f: impl FnOnce(&mut CaptureStatus)) {
    match status.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(err) => error!("Capture: unable to update status: {err}"),
    }
}

fn write_event_to_fallback(
    fallback_path: &Path,
    event: &CaptureEvent,
    status: &Arc<Mutex<CaptureStatus>>,
    last_error: Option<String>,
) {
    match append_fallback_event(fallback_path, event) {
        Ok(()) => update_status(status, |status| {
            status.fallback_events += 1;
            status.last_error = last_error;
        }),
        Err(err) => {
            error!(
                "Capture: FAILED to write fallback event to {:?}: {err}",
                fallback_path
            );
            update_status(status, |status| {
                status.failed_events += 1;
                status.last_error = Some(format!("Fallback write failed: {err}"));
            });
        }
    }
}

fn append_fallback_event(fallback_path: &Path, event: &CaptureEvent) -> io::Result<()> {
    if let Some(parent) = fallback_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(fallback_path)?;
    let record = serde_json::json!({
        "fallback_timestamp": Utc::now().to_rfc3339(),
        "event": {
            "event_id": event.event_id,
            "sequence_number": event.sequence_number,
            "schema_version": event.schema_version,
            "user_id": event.user_id,
            "session_id": event.session_id,
            "event_source": event.event_source,
            "language_id": event.language_id,
            "file_hash": event.file_hash,
            "event_type": event.event_type.as_str(),
            "timestamp": event.timestamp.to_rfc3339(),
            "client_tz_offset_min": event.client_tz_offset_min,
            "data": event.data,
        }
    });
    writeln!(file, "{record}")?;
    Ok(())
}

fn log_pg_connect_error(context: &str, err: &tokio_postgres::Error) {
    // If Postgres returned a structured DbError, log it ONCE and bail.
    if let Some(db) = err.as_db_error() {
        // Example: 28P01 = invalid\_password
        error!(
            "{context}: PostgreSQL {} (SQLSTATE {})",
            db.message(),
            db.code().code()
        );

        if let Some(detail) = db.detail() {
            error!("{context}: detail: {detail}");
        }
        if let Some(hint) = db.hint() {
            error!("{context}: hint: {hint}");
        }
        return;
    }

    // Otherwise, try to find an underlying std::io::Error (refused, timed out,
    // DNS, etc.)
    let mut current: &(dyn Error + 'static) = err;
    while let Some(source) = current.source() {
        if let Some(ioe) = source.downcast_ref::<std::io::Error>() {
            error!(
                "{context}: I/O error kind={:?} raw_os_error={:?} msg={}",
                ioe.kind(),
                ioe.raw_os_error(),
                ioe
            );
            return;
        }
        current = source;
    }

    // Fallback: log once (Display)
    error!("{context}: {err}");
}

fn should_retry_legacy_insert(err: &tokio_postgres::Error) -> bool {
    matches!(
        err.code().map(|code| code.code()),
        Some("42703" | "42P01" | "42804")
    )
}

/// Insert a single event into the `events` table.
async fn insert_event(client: &Client, event: &CaptureEvent) -> Result<u64, tokio_postgres::Error> {
    match insert_rich_event(client, event).await {
        Ok(rows) => Ok(rows),
        Err(err) if should_retry_legacy_insert(&err) => {
            warn!(
                "Capture: rich events insert failed against the current schema; retrying legacy insert: {err}"
            );
            insert_legacy_event(client, event).await
        }
        Err(err) => Err(err),
    }
}

async fn insert_rich_event(
    client: &Client,
    event: &CaptureEvent,
) -> Result<u64, tokio_postgres::Error> {
    let timestamp = event.timestamp.to_rfc3339();
    let data_text = event.data.to_string();
    let event_type = event.event_type.as_str();

    debug!(
        "Capture: executing rich INSERT for user_id={}, event_type={}, timestamp={}",
        event.user_id, event_type, timestamp
    );

    client
        .execute(
            "INSERT INTO events \
             (event_id, sequence_number, schema_version, \
              user_id, session_id, \
              event_source, language_id, file_hash, \
              event_type, timestamp, client_tz_offset_min, data) \
             VALUES \
              ($1, $2, $3, \
              $4, $5, \
              $6, $7, $8, \
              $9, $10::text::timestamptz, $11, $12::text::jsonb)",
            &[
                &event.event_id,
                &event.sequence_number,
                &event.schema_version,
                &event.user_id,
                &event.session_id,
                &event.event_source,
                &event.language_id,
                &event.file_hash,
                &event_type,
                &timestamp,
                &event.client_tz_offset_min,
                &data_text,
            ],
        )
        .await
}

async fn insert_legacy_event(
    client: &Client,
    event: &CaptureEvent,
) -> Result<u64, tokio_postgres::Error> {
    let timestamp = event.timestamp.to_rfc3339();
    let data_text = event.data.to_string();
    let event_type = event.event_type.as_str();

    debug!(
        "Capture: executing legacy INSERT for user_id={}, event_type={}, timestamp={}",
        event.user_id, event_type, timestamp
    );
    let file_path: Option<String> = None;

    client
        .execute(
            "INSERT INTO events \
             (user_id, file_path, event_type, timestamp, data) \
             VALUES ($1, $2, $3, $4::text::timestamptz, $5::text::jsonb)",
            &[
                &event.user_id,
                &file_path,
                &event_type,
                &timestamp,
                &data_text,
            ],
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{fs, thread, time::Duration};

    fn valid_capture_config() -> CaptureConfig {
        CaptureConfig {
            host: "localhost".to_string(),
            port: Some(5432),
            user: "alice".to_string(),
            password: "secret".to_string(),
            dbname: "codechat_capture".to_string(),
            app_id: None,
            fallback_path: None,
        }
    }

    #[test]
    fn capture_config_to_conn_str_is_well_formed() {
        let cfg = CaptureConfig {
            host: "localhost".to_string(),
            port: Some(5432),
            user: "alice".to_string(),
            password: "secret".to_string(),
            dbname: "codechat_capture".to_string(),
            app_id: Some("spring25-study".to_string()),
            fallback_path: Some(PathBuf::from("capture-events-fallback.jsonl")),
        };

        let conn = cfg.to_conn_str();
        // Very simple checks: we don't care about ordering beyond what we
        // format.
        assert!(conn.contains("host=localhost"));
        assert!(conn.contains("user=alice"));
        assert!(conn.contains("password=secret"));
        assert!(conn.contains("dbname=codechat_capture"));
        assert!(conn.contains("port=5432"));
        assert!(!cfg.redacted_summary().contains("secret"));
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

        // Timestamp sanity check: it should be between before and after
        assert!(ev.timestamp >= before);
        assert!(ev.timestamp <= after);
    }

    #[test]
    fn fallback_only_capture_writes_jsonl() {
        let fallback_path = std::env::temp_dir().join(format!(
            "codechat-capture-fallback-test-{}-{}.jsonl",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let _ = fs::remove_file(&fallback_path);

        let capture = EventCapture::fallback_only(fallback_path.clone())
            .expect("fallback capture should start");
        capture.log(CaptureEvent::with_columns(
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
            if let Ok(contents) = fs::read_to_string(&fallback_path) {
                text = contents;
                if text.contains("\"event_id\":\"event-1\"") {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(text.contains("\"event_type\":\"save\""));
        assert!(text.contains("\"fallback_timestamp\""));
        assert_eq!(capture.status().state, CaptureState::Fallback);
        let _ = fs::remove_file(&fallback_path);
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
    fn capture_config_json_round_trip() {
        let json_text = r#"
        {
            "host": "db.example.com",
            "user": "bob",
            "port": 5433,
            "password": "hunter2",
            "dbname": "cc_events",
            "app_id": "fall25",
            "fallback_path": "capture-events-fallback.jsonl"
        }
        "#;

        let cfg: CaptureConfig = serde_json::from_str(json_text).expect("JSON should parse");
        assert_eq!(cfg.host, "db.example.com");
        assert_eq!(cfg.port, Some(5433));
        assert_eq!(cfg.user, "bob");
        assert_eq!(cfg.password, "hunter2");
        assert_eq!(cfg.dbname, "cc_events");
        assert_eq!(cfg.app_id.as_deref(), Some("fall25"));
        assert_eq!(
            cfg.fallback_path.as_deref(),
            Some(std::path::Path::new("capture-events-fallback.jsonl"))
        );

        // And it should serialize back to JSON without error
        let _back = serde_json::to_string(&cfg).expect("Should serialize");
    }

    #[test]
    fn capture_config_rejects_port_zero() {
        let cfg = CaptureConfig {
            host: "localhost".to_string(),
            port: Some(0),
            user: "alice".to_string(),
            password: "secret".to_string(),
            dbname: "codechat_capture".to_string(),
            app_id: None,
            fallback_path: None,
        };

        assert!(cfg.validate().is_err());
    }

    #[test]
    fn capture_config_rejects_unquoted_conn_str_whitespace() {
        let mut cfg = valid_capture_config();
        cfg.host = "db host".to_string();
        assert_eq!(
            cfg.validate().unwrap_err(),
            "capture database host must not contain whitespace"
        );

        let mut cfg = valid_capture_config();
        cfg.user = "alice example".to_string();
        assert_eq!(
            cfg.validate().unwrap_err(),
            "capture database user must not contain whitespace"
        );

        let mut cfg = valid_capture_config();
        cfg.password = "secret value".to_string();
        assert_eq!(
            cfg.validate().unwrap_err(),
            "capture database password must not contain whitespace"
        );

        let mut cfg = valid_capture_config();
        cfg.dbname = "codechat capture".to_string();
        assert_eq!(
            cfg.validate().unwrap_err(),
            "capture database dbname must not contain whitespace"
        );
    }

    #[test]
    fn capture_config_rejects_empty_conn_str_fields() {
        let mut cfg = valid_capture_config();
        cfg.host.clear();
        assert_eq!(
            cfg.validate().unwrap_err(),
            "capture database host must not be empty"
        );

        let mut cfg = valid_capture_config();
        cfg.user = " \t".to_string();
        assert_eq!(
            cfg.validate().unwrap_err(),
            "capture database user must not be empty"
        );
    }

    //use tokio::time::{sleep, Duration};

    /// Integration-style test: verify that EventCapture inserts into the rich
    /// capture schema used by dissertation analysis.
    ///
    /// Reads connection parameters from the repo-root `capture_config.json`.
    /// Logs the config and connection details via log4rs so you can confirm
    /// what is used.
    ///
    /// Run this test with:
    /// cargo test event\_capture\_inserts\_rich_schema\_event\_into\_db
    /// -- --ignored --nocapture
    ///
    /// You must have a PostgreSQL database and a `capture_config.json` file
    /// such as: { "host": "localhost", "user": "codechat\_test\_user",
    /// "password": "codechat\_test\_password", "dbname":
    /// "codechat\_capture\_test", "app\_id": "integration-test" }
    #[tokio::test]
    #[ignore]
    async fn event_capture_inserts_rich_schema_event_into_db()
    -> Result<(), Box<dyn std::error::Error>> {
        // Initialize logging for this test, using the same log4rs.yml as the
        // server. If logging is already initialized, this will just return an
        // error which we ignore.
        let _ = log4rs::init_file("log4rs.yml", Default::default());

        // 1. Load the capture configuration from file.
        let cfg_text = fs::read_to_string("../capture_config.json")
            .expect("capture_config.json must exist in the repo root for this test");
        let cfg: CaptureConfig =
            serde_json::from_str(&cfg_text).expect("capture_config.json must be valid JSON");

        log::info!(
            "TEST: Loaded DB config from capture_config.json: host={}, user={}, dbname={}, app_id={:?}",
            cfg.host,
            cfg.user,
            cfg.dbname,
            cfg.app_id
        );

        // 2. Connect directly for setup + verification.
        let conn_str = cfg.to_conn_str();
        log::info!("TEST: Attempting direct tokio_postgres connection for verification.");

        let (client, connection) = tokio_postgres::connect(&conn_str, NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                log::error!("TEST: direct connection error: {e}");
            }
        });

        let required_columns = [
            "event_id",
            "sequence_number",
            "schema_version",
            "session_id",
            "event_source",
            "language_id",
            "file_hash",
            "client_tz_offset_min",
        ];
        for column in required_columns {
            let row = client
                .query_one(
                    r#"
                    SELECT data_type
                    FROM information_schema.columns
                    WHERE table_schema = 'public'
                      AND table_name = 'events'
                      AND column_name = $1
                    "#,
                    &[&column],
                )
                .await
                .map_err(|err| {
                    format!(
                        "TEST SETUP ERROR: missing public.events.{column}; \
                        run server/scripts/capture_events_schema.sql first: {err}"
                    )
                })?;
            let data_type: String = row.get(0);
            info!("TEST: public.events.{column} type={data_type}");
        }

        // 4. Start the EventCapture worker using the loaded config.
        let capture = EventCapture::new(cfg.clone())?;
        log::info!("TEST: EventCapture worker started.");

        // 5. Log a schema-v2 test event with all typed analysis metadata.
        let test_suffix = Utc::now().timestamp_millis().to_string();
        let expected_event_id = format!("TEST_EVENT_{test_suffix}");
        let expected_user_id = format!("TEST_USER_{test_suffix}");
        let expected_session_id = format!("TEST_SESSION_{test_suffix}");
        let expected_file_hash = format!("TEST_FILE_HASH_{test_suffix}");
        let event_timestamp = Utc::now();
        let expected_data = json!({
            "chars_typed": 123,
            "classification_basis": "integration_test"
        });
        let event = CaptureEvent::with_columns(
            Some(expected_event_id.clone()),
            Some(42),
            Some(2),
            expected_user_id.clone(),
            Some(expected_session_id.clone()),
            Some("integration_test".to_string()),
            Some("rust".to_string()),
            Some(expected_file_hash.clone()),
            CaptureEventType::WriteDoc,
            event_timestamp,
            Some(360),
            expected_data.clone(),
        );

        log::info!("TEST: logging a test capture event.");
        capture.log(event);

        // 6. Wait (deterministically) for the background worker to insert the event,
        // then fetch THAT row (instead of "latest row in the table").
        use tokio::time::{Duration, Instant, sleep};

        let deadline = Instant::now() + Duration::from_secs(2);

        let row = loop {
            match client
                .query_one(
                    r#"
                    SELECT user_id, event_type,
                           event_id, sequence_number, schema_version,
                           session_id, event_source, language_id, file_hash,
                           client_tz_offset_min, data::text
                    FROM events
                    WHERE event_id = $1
                    ORDER BY id DESC
                    LIMIT 1
                    "#,
                    &[&expected_event_id],
                )
                .await
            {
                Ok(row) => break row, // found it
                Err(_) => {
                    if Instant::now() >= deadline {
                        return Err("Timed out waiting for EventCapture insert".into());
                    }
                    sleep(Duration::from_millis(50)).await;
                }
            }
        };

        let user_id: String = row.get("user_id");
        let event_type: String = row.get(1);
        let event_id: Option<String> = row.get(2);
        let sequence_number: Option<i64> = row.get(3);
        let schema_version: Option<i32> = row.get(4);
        let session_id: Option<String> = row.get(5);
        let event_source: Option<String> = row.get(6);
        let language_id: Option<String> = row.get(7);
        let file_hash: Option<String> = row.get(8);
        let client_tz_offset_min: Option<i32> = row.get(9);
        let data_text: String = row.get(10);
        let data_value: serde_json::Value = serde_json::from_str(&data_text)?;

        assert_eq!(user_id, expected_user_id);
        assert_eq!(event_type, CaptureEventType::WriteDoc.as_str());
        assert_eq!(event_id.as_deref(), Some(expected_event_id.as_str()));
        assert_eq!(sequence_number, Some(42));
        assert_eq!(schema_version, Some(2));
        assert_eq!(session_id.as_deref(), Some(expected_session_id.as_str()));
        assert_eq!(event_source.as_deref(), Some("integration_test"));
        assert_eq!(language_id.as_deref(), Some("rust"));
        assert_eq!(file_hash.as_deref(), Some(expected_file_hash.as_str()));
        assert_eq!(client_tz_offset_min, Some(360));
        assert_eq!(data_value, expected_data);

        log::info!("✅ TEST: EventCapture integration test succeeded and wrote to database.");
        Ok(())
    }
}
