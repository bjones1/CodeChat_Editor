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
// user_id, session_id, event_source, language_id, file_hash, file_path,
// path_privacy, event_type, timestamp, client_timestamp_ms,
// client_tz_offset_min, server_timestamp_ms, data
// ```
//
// * `user_id` – pseudonymous participant UUID. Course, group, assignment, and
//   study condition are intentionally joined later from researcher-managed
//   participant/date mappings instead of being configured by students.
// * `session_id`, `event_id`, `sequence_number`, `schema_version` – event
//   integrity and versioning metadata.
// * `file_path` – logical path of the file being edited.
// * `file_hash` – privacy-preserving SHA-256 hash of the file path.
// * `event_type` – coarse event type (see `CaptureEventType` below).
// * `timestamp` – RFC3339 timestamp (in UTC).
// * `data` – JSONB payload with event-specific details.

use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
    thread,
};

use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::sync::mpsc;
use tokio_postgres::{Client, NoTls};
use ts_rs::TS;

static NEXT_CAPTURE_EVENT_ID: AtomicU64 = AtomicU64::new(1);

/// Canonical event types. Keep the serialized strings stable for analysis.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureEventType {
    /// Server-classified edit to documentation/prose.
    WriteDoc,
    /// Server-classified edit to executable source code.
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

pub mod event_types {
    use super::CaptureEventType;

    pub const WRITE_DOC: CaptureEventType = CaptureEventType::WriteDoc;
    pub const WRITE_CODE: CaptureEventType = CaptureEventType::WriteCode;
    pub const SWITCH_PANE: CaptureEventType = CaptureEventType::SwitchPane;
    pub const DOC_SESSION: CaptureEventType = CaptureEventType::DocSession;
    pub const SAVE: CaptureEventType = CaptureEventType::Save;
    pub const COMPILE: CaptureEventType = CaptureEventType::Compile;
    pub const RUN: CaptureEventType = CaptureEventType::Run;
    pub const SESSION_START: CaptureEventType = CaptureEventType::SessionStart;
    pub const SESSION_END: CaptureEventType = CaptureEventType::SessionEnd;
    /// Audit row emitted when the user changes consent or recording settings.
    pub const CAPTURE_SETTINGS_CHANGED: CaptureEventType = CaptureEventType::CaptureSettingsChanged;
    pub const COMPILE_END: CaptureEventType = CaptureEventType::CompileEnd;
    pub const RUN_END: CaptureEventType = CaptureEventType::RunEnd;
    pub const TASK_START: CaptureEventType = CaptureEventType::TaskStart;
    pub const TASK_SUBMIT: CaptureEventType = CaptureEventType::TaskSubmit;
    pub const DEBUG_TASK_START: CaptureEventType = CaptureEventType::DebugTaskStart;
    pub const DEBUG_TASK_SUBMIT: CaptureEventType = CaptureEventType::DebugTaskSubmit;
    pub const HANDOFF_START: CaptureEventType = CaptureEventType::HandoffStart;
    pub const HANDOFF_END: CaptureEventType = CaptureEventType::HandoffEnd;
    pub const REFLECTION_PROMPT_INSERTED: CaptureEventType =
        CaptureEventType::ReflectionPromptInserted;
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

        Ok(Some(Self {
            host,
            port,
            user: required_env_var("CODECHAT_CAPTURE_USER")?,
            password: required_env_var("CODECHAT_CAPTURE_PASSWORD")?,
            dbname: required_env_var("CODECHAT_CAPTURE_DBNAME")?,
            app_id: env_var_trimmed("CODECHAT_CAPTURE_APP_ID"),
            fallback_path: env_var_trimmed("CODECHAT_CAPTURE_FALLBACK_PATH").map(PathBuf::from),
        }))
    }
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
            Ok(cfg) => Some(with_default_capture_fallback_path(cfg, root_path)),
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

/// Capture worker health, exposed through `/capture/status` and the VS Code
/// status item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export)]
pub struct CaptureStatus {
    /// True when the capture worker is configured and accepting events.
    pub enabled: bool,
    /// Worker state: `starting`, `database`, `fallback`, or `disabled`.
    pub state: String,
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
            state: "disabled".to_string(),
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
            state: "starting".to_string(),
            queued_events: 0,
            persisted_events: 0,
            fallback_events: 0,
            failed_events: 0,
            last_error: None,
            fallback_path,
        }
    }
}

/// Typed metadata stored in first-class DB columns instead of the event-specific
/// JSON `data` payload.
#[derive(Debug, Clone, Default)]
pub struct CaptureEventMetadata {
    /// Globally unique event identifier, generated by the client or server.
    pub event_id: Option<String>,
    /// Client-local event order for one extension session.
    pub sequence_number: Option<i64>,
    /// Capture payload schema version.
    pub schema_version: Option<i32>,
    /// Logical capture session UUID.
    pub session_id: Option<String>,
    /// Origin of the event stream, such as the VS Code extension.
    pub event_source: Option<String>,
    /// VS Code language identifier for the active file, when known.
    pub language_id: Option<String>,
    /// Privacy-preserving SHA-256 hash of the local file path.
    pub file_hash: Option<String>,
    /// Whether the path was sent plainly, hashed, or omitted.
    pub path_privacy: Option<String>,
    /// Client timestamp, in milliseconds since Unix epoch.
    pub client_timestamp_ms: Option<i64>,
    /// Client timezone offset in minutes.
    pub client_tz_offset_min: Option<i32>,
    /// Server timestamp, in milliseconds since Unix epoch.
    pub server_timestamp_ms: Option<i64>,
}

/// The in-memory representation of a single capture event.
#[derive(Debug, Clone)]
pub struct CaptureEvent {
    /// Globally unique event identifier, generated by the client or server.
    pub event_id: Option<String>,
    /// Client-local event order for one extension session.
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
    /// Raw file path when path hashing is disabled.
    pub file_path: Option<String>,
    /// Whether the path was sent plainly, hashed, or omitted.
    pub path_privacy: Option<String>,
    /// Canonical type of the captured event.
    pub event_type: CaptureEventType,
    /// When the event occurred, in UTC.
    pub timestamp: DateTime<Utc>,
    /// Client timestamp, in milliseconds since Unix epoch.
    pub client_timestamp_ms: Option<i64>,
    /// Client timezone offset in minutes.
    pub client_tz_offset_min: Option<i32>,
    /// Server timestamp, in milliseconds since Unix epoch.
    pub server_timestamp_ms: i64,
    /// Event-specific payload, stored as JSON text in the DB.
    pub data: serde_json::Value,
}

impl CaptureEvent {
    /// Convenience constructor when the caller already has a timestamp.
    pub fn new(
        user_id: String,
        file_path: Option<String>,
        event_type: CaptureEventType,
        timestamp: DateTime<Utc>,
        data: serde_json::Value,
    ) -> Self {
        Self::with_metadata(
            user_id,
            file_path,
            event_type,
            timestamp,
            data,
            CaptureEventMetadata::default(),
        )
    }

    /// Constructor for callers that already have first-class capture metadata.
    pub fn with_metadata(
        user_id: String,
        file_path: Option<String>,
        event_type: CaptureEventType,
        timestamp: DateTime<Utc>,
        data: serde_json::Value,
        metadata: CaptureEventMetadata,
    ) -> Self {
        Self {
            event_id: metadata.event_id,
            sequence_number: metadata.sequence_number,
            schema_version: metadata.schema_version,
            user_id,
            session_id: metadata.session_id,
            event_source: metadata.event_source,
            language_id: metadata.language_id,
            file_hash: metadata.file_hash,
            file_path,
            path_privacy: metadata.path_privacy,
            event_type,
            timestamp,
            client_timestamp_ms: metadata.client_timestamp_ms,
            client_tz_offset_min: metadata.client_tz_offset_min,
            server_timestamp_ms: metadata
                .server_timestamp_ms
                .unwrap_or_else(|| timestamp.timestamp_millis()),
            data,
        }
    }

    /// Convenience constructor which uses the current time.
    pub fn now(
        user_id: String,
        file_path: Option<String>,
        event_type: CaptureEventType,
        data: serde_json::Value,
    ) -> Self {
        Self::new(user_id, file_path, event_type, Utc::now(), data)
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
                                status.state = "database".to_string();
                                status.last_error = None;
                            });

                            // Drive the connection in its own task.
                            let status_connection = status_worker.clone();
                            tokio::spawn(async move {
                                if let Err(err) = connection.await {
                                    error!("Capture PostgreSQL connection error: {err}");
                                    update_status(&status_connection, |status| {
                                        status.state = "fallback".to_string();
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
                                    "Capture: inserting event: type={}, user_id={}, file_path={:?}",
                                    event.event_type, event.user_id, event.file_path
                                );

                                if let Err(err) = insert_event(&client, &event).await {
                                    error!(
                                        "Capture: FAILED to insert event (type={}, user_id={}): {err}",
                                        event.event_type, event.user_id
                                    );
                                    update_status(&status_worker, |status| {
                                        status.state = "fallback".to_string();
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
                                        if status.state != "database" {
                                            status.state = "database".to_string();
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
                                status.state = "fallback".to_string();
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
            "Capture: queueing event: type={}, user_id={}, file_path={:?}",
            event.event_type, event.user_id, event.file_path
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
            "file_path": event.file_path,
            "path_privacy": event.path_privacy,
            "event_type": event.event_type.as_str(),
            "timestamp": event.timestamp.to_rfc3339(),
            "client_timestamp_ms": event.client_timestamp_ms,
            "client_tz_offset_min": event.client_tz_offset_min,
            "server_timestamp_ms": event.server_timestamp_ms,
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
              event_source, language_id, file_hash, file_path, path_privacy, \
              event_type, timestamp, client_timestamp_ms, client_tz_offset_min, \
              server_timestamp_ms, data) \
             VALUES \
              ($1, $2, $3, \
              $4, $5, \
              $6, $7, $8, $9, $10, \
              $11, $12::text::timestamptz, $13, $14, \
              $15, $16::text::jsonb)",
            &[
                &event.event_id,
                &event.sequence_number,
                &event.schema_version,
                &event.user_id,
                &event.session_id,
                &event.event_source,
                &event.language_id,
                &event.file_hash,
                &event.file_path,
                &event.path_privacy,
                &event_type,
                &timestamp,
                &event.client_timestamp_ms,
                &event.client_tz_offset_min,
                &event.server_timestamp_ms,
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

    client
        .execute(
            "INSERT INTO events \
             (user_id, file_path, event_type, timestamp, data) \
             VALUES ($1, $2, $3, $4::text::timestamptz, $5::text::jsonb)",
            &[
                &event.user_id,
                &event.file_path,
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
            serde_json::to_value(event_types::WRITE_DOC).unwrap(),
            json!("write_doc")
        );
        assert_eq!(
            serde_json::from_value::<CaptureEventType>(json!("compile_end")).unwrap(),
            event_types::COMPILE_END
        );
        assert_eq!(
            serde_json::to_value(event_types::CAPTURE_SETTINGS_CHANGED).unwrap(),
            json!("capture_settings_changed")
        );
        assert!(serde_json::from_value::<CaptureEventType>(json!("random")).is_err());
    }

    #[test]
    fn capture_event_new_sets_all_fields() {
        let ts = Utc::now();

        let ev = CaptureEvent::new(
            "user123".to_string(),
            Some("/path/to/file.rs".to_string()),
            event_types::WRITE_DOC,
            ts,
            json!({ "chars_typed": 42 }),
        );

        assert_eq!(ev.user_id, "user123");
        assert_eq!(ev.file_path.as_deref(), Some("/path/to/file.rs"));
        assert_eq!(ev.event_type, event_types::WRITE_DOC);
        assert_eq!(ev.timestamp, ts);
        assert_eq!(ev.server_timestamp_ms, ts.timestamp_millis());
        assert!(ev.event_id.is_none());
        assert_eq!(ev.data, json!({ "chars_typed": 42 }));
    }

    #[test]
    fn capture_event_now_uses_current_time_and_fields() {
        let before = Utc::now();
        let ev = CaptureEvent::now(
            "user123".to_string(),
            None,
            event_types::SAVE,
            json!({ "reason": "manual" }),
        );
        let after = Utc::now();

        assert_eq!(ev.user_id, "user123");
        assert!(ev.file_path.is_none());
        assert_eq!(ev.event_type, event_types::SAVE);
        assert_eq!(ev.server_timestamp_ms, ev.timestamp.timestamp_millis());
        assert_eq!(ev.data, json!({ "reason": "manual" }));

        // Timestamp sanity check: it should be between before and after
        assert!(ev.timestamp >= before);
        assert!(ev.timestamp <= after);
    }

    #[test]
    fn capture_event_with_metadata_sets_analysis_columns() {
        let ts = Utc::now();

        let ev = CaptureEvent::with_metadata(
            "user123".to_string(),
            None,
            event_types::WRITE_CODE,
            ts,
            json!({ "chars_typed": 42 }),
            CaptureEventMetadata {
                event_id: Some("abc-123".to_string()),
                sequence_number: Some(42),
                schema_version: Some(2),
                session_id: Some("session-1".to_string()),
                event_source: Some("vscode_extension".to_string()),
                language_id: Some("rust".to_string()),
                file_hash: Some("hash".to_string()),
                path_privacy: Some("sha256".to_string()),
                client_timestamp_ms: Some(ts.timestamp_millis() - 50),
                client_tz_offset_min: Some(-360),
                server_timestamp_ms: Some(ts.timestamp_millis()),
            },
        );

        assert_eq!(ev.event_id.as_deref(), Some("abc-123"));
        assert_eq!(ev.sequence_number, Some(42));
        assert_eq!(ev.schema_version, Some(2));
        assert_eq!(ev.session_id.as_deref(), Some("session-1"));
        assert_eq!(ev.event_source.as_deref(), Some("vscode_extension"));
        assert_eq!(ev.language_id.as_deref(), Some("rust"));
        assert_eq!(ev.file_hash.as_deref(), Some("hash"));
        assert_eq!(ev.path_privacy.as_deref(), Some("sha256"));
        assert_eq!(ev.client_timestamp_ms, Some(ts.timestamp_millis() - 50));
        assert_eq!(ev.client_tz_offset_min, Some(-360));
        assert_eq!(ev.server_timestamp_ms, ts.timestamp_millis());
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

    use std::fs;
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
            "path_privacy",
            "client_timestamp_ms",
            "client_tz_offset_min",
            "server_timestamp_ms",
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
        let expected_server_timestamp_ms = event_timestamp.timestamp_millis();
        let expected_client_timestamp_ms = expected_server_timestamp_ms - 50;
        let expected_data = json!({
            "chars_typed": 123,
            "classification_basis": "integration_test"
        });
        let event = CaptureEvent::with_metadata(
            expected_user_id.clone(),
            None,
            event_types::WRITE_DOC,
            event_timestamp,
            expected_data.clone(),
            CaptureEventMetadata {
                event_id: Some(expected_event_id.clone()),
                sequence_number: Some(42),
                schema_version: Some(2),
                session_id: Some(expected_session_id.clone()),
                event_source: Some("integration_test".to_string()),
                language_id: Some("rust".to_string()),
                file_hash: Some(expected_file_hash.clone()),
                path_privacy: Some("sha256".to_string()),
                client_timestamp_ms: Some(expected_client_timestamp_ms),
                client_tz_offset_min: Some(360),
                server_timestamp_ms: Some(expected_server_timestamp_ms),
            },
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
                    SELECT user_id, file_path, event_type,
                           event_id, sequence_number, schema_version,
                           session_id, event_source, language_id, file_hash,
                           path_privacy, client_timestamp_ms,
                           client_tz_offset_min, server_timestamp_ms, data::text
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
        let file_path: Option<String> = row.get(1);
        let event_type: String = row.get(2);
        let event_id: Option<String> = row.get(3);
        let sequence_number: Option<i64> = row.get(4);
        let schema_version: Option<i32> = row.get(5);
        let session_id: Option<String> = row.get(6);
        let event_source: Option<String> = row.get(7);
        let language_id: Option<String> = row.get(8);
        let file_hash: Option<String> = row.get(9);
        let path_privacy: Option<String> = row.get(10);
        let client_timestamp_ms: Option<i64> = row.get(11);
        let client_tz_offset_min: Option<i32> = row.get(12);
        let server_timestamp_ms: Option<i64> = row.get(13);
        let data_text: String = row.get(14);
        let data_value: serde_json::Value = serde_json::from_str(&data_text)?;

        assert_eq!(user_id, expected_user_id);
        assert!(file_path.is_none());
        assert_eq!(event_type, event_types::WRITE_DOC.as_str());
        assert_eq!(event_id.as_deref(), Some(expected_event_id.as_str()));
        assert_eq!(sequence_number, Some(42));
        assert_eq!(schema_version, Some(2));
        assert_eq!(session_id.as_deref(), Some(expected_session_id.as_str()));
        assert_eq!(event_source.as_deref(), Some("integration_test"));
        assert_eq!(language_id.as_deref(), Some("rust"));
        assert_eq!(file_hash.as_deref(), Some(expected_file_hash.as_str()));
        assert_eq!(path_privacy.as_deref(), Some("sha256"));
        assert_eq!(client_timestamp_ms, Some(expected_client_timestamp_ms));
        assert_eq!(client_tz_offset_min, Some(360));
        assert_eq!(server_timestamp_ms, Some(expected_server_timestamp_ms));
        assert_eq!(data_value, expected_data);

        log::info!("✅ TEST: EventCapture integration test succeeded and wrote to database.");
        Ok(())
    }
}
