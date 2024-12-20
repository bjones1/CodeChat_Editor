// Copyright (C) 2023 Bryan A. Jones.
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
/// # `Capture.rs` -- Capture CodeChat Editor Events
// ## Submodules

// ## Imports
//
// Standard library
use indoc::indoc;
use lazy_static::lazy_static;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

// Third-party
use chrono::Local;
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

// Local

/* ## The Event Structure:

   The `Event` struct represents an event to be stored in the database.

   Fields: - `user_id`: The ID of the user associated with the event. -
   `event_type`: The type of event (e.g., "keystroke", "file_open"). - `data`:
   Optional additional data associated with the event.

   ### Example

   let event = Event { user_id: "user123".to_string(), event_type:
   "keystroke".to_string(), data: Some("Pressed key A".to_string()), };
*/

#[derive(Deserialize, Debug)]
pub struct Event {
    pub user_id: String,
    pub event_type: String,
    pub data: Option<String>,
}

/*
    ## The Config Structure:

    The `Config` struct represents the database connection parameters read from
   `config.json`.

   Fields: - `db_host`: The hostname or IP address of the database server. -
   `db_user`: The username for the database connection. - `db_password`: The
   password for the database connection. - `db_name`: The name of the database.

   let config = Config { db_host: "localhost".to_string(), db_user:
   "your_db_user".to_string(), db_password: "your_db_password".to_string(),
   db_name: "your_db_name".to_string(), };
*/

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub db_ip: String,
    pub db_user: String,
    pub db_password: String,
    pub db_name: String,
}

/*

 ## The EventCapture Structure:

 The `EventCapture` struct provides methods to interact with the database. It
holds a `tokio_postgres::Client` for database operations.

### Usage Example

#\[tokio::main\] async fn main() -> Result<(), Box> {

```
 // Create an instance of EventCapture using the configuration file
 let event_capture = EventCapture::new("config.json").await?;

 // Create an event
 let event = Event {
     user_id: "user123".to_string(),
     event_type: "keystroke".to_string(),
     data: Some("Pressed key A".to_string()),
 };

 // Insert the event into the database
 event_capture.insert_event(event).await?;

 Ok(())
```
} */

pub struct EventCapture {
    db_client: Arc<Mutex<Client>>,
}

// lazy_static! {
//     pub static ref GLOBAL_EVENT_CAPTURE: Arc<EventCapture> = {
//         // Create a synchronous runtime for the async initialization
//         let rt = Runtime::new().expect("Failed to create tokio runtime");
//         let capture = rt.block_on(EventCapture::new("config.json"))
//             .expect("Failed to initialize EventCapture");
//         Arc::new(capture)
//     };
// }

/*
    ## The EventCapture Implementation
*/

impl EventCapture {
    /*
        Creates a new `EventCapture` instance by reading the database connection parameters from the `config.json` file and connecting to the PostgreSQL database.
            # Arguments
            - config_path: The file path to the config.json file.

            # Returns

            A `Result` containing an `EventCapture` instance
    */

    pub async fn new<P: AsRef<Path>>(config_path: P) -> Result<Self, io::Error> {
        // Read the configuration file
        let config_content =
            fs::read_to_string(config_path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let config: Config = serde_json::from_str(&config_content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Build the connection string for the PostgreSQL database
        let conn_str = format!(
            "host={} user={} password={} dbname={}",
            config.db_ip, config.db_user, config.db_password, config.db_name
        );

        info!(
            "Attempting Capture Database Connection. IP:[{}] Username:[{}] Database Name:[{}]",
            config.db_ip, config.db_user, config.db_name
        );

        // Connect to the database asynchronously
        let (client, connection) = tokio_postgres::connect(&conn_str, NoTls)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;

        // Spawn a task to manage the database connection in the background
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("Database connection error: [{}]", e);
            }
        });

        info!(
            "Connected to Database [{}] as User [{}]",
            config.db_name, config.db_user
        );

        Ok(EventCapture {
            db_client: Arc::new(Mutex::new(client)),
        })
    }

    /*
       Inserts an event into the database.

       # Arguments
       - `event`: An `Event` instance containing the event data to insert.

       # Returns
       A `Result` indicating success or containing a `tokio_postgres::Error`.

       # Example
       #[tokio::main]
       async fn main() -> Result<(), Box<dyn std::error::Error>> {
           let event_capture = EventCapture::new("config.json").await?;

           let event = Event {
               user_id: "user123".to_string(),
               event_type: "keystroke".to_string(),
               data: Some("Pressed key A".to_string()),
           };

           event_capture.insert_event(event).await?;
           Ok(())
       }
    */

    pub async fn insert_event(&self, event: Event) -> Result<(), io::Error> {
        let current_time = Local::now();
        let formatted_time = current_time.to_rfc3339();

        // SQL statement to insert the event into the 'events' table
        let stmt = indoc! {"
            INSERT INTO events (user_id, event_type, timestamp, data)
            VALUES ($1, $2, $3, $4)
        "};

        // Acquire a lock on the database client for thread-safe access
        let client = self.db_client.lock().await;

        // Execute the SQL statement with the event data
        client
            .execute(
                stmt,
                &[
                    &event.user_id,
                    &event.event_type,
                    &formatted_time,
                    &event.data,
                ],
            )
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        info!("Event inserted into database: {:?}", event);

        Ok(())
    }
}

/* Database Schema (SQL DDL)

The following SQL statement creates the `events` table used by this library:

CREATE TABLE events ( id SERIAL PRIMARY KEY, user_id TEXT NOT NULL,
event_type TEXT NOT NULL, timestamp TEXT NOT NULL, data TEXT );

- **`id SERIAL PRIMARY KEY`**: Auto-incrementing primary key.
- **`user_id TEXT NOT NULL`**: The ID of the user associated with the event.
- **`event_type TEXT NOT NULL`**: The type of event.
- **`timestamp TEXT NOT NULL`**: The timestamp of the event.
- **`data TEXT`**: Optional additional data associated with the event.
  **Note:** Ensure this table exists in your PostgreSQL database before using
  the library. */
