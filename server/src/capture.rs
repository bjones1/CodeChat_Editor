use log::{error, info};
use serde::Deserialize;
use simplelog::*;
use simplelog::{ColorChoice, TermLogger, TerminalMode};
use std::fs;
use std::fs::File;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_postgres::{Client, NoTls}; // Import necessary types

#[derive(Debug, Deserialize)]
struct DatabaseConfig {
    db_name: String,
    db_ip: String,
    db_user: String,
    db_password: String,
}

#[derive(Deserialize, Debug)]
struct Event {
    user_id: String,
    event_type: String,
    timestamp: String,
    data: Option<String>,
}

fn load_config() -> Result<DatabaseConfig, Box<dyn std::error::Error>> {
    // Specify the path to your configuration file
    let config_path = Path::new("config.json");
    let config_data = fs::read_to_string(config_path)?;
    let config: DatabaseConfig = serde_json::from_str(&config_data)?;
    Ok(config)
}

async fn db_connect() -> Result<Client, tokio_postgres::Error> {
    // Load the database configuration
    let config = load_config().expect("Failed to load database configuration");

    let db_user = &config.db_user;
    let db_password = &config.db_password;
    let db_name = &config.db_name;
    let db_host = &config.db_ip;

    info!(
        "Connecting to Database:[{}] IP:[{}] as User [{}]...",
        db_name, db_host, db_user
    );

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
            error!("Database connection error: [{}]", e);
        }
    });

    // Print a message for successful DB connection
    info!(
        "Successfully Connected to Database [{}] as User [{}]",
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
        WriteLogger::new(LevelFilter::Info, config, log_file),
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
