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

/// `capture.rs` -- Capture CodeChat Editor Events
/// ============================================================================
///
/// This module provides an asynchronous event capture facility backed by a
/// PostgreSQL database. It is designed to support the dissertation study by
/// recording process-level data such as:
///
/// * Frequency and timing of writing entries
/// * Edits to documentation and code
/// * Switches between documentation and coding activity
/// * Duration of engagement with reflective writing
/// * Save, compile, and run events
///
/// Events are sent from the client (browser and/or VS Code extension) to the
/// server as JSON. The server enqueues events into an asynchronous worker which
/// performs batched inserts into the `events` table.
///
/// Database schema
/// ----------------------------------------------------------------------------
///
/// The following SQL statement creates the `events` table used by this module:
///
/// ```sql
/// CREATE TABLE events (
///     id            SERIAL PRIMARY KEY,
///     user_id       TEXT NOT NULL,
///     assignment_id TEXT,
///     group_id      TEXT,
///     file_path     TEXT,
///     event_type    TEXT NOT NULL,
///     timestamp     TEXT NOT NULL,
///     data          TEXT
/// );
/// ```
///
/// * `user_id` – participant identifier (student id, pseudonym, etc.).
/// * `assignment_id` – logical assignment / lab identifier.
/// * `group_id` – optional grouping (treatment / comparison, section).
/// * `file_path` – logical path of the file being edited.
/// * `event_type` – coarse event type (see `event_type` constants below).
/// * `timestamp` – RFC3339 timestamp (in UTC).
/// * `data` – JSON payload with event-specific details.

use std::io;

use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_postgres::{Client, NoTls};
use std::error::Error;

/// Canonical event type strings. Keep these stable for analysis.
pub mod event_types {
    pub const WRITE_DOC: &str = "write_doc";
    pub const WRITE_CODE: &str = "write_code";
    pub const SWITCH_PANE: &str = "switch_pane";
    pub const DOC_SESSION: &str = "doc_session"; // duration of reflective writing
    pub const SAVE: &str = "save";
    pub const COMPILE: &str = "compile";
    pub const RUN: &str = "run";
    pub const SESSION_START: &str = "session_start";
    pub const SESSION_END: &str = "session_end";
}

/// Configuration used to construct the PostgreSQL connection string.
///
/// You can populate this from a JSON file or environment variables in
/// `main.rs`; this module stays agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub host: String,
    pub user: String,
    pub password: String,
    pub dbname: String,
    /// Optional: application-level identifier for this deployment (e.g., course
    /// code or semester). Not stored in the DB directly; callers can embed this
    /// in `data` if desired.
    #[serde(default)]
    pub app_id: Option<String>,
}

impl CaptureConfig {
    /// Build a libpq-style connection string.
    pub fn to_conn_str(&self) -> String {
        format!(
            "host={} user={} password={} dbname={}",
            self.host, self.user, self.password, self.dbname
        )
    }
}

/// The in-memory representation of a single capture event.
#[derive(Debug, Clone)]
pub struct CaptureEvent {
    pub user_id: String,
    pub assignment_id: Option<String>,
    pub group_id: Option<String>,
    pub file_path: Option<String>,
    pub event_type: String,
    /// When the event occurred, in UTC.
    pub timestamp: DateTime<Utc>,
    /// Event-specific payload, stored as JSON text in the DB.
    pub data: serde_json::Value,
}

impl CaptureEvent {
    /// Convenience constructor when the caller already has a timestamp.
    pub fn new(
        user_id: String,
        assignment_id: Option<String>,
        group_id: Option<String>,
        file_path: Option<String>,
        event_type: impl Into<String>,
        timestamp: DateTime<Utc>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            user_id,
            assignment_id,
            group_id,
            file_path,
            event_type: event_type.into(),
            timestamp,
            data,
        }
    }

    /// Convenience constructor which uses the current time.
    pub fn now(
        user_id: String,
        assignment_id: Option<String>,
        group_id: Option<String>,
        file_path: Option<String>,
        event_type: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self::new(
            user_id,
            assignment_id,
            group_id,
            file_path,
            event_type,
            Utc::now(),
            data,
        )
    }
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
}

impl EventCapture {
    /// Create a new `EventCapture` instance and spawn a background worker which
    /// consumes events and inserts them into PostgreSQL.
    ///
    /// This function is synchronous so it can be called from non-async server
    /// setup code. It spawns an async task internally which performs the
    /// database connection and event processing.
    pub fn new(config: CaptureConfig) -> Result<Self, io::Error> {
        let conn_str = config.to_conn_str();

        // High-level DB connection details (no password).
        info!(
            "Capture: preparing PostgreSQL connection (host={}, dbname={}, user={}, app_id={:?})",
            config.host, config.dbname, config.user, config.app_id
        );
        debug!("Capture: raw PostgreSQL connection string: {}", conn_str);

        let (tx, mut rx) = mpsc::unbounded_channel::<WorkerMsg>();

        // Spawn a background task that will connect to PostgreSQL and then
        // process events. This task runs on the Tokio/Actix runtime once the
        // system starts, so the caller does not need to be async.
        tokio::spawn(async move {
            info!("Capture: attempting to connect to PostgreSQL…");

            match tokio_postgres::connect(&conn_str, NoTls).await {
                Ok((client, connection)) => {
                    info!("Capture: successfully connected to PostgreSQL.");

                    // Drive the connection in its own task.
                    tokio::spawn(async move {
                        if let Err(err) = connection.await {
                            error!("Capture PostgreSQL connection error: {err}");
                        }
                    });

                    // Main event loop: pull events off the channel and insert
                    // them into the database.
                    while let Some(event) = rx.recv().await {
                        debug!(
                            "Capture: inserting event: type={}, user_id={}, assignment_id={:?}, group_id={:?}, file_path={:?}",
                            event.event_type,
                            event.user_id,
                            event.assignment_id,
                            event.group_id,
                            event.file_path
                        );

                        if let Err(err) = insert_event(&client, &event).await {
                            error!(
                                "Capture: FAILED to insert event (type={}, user_id={}): {err}",
                                event.event_type, event.user_id
                            );
                        } else {
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

    // Drain and drop any events so we don't hold the sender.
    warn!("Capture: draining pending events after failed DB connection.");
    while rx.recv().await.is_some() {}
    warn!("Capture: all pending events dropped due to connection failure.");
}

                // Err(err) => { // NOTE: we *don't* pass `err` twice here;
                // `{err}` in the format // string already grabs the local `err`
                // binding. error!( "Capture: FAILED to connect to PostgreSQL
                // (host={}, dbname={}, user={}): {err}", config.host,
                // config.dbname, config.user, ); // Drain and drop any events
                // so we don't hold the sender. warn!("Capture: draining pending
                // events after failed DB connection."); while
                // rx.recv().await.is\_some() {} warn!("Capture: all pending
                // events dropped due to connection failure."); }
            }
        });

        Ok(Self { tx })
    }

    /// Enqueue an event for insertion. This is non-blocking.
    pub fn log(&self, event: CaptureEvent) {
        debug!(
            "Capture: queueing event: type={}, user_id={}, assignment_id={:?}, group_id={:?}, file_path={:?}",
            event.event_type,
            event.user_id,
            event.assignment_id,
            event.group_id,
            event.file_path
        );

        if let Err(err) = self.tx.send(event) {
            error!("Capture: FAILED to enqueue capture event: {err}");
        }
    }
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



/// Insert a single event into the `events` table.
async fn insert_event(client: &Client, event: &CaptureEvent) -> Result<u64, tokio_postgres::Error> {
    let timestamp = event.timestamp.to_rfc3339();
    let data_text = event.data.to_string();

    debug!(
        "Capture: executing INSERT for user_id={}, event_type={}, timestamp={}",
        event.user_id, event.event_type, timestamp
    );

    client
        .execute(
            "INSERT INTO events \
             (user_id, assignment_id, group_id, file_path, event_type, timestamp, data) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            &[
                &event.user_id,
                &event.assignment_id,
                &event.group_id,
                &event.file_path,
                &event.event_type,
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
            user: "alice".to_string(),
            password: "secret".to_string(),
            dbname: "codechat_capture".to_string(),
            app_id: Some("spring25-study".to_string()),
        };

        let conn = cfg.to_conn_str();
        // Very simple checks: we don't care about ordering beyond what we
        // format.
        assert!(conn.contains("host=localhost"));
        assert!(conn.contains("user=alice"));
        assert!(conn.contains("password=secret"));
        assert!(conn.contains("dbname=codechat_capture"));
    }

    #[test]
    fn capture_event_new_sets_all_fields() {
        let ts = Utc::now();

        let ev = CaptureEvent::new(
            "user123".to_string(),
            Some("lab1".to_string()),
            Some("groupA".to_string()),
            Some("/path/to/file.rs".to_string()),
            "write_doc",
            ts,
            json!({ "chars_typed": 42 }),
        );

        assert_eq!(ev.user_id, "user123");
        assert_eq!(ev.assignment_id.as_deref(), Some("lab1"));
        assert_eq!(ev.group_id.as_deref(), Some("groupA"));
        assert_eq!(ev.file_path.as_deref(), Some("/path/to/file.rs"));
        assert_eq!(ev.event_type, "write_doc");
        assert_eq!(ev.timestamp, ts);
        assert_eq!(ev.data, json!({ "chars_typed": 42 }));
    }

    #[test]
    fn capture_event_now_uses_current_time_and_fields() {
        let before = Utc::now();
        let ev = CaptureEvent::now(
            "user123".to_string(),
            None,
            None,
            None,
            "save",
            json!({ "reason": "manual" }),
        );
        let after = Utc::now();

        assert_eq!(ev.user_id, "user123");
        assert!(ev.assignment_id.is_none());
        assert!(ev.group_id.is_none());
        assert!(ev.file_path.is_none());
        assert_eq!(ev.event_type, "save");
        assert_eq!(ev.data, json!({ "reason": "manual" }));

        // Timestamp sanity check: it should be between before and after
        assert!(ev.timestamp >= before);
        assert!(ev.timestamp <= after);
    }

    #[test]
    fn capture_config_json_round_trip() {
        let json_text = r#"
        {
            "host": "db.example.com",
            "user": "bob",
            "password": "hunter2",
            "dbname": "cc_events",
            "app_id": "fall25"
        }
        "#;

        let cfg: CaptureConfig = serde_json::from_str(json_text).expect("JSON should parse");
        assert_eq!(cfg.host, "db.example.com");
        assert_eq!(cfg.user, "bob");
        assert_eq!(cfg.password, "hunter2");
        assert_eq!(cfg.dbname, "cc_events");
        assert_eq!(cfg.app_id.as_deref(), Some("fall25"));

        // And it should serialize back to JSON without error
        let _back = serde_json::to_string(&cfg).expect("Should serialize");
    }

    use std::fs;
    //use tokio::time::{sleep, Duration};

    /// Integration-style test: verify that EventCapture actually inserts into
    /// the DB.
    ///
    /// Reads connection parameters from `capture_config.json` in the current
    /// working directory. Logs the config and connection details via log4rs so
    /// you can confirm what is used.
    ///
    /// Run this test with: cargo test event\_capture\_inserts\_event\_into\_db
    /// -- --ignored --nocapture
    ///
    /// You must have a PostgreSQL database and a `capture_config.json` file
    /// such as: { "host": "localhost", "user": "codechat\_test\_user",
    /// "password": "codechat\_test\_password", "dbname":
    /// "codechat\_capture\_test", "app\_id": "integration-test" }
    #[tokio::test]
    #[ignore]
    async fn event_capture_inserts_event_into_db() -> Result<(), Box<dyn std::error::Error>> {

        // Initialize logging for this test, using the same log4rs.yml as the
        // server. If logging is already initialized, this will just return an
        // error which we ignore.
        let _ = log4rs::init_file("log4rs.yml", Default::default());

        // 1. Load the capture configuration from file.
        let cfg_text = fs::read_to_string("capture_config.json")
            .expect("capture_config.json must exist in project root for this test");
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

        // Verify the events table already exists
        let row = client
            .query_one(
                r#"
                SELECT EXISTS (
                    SELECT 1
                    FROM information_schema.tables
                    WHERE table_schema = 'public'
                    AND table_name   = 'events'
                ) AS exists
                "#,
                &[],
            )
            .await?;

        let exists: bool = row.get("exists");
        assert!(
            exists,
            "TEST SETUP ERROR: public.events table does not exist. \
            It must be created by a migration or admin step."
        );

        // Insert a single test row (this is what the app really needs)
        let test_user_id = format!(
            "TEST_USER_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        let insert_row = client
            .query_one(
                r#"
                INSERT INTO public.events
                    (user_id, assignment_id, group_id, file_path, event_type, timestamp, data)
                VALUES
                    ($1, NULL, NULL, NULL, 'test_event', $2, '{"test":true}')
                RETURNING id
                "#,
                &[&test_user_id, &format!("{:?}", std::time::SystemTime::now())],
            )
            .await?;

        let inserted_id: i32 = insert_row.get("id");
        info!("TEST: inserted event id={}", inserted_id);

        // 4. Start the EventCapture worker using the loaded config.
        let capture = EventCapture::new(cfg.clone())?;
        log::info!("TEST: EventCapture worker started.");

        // 5. Log a test event.
        let expected_data = json!({ "chars_typed": 123 });
        let event = CaptureEvent::now(
            "test-user".to_string(),
            Some("hw1".to_string()),
            Some("groupA".to_string()),
            Some("/tmp/test.rs".to_string()),
            event_types::WRITE_DOC,
            expected_data.clone(),
        );

        log::info!("TEST: logging a test capture event.");
        capture.log(event);

        // 6. Wait (deterministically) for the background worker to insert the event,
        // then fetch THAT row (instead of "latest row in the table").
        use tokio::time::{sleep, Duration, Instant};

        let deadline = Instant::now() + Duration::from_secs(2);

        let row = loop {
            match client
                .query_one(
                    r#"
                    SELECT user_id, assignment_id, group_id, file_path, event_type, data
                    FROM events
                    WHERE user_id = $1 AND event_type = $2
                    ORDER BY id DESC
                    LIMIT 1
                    "#,
                    &[&"test-user", &event_types::WRITE_DOC],
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

        let user_id: String = row.get(0);
        let assignment_id: Option<String> = row.get(1);
        let group_id: Option<String> = row.get(2);
        let file_path: Option<String> = row.get(3);
        let event_type: String = row.get(4);
        let data_text: String = row.get(5);
        let data_value: serde_json::Value = serde_json::from_str(&data_text)?;

        assert_eq!(user_id, "test-user");
        assert_eq!(assignment_id.as_deref(), Some("hw1"));
        assert_eq!(group_id.as_deref(), Some("groupA"));
        assert_eq!(file_path.as_deref(), Some("/tmp/test.rs"));
        assert_eq!(event_type, event_types::WRITE_DOC);
        assert_eq!(data_value, expected_data);

        log::info!("✅ TEST: EventCapture integration test succeeded and wrote to database.");
        Ok(())
    }
}
