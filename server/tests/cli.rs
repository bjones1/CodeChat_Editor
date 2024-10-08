// # `cli.rs` - Test the CLI interface
//
// ## Imports
//
// ### Standard library
// None.
//
// ### Third-party
use actix_web::{App, HttpServer};
use assert_cmd::Command;
use predicates::str::contains;

// ### Local
use code_chat_editor::webserver::IP_ADDRESS;
use tokio::task::spawn_blocking;

// ## Tests
#[test]
fn test_start_not_found() {
    let mut cmd = Command::cargo_bin("codechat-editor-server").unwrap();
    let assert = cmd.args(["--test-mode", "not-found", "start"]).assert();
    assert
        .failure()
        .stderr("Failed to start server: program not found\n");
}

#[test]
fn test_start_no_start() {
    let mut cmd = Command::cargo_bin("codechat-editor-server").unwrap();
    let assert = cmd
        .args(["--test-mode", "sleep", "--port", "8081", "start"])
        .assert();
    assert
        .failure()
        .stderr(contains("Server failed to start after 5 seconds.\n"));
}

#[actix_web::test]
async fn test_start_no_response() {
    // Run a dummy webserver that doesn't respond to the `/stop` endpoint.
    actix_rt::spawn(async move {
        HttpServer::new(App::new)
            .bind((IP_ADDRESS, 8082))
            .unwrap()
            .run()
            .await
            .unwrap();
    });
    // The test must be run in a separate thread to avoid blocking the main thread; otherwise, the webserver will not respond.
    let test = spawn_blocking(move || {
        let mut cmd = Command::cargo_bin("codechat-editor-server").unwrap();
        let assert = cmd
            .args(["--test-mode", "sleep", "--port", "8082", "start"])
            .assert();
        assert
            .failure()
            .stderr(contains("Server failed to start after 5 seconds.\n"))
            .stderr(contains("status code = 404\n"));
    });
    test.await.unwrap();
}
