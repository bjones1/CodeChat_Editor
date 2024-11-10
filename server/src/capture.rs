use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_postgres::{NoTls, Client};
use serde::Deserialize;
use tokio::sync::mpsc;
use simplelog::*;
use std::fs::File;
use log::{error, info};
use simplelog::{TermLogger, TerminalMode, ColorChoice}; // Import necessary types

#[derive(Deserialize, Debug)]
struct Event {
    user_id: String,
    event_type: String,
    timestamp: String,
    data: Option<String>,
}

async fn db_connect() -> Result<Client, tokio_postgres::Error> {
    // Hardcoded database credentials
    let db_user = "CodeChatCaptureUser";
    let db_password = "OB3yc8Hk9SuVjzXMdUDr0C7w4PqLQisn"; // Ensure special characters are escaped
    let db_name = "CodeChatCaptureDB";
    let db_host = "3.146.138.182";

    // Build the connection string
    let conn_str = format!(
        "host={} user={} password={} dbname={}",
        db_host, db_user, db_password, db_name
    );

    // Connect to the database
    let (client, connection) = tokio_postgres::connect(&conn_str, NoTls).await?;

    // Spawn the connection handling in the background
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!("Database connection error: {}", e);
        }
    });

    // Print a message for successful DB connection
    info!(
        "Successfully connected to database '{}' as user '{}'",
        db_name, db_user
    );

    Ok(client)
}

#[tokio::main]
pub async fn run() -> std::io::Result<()> {
    // Initialize logging
    let log_file_path = "event_capture.log";
    let log_file = File::create(log_file_path)?; // Clears the log file for every run

    let config = {
        let mut cfg_builder = ConfigBuilder::new();
        let cfg_builder = match cfg_builder.set_time_offset_to_local() {
            Ok(cfg) => cfg,
            Err(cfg) => cfg, // Proceed even if setting the local time offset fails
        };
        cfg_builder
            //.set_time_format(Some("%Y-%m-%d %H:%M:%S"))
            .build()
    };

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Info,
            config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            config,
            log_file,
        ),
    ])
    .unwrap();

    info!("Starting Event Capture Server");

    // Start the TCP server
    let listen_addr = "0.0.0.0";
    let listen_port = 3947;
    let listener = TcpListener::bind((listen_addr, listen_port)).await?;
    info!("Server listening on {}:{}", listen_addr, listen_port);

    // Create a channel for events
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(100);

    // Spawn a task to handle database operations
    tokio::spawn(async move {
        match db_connect().await {
            Ok(db_client) => {
                while let Some(event) = event_rx.recv().await {
                    if let Err(e) = insert_event(&db_client, &event).await {
                        error!("Failed to insert event: {}", e);
                    } else {
                        info!("Event inserted: {:?}", event);
                    }
                }
            }
            Err(e) => {
                error!("Failed to connect to DB: {}", e);
            }
        }
    });

    loop {
        let (socket, addr) = listener.accept().await?;
        info!("New connection from {}", addr);

        let event_tx = event_tx.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(socket);
            let mut buffer = Vec::new();

            loop {
                buffer.clear();
                // Read until newline (\n)
                match reader.read_until(b'\n', &mut buffer).await {
                    Ok(0) => {
                        // Connection closed
                        break;
                    }
                    Ok(_) => {
                        let data = String::from_utf8_lossy(&buffer);
                        // Trim the newline character
                        let data = data.trim_end_matches('\n');

                        match serde_json::from_str::<Event>(&data) {
                            Ok(event) => {
                                if let Err(e) = event_tx.send(event).await {
                                    error!("Failed to send event: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse event: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read from socket: {}", e);
                        break;
                    }
                }
            }
            info!("Connection from {} closed", addr);
        });
    }
}

async fn insert_event(db_client: &Client, event: &Event) -> Result<(), tokio_postgres::Error> {
    let stmt = "
        INSERT INTO events (user_id, event_type, timestamp, data)
        VALUES ($1, $2, $3, $4)
    ";

    db_client
        .execute(
            stmt,
            &[
                &event.user_id,
                &event.event_type,
                &event.timestamp,
                &event.data,
            ],
        )
        .await?;

    Ok(())
}
