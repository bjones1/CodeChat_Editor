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
/// `overall.rs` - test the overall system
/// ======================================
///
/// These are functional tests of the overall system, performed by attaching a
/// testing IDE to generate commands then observe results, along with a browser
/// tester.
// Imports
// -------
//
#[cfg(feature = "all_tests")]
// ### Standard library
use {
    // ### Local
    code_chat_editor::{
        cast,
        ide::CodeChatEditorServer,
        webserver::{EditorMessage, EditorMessageContents, ResultOkTypes, set_root_path},
    },
    // ### Third-party
    pretty_assertions::assert_eq,
    std::{env, error::Error},

    thirtyfour::prelude::*,
    tokio::time::sleep,
};

// Tests
// -----
#[cfg(feature = "all_tests")]
#[tokio::test]
async fn thirtyfour() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Start the webdriver.
    let server_url = "http://localhost:4444";
    let caps = DesiredCapabilities::chrome();
    start_webdriver_process(server_url, &caps);
    let driver = WebDriver::new(server_url, caps).await?;

    // Run the test.
    let ret = test_body(&driver).await;

    // Always explicitly close the browser.
    driver.quit().await?;
    ret
}

#[cfg(feature = "all_tests")]
async fn test_body(driver: &WebDriver) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Set up the Server.
    let p = env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("../../../..");
    set_root_path(Some(&p))?;
    let codechat_server = CodeChatEditorServer::new()?;

    // Get the resulting web page text.
    let opened_id = codechat_server.send_message_opened(true).await?;
    assert_eq!(
        codechat_server
            .get_message()
            .await
            .expect("Expected message."),
        EditorMessage {
            id: opened_id,
            message: EditorMessageContents::Result(Ok(ResultOkTypes::Void))
        }
    );
    let em_html = codechat_server
        .get_message()
        .await
        .expect("Expected message.");

    // Parse out the address to use.
    let client_html = cast!(&em_html.message, EditorMessageContents::ClientHtml);
    let find_str = "<iframe src=\"";
    let address_start = client_html.find(find_str).unwrap() + find_str.len();
    let address_end = client_html[address_start..].find("\"").unwrap() + address_start - 1;
    let address = &client_html[address_start..address_end];
    println!("Address: {address}");

    // Open the Client.
    driver.goto(address).await?;
    //codechat_server.send_message_current_file(url)

    // Provide a source file.
    let elem_form = driver.find(By::Id("search-form")).await?;

    // Find element from element.
    let elem_text = elem_form.find(By::Id("searchInput")).await?;

    // Type in the search terms.
    elem_text.send_keys("selenium").await?;

    // Click the search button.
    let elem_button = elem_form.find(By::Css("button[type='submit']")).await?;
    elem_button.click().await?;

    // Look for header to implicitly wait for the page to load.
    driver.query(By::ClassName("firstHeading")).first().await?;
    assert_eq!(driver.title().await?, "Selenium - Wikipedia");

    sleep(std::time::Duration::from_secs(1)).await;

    Ok(())
}
