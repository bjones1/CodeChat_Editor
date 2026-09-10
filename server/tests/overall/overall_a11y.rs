// Copyright (C) 2026 Bryan A. Jones.
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
//! `overall_a11y.rs` - audit the accessibility of the rendered Client
//! ==================================================================
//!
//! These tests drive the same headless browser as the other `overall_*`
//! modules, but instead of asserting on editing behavior they gather the raw
//! material for an accessibility review: each loads a file, lets the Client
//! render it, then runs the
//! [axe-core](https://github.com/dequelabs/axe-core) audit engine plus a
//! structural probe against every frame of the resulting page.
//!
//! Four scenarios are covered, because the Server emits materially different
//! markup for each: a doc-only (Markdown) file and a source file, each both
//! outside a project and inside one. A project adds the table-of-contents
//! `<nav>`/`<iframe>` pair and a second stylesheet, so its markup can't be
//! inferred from the standalone case.
//!
//! Each scenario writes `server/target/a11y/<scenario>.json` and logs a
//! summary. The tests deliberately assert only that the audit *ran* -- they
//! report findings rather than gate on them, since the Client currently fails
//! several checks and a gate would simply be disabled. Once the findings in
//! `docs/accessibility_review.md` are addressed, \[`assert_audit_ran`\] is the
//! place to add a violation budget so that regressions fail the build.
//!
//! The audit engine is `axe-core`, a Client dev dependency; run `pnpm install`
//! in `client/` if these tests report it missing.
// Imports
// -------
//
// ### Standard library
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

// ### Third-party
use chrono::{DateTime, Local};
use indoc::indoc;
use serde_json::{Value, json};
use thirtyfour::{
    By, Key, WebDriver, error::WebDriverError, prelude::ElementQueryable, support::sleep,
};
use tracing::{info, warn};

// ### Local
use crate::common::{CodeChatEditorServerLog, DOC_BLOCK_CSS, perform_loadfile};
use crate::make_test;
use test_utils::prep_test_dir;

// The audit engine
// ----------------
//
// Read the `axe-core` source, which is injected into each frame before the
// audit runs. It's a dev dependency of the Client rather than a file checked
// into this repo, so that its version is managed alongside the Client's other
// JavaScript libraries.
//
// The audit engine's source.
fn axe_source() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../client/node_modules/axe-core/axe.min.js");
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "Unable to read the axe-core audit engine at {}: {err}.\nRun `pnpm install` in the `client/` directory to install it.",
            path.display()
        )
    })
}

// Run the audit engine against the current frame's document, returning a
// trimmed summary. `iframes: false` confines the run to this one document:
// axe can only audit a frame it has been injected into, and \[`audit_page`\]
// walks the frames itself, so letting axe recurse would report the other
// frames as untestable instead of auditing them.
//
// The full axe result is far too large to move through the WebDriver protocol
// -- it names every passing node -- so the summary is built in the browser.
const AXE_RUN_JS: &str = r"
const done = arguments[0];
const summarize = (results) =>
    results.map((result) => ({
        id: result.id,
        impact: result.impact,
        help: result.help,
        helpUrl: result.helpUrl,
        tags: result.tags,
        nodes: result.nodes.map((node) => ({
            target: node.target,
            html: node.html.slice(0, 400),
            failureSummary: node.failureSummary,
        })),
    }));
axe.run(document, { iframes: false })
    .then((results) =>
        done(
            JSON.stringify({
                violations: summarize(results.violations),
                incomplete: summarize(results.incomplete),
                passed_rules: results.passes.map((result) => result.id),
            }),
        ),
    )
    .catch((err) => done(JSON.stringify({ error: String(err) })));
";

// Collect the structural facts a reviewer needs which axe doesn't report: the
// heading outline, the landmark regions, and every focusable control with
// whatever accessible name it exposes. axe flags a *missing* name; this probe
// shows what the names actually are, which is what reveals the page as a
// keyboard or screen reader user meets it.
const STRUCTURE_PROBE_JS: &str = r#"
// A control is reachable by Tab if it's inherently focusable or carries a
// non-negative `tabindex`, and is neither disabled nor hidden.
const FOCUSABLE = [
    "a[href]", "button", "input", "select", "textarea", "summary",
    "[contenteditable='true']", "[tabindex]",
].join(",");
const isVisible = (el) => {
    const style = getComputedStyle(el);
    return style.display !== "none" && style.visibility !== "hidden" &&
        (el.offsetWidth > 0 || el.offsetHeight > 0 ||
            el.getClientRects().length > 0);
};
// An approximation of the accessible name, covering the sources this page
// actually uses. The full algorithm is far larger; axe applies it when
// deciding whether a name exists at all.
const accessibleName = (el) => {
    const labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {
        const text = labelledBy.split(/\s+/)
            .map((id) => document.getElementById(id)?.textContent ?? "")
            .join(" ").trim();
        if (text) return text.slice(0, 80);
    }
    for (const source of [
        el.getAttribute("aria-label"), el.getAttribute("title"),
        el.getAttribute("alt"), el.tagName === "INPUT" ? el.value : null,
        el.textContent,
    ]) {
        const text = (source ?? "").replace(/\s+/g, " ").trim();
        if (text) return text.slice(0, 80);
    }
    return "";
};
const describe = (el) => ({
    tag: el.tagName.toLowerCase(),
    id: el.id || undefined,
    class: typeof el.className === "string" && el.className
        ? el.className.slice(0, 80) : undefined,
    role: el.getAttribute("role") ?? undefined,
    tabindex: el.getAttribute("tabindex") ?? undefined,
    name: accessibleName(el),
});
const LANDMARK_ROLES = [
    "main", "navigation", "banner", "contentinfo", "complementary", "region",
    "search", "form",
];
// A composite widget manages focus itself: only one item is reachable by Tab,
// and the arrow keys move between items. Its items are therefore excluded
// from `focusable` below, so report them separately -- a widget whose items
// are all `tabindex="-1"` can't be entered from the keyboard at all.
const describeWidget = (widget) => ({
    ...describe(widget),
    items: [...widget.querySelectorAll("[role='menuitem'],[role='menuitemcheckbox'],[role='menuitemradio'],[role='tab'],[role='button']")]
        .map(describe),
});
return JSON.stringify({
    title: document.title,
    lang: document.documentElement.getAttribute("lang"),
    headings: [...document.querySelectorAll("h1,h2,h3,h4,h5,h6,[role='heading']")]
        .map((el) => ({
            level: el.getAttribute("aria-level") ?? el.tagName.slice(1),
            text: (el.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 80),
        })),
    landmarks: [...document.querySelectorAll(
        "main,nav,header,footer,aside,section,form,[role]")]
        .filter((el) =>
            el.matches("main,nav,header,footer,aside,section,form") ||
            LANDMARK_ROLES.includes(el.getAttribute("role")))
        .map(describe),
    focusable: [...document.querySelectorAll(FOCUSABLE)]
        .filter((el) => !el.hasAttribute("disabled") &&
            el.getAttribute("tabindex") !== "-1" && isVisible(el))
        .map(describe),
    widgets: [...document.querySelectorAll(
        "[role='menubar'],[role='menu'],[role='toolbar'],[role='tablist']")]
        .filter(isVisible).map(describeWidget),
    iframes: [...document.querySelectorAll("iframe")].map((el) => ({
        id: el.id || undefined,
        title: el.getAttribute("title"),
    })),
});
"#;

// Describe whatever currently has focus, used by the tab-order walk below.
// The description is deliberately terse: the walk produces one of these per
// keystroke, and only enough detail to recognize the element is useful.
const ACTIVE_ELEMENT_JS: &str = r#"
const el = document.activeElement;
if (!el) return "(nothing)";
// Keep enough of the name for the run's fixture directory to survive whole:
// the summary substitutes that path out (see `normalize`), and it can't match
// a path this probe has already cut in half. The summary shortens what's left
// for display.
const name = (el.getAttribute("aria-label") ?? el.getAttribute("title") ??
    el.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 300);
return [
    el.tagName.toLowerCase(),
    el.id ? `#${el.id}` : "",
    typeof el.className === "string" && el.className
        ? `.${el.className.trim().split(/\s+/).join(".")}` : "",
    el.getAttribute("role") ? ` role=${el.getAttribute("role")}` : "",
    name ? ` "${name}"` : "",
].join("");
"#;

// Walk the tab order of the current frame, returning what each press of
// <kbd>Tab</kbd> reaches. This is the one question a static scan can't answer:
// axe reports whether a control *has* a name, while only pressing the key
// shows whether a keyboard user can get to it at all.
//
// The walk starts by blurring whatever the audit left focused, so that it
// begins at the top of the document rather than mid-page.
async fn tab_order(
    driver: &WebDriver,
    // How many times to press Tab. Focus cycles back to the browser chrome
    // once the document's last control is passed, so a handful of presses
    // beyond the number of known controls is enough.
    steps: usize,
    // What each press reached, in order.
) -> Result<Vec<String>, WebDriverError> {
    driver
        .execute("document.activeElement?.blur();", Vec::new())
        .await?;
    let mut reached = Vec::new();
    for _ in 0..steps {
        // Send the keystroke through an action chain rather than to a fetched
        // `active_element()`: tabbing into a doc block hands it to TinyMCE,
        // which replaces the block's DOM node, so any element handle held
        // across the keystroke goes stale.
        driver.action_chain().send_keys(Key::Tab).perform().await?;
        // For the same reason, let that replacement settle before reading
        // what ended up focused; otherwise the read can catch `<body>`
        // mid-swap.
        sleep(Duration::from_millis(250)).await;
        let description = driver.execute(ACTIVE_ELEMENT_JS, Vec::new()).await?;
        reached.push(
            description
                .json()
                .as_str()
                .unwrap_or("(not a string)")
                .to_string(),
        );
    }
    Ok(reached)
}

// Auditing a page
// ---------------
//
// Audit the document in the frame the driver is currently focused on.
async fn audit_current_frame(
    driver: &WebDriver,
    // A name for this frame, recorded in the report.
    frame: &str,
    // The frame's audit results and structural facts.
) -> Result<Value, WebDriverError> {
    driver.execute(axe_source(), Vec::new()).await?;
    // `axe.run` returns a promise, so the audit must run as an async script.
    // Give it more than the WebDriver default of 30 s: a full audit of the
    // Client, whose doc blocks each carry a TinyMCE editor, is not fast in a
    // debug build under CI load.
    driver.set_script_timeout(Duration::from_secs(120)).await?;
    let axe_json = driver.execute_async(AXE_RUN_JS, Vec::new()).await?;
    let structure_json = driver.execute(STRUCTURE_PROBE_JS, Vec::new()).await?;

    // Both scripts stringify their result, so that the deeply nested data
    // survives the WebDriver protocol's value conversion intact.
    let parse = |value: &Value, what: &str| -> Value {
        let text = value
            .as_str()
            .unwrap_or_else(|| panic!("The {what} probe returned {value} rather than a string."));
        serde_json::from_str(text)
            .unwrap_or_else(|err| panic!("Unable to parse the {what} probe's result: {err}."))
    };
    let axe: Value = parse(axe_json.json(), "audit");
    assert!(
        axe.get("error").is_none(),
        "The audit engine failed in frame `{frame}`: {}.",
        axe["error"]
    );

    Ok(json!({
        "frame": frame,
        "url": driver.current_url().await?.to_string(),
        "structure": parse(structure_json.json(), "structure"),
        "violations": axe["violations"],
        "incomplete": axe["incomplete"],
        "passed_rules": axe["passed_rules"],
    }))
}

// Audit every frame of the loaded page, check the audit produced usable
// results, then write the report.
//
// The Client is nested two frames deep: the Server serves a framework page
// holding `#CodeChat-iframe`, which holds the Client's page, which -- for a
// file inside a project -- itself holds the table of contents in
// `#CodeChat-sidebar`. Each is a separate document, so each needs its own
// injection of the audit engine.
#[allow(deprecated)]
async fn audit_page(
    driver: &WebDriver,
    // Names the report file and identifies the scenario within it.
    scenario: &str,
    // Whether the file under test lives in a project, which adds the
    // table-of-contents frame.
    is_project: bool,
    // The fixture copy under test, recorded so the summary can normalize its
    // random name away.
    test_dir: &Path,
) -> Result<(), WebDriverError> {
    let mut frames = Vec::new();

    driver.enter_default_frame().await?;
    frames.push(audit_current_frame(driver, "framework").await?);

    let client_iframe = driver.query(By::Css("#CodeChat-iframe")).first().await?;
    client_iframe.enter_frame().await?;
    frames.push(audit_current_frame(driver, "client").await?);

    // Audit the Client again while a doc block is being edited. This is a
    // distinct page: clicking a doc block hands it to TinyMCE, which replaces
    // the block with a `contenteditable` region and builds its menu bar into
    // `#CodeChat-menu`. That menu bar is the Client's only real chrome, so an
    // audit which skipped it would miss most of the interactive surface.
    driver
        .query(By::Css(".CodeChat-doc-contents"))
        .first()
        .await?
        .click()
        .await?;
    driver
        .query(By::Css("#CodeChat-menu [role='menubar']"))
        .first()
        .await?;
    frames.push(audit_current_frame(driver, "client-editing").await?);

    // Every item in TinyMCE's menu bar carries `tabindex="-1"`, so the tab
    // walk below can't reach it; TinyMCE offers `Alt+F9` instead. Record where
    // that shortcut lands, since a menu bar reachable only by a shortcut the
    // UI never mentions is a much smaller finding than one which can't be
    // reached at all. This runs before the tab walk because, unlike the walk,
    // it doesn't edit the document.
    //
    // Log the `keydown` events the page receives alongside the result, so that
    // "the shortcut was delivered and ignored" can be told apart from
    // "WebDriver never delivered the shortcut" -- a real hazard for modified
    // function keys, which the OS and the browser both lay claim to (see the
    // discussion of macOS modifiers in `common/mod.rs`).
    driver
        .execute(
            "window.__a11yKeys = [];
             document.addEventListener('keydown', (event) => {
                 window.__a11yKeys.push(
                     `${event.altKey ? 'Alt+' : ''}${event.key}`);
             }, true);",
            Vec::new(),
        )
        .await?;
    // Hold Alt down across the F9 press explicitly. Passing the combination to
    // `send_keys` instead delivers `Alt` and `F9` as two independent presses,
    // so the page sees a bare `F9` and the shortcut never fires.
    driver
        .action_chain()
        .key_down(Key::Alt)
        .key_down(Key::F9)
        .key_up(Key::F9)
        .key_up(Key::Alt)
        .perform()
        .await?;
    sleep(Duration::from_millis(250)).await;
    // Return the key list as an array rather than a JSON string: a string
    // would arrive already encoded, and reporting it would show the reader
    // escaped quotes instead of the keys.
    let keys_seen = driver
        .execute("return window.__a11yKeys;", Vec::new())
        .await?;
    let keys_seen: Vec<&str> = keys_seen
        .json()
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let menubar_shortcut = format!(
        "keys delivered: {}; focus: {}",
        if keys_seen.is_empty() {
            "none".to_string()
        } else {
            keys_seen.join(", ")
        },
        driver
            .execute(ACTIVE_ELEMENT_JS, Vec::new())
            .await?
            .json()
            .as_str()
            .unwrap_or("(not a string)")
    );

    if is_project {
        let sidebar_iframe = driver.query(By::Css("#CodeChat-sidebar")).first().await?;
        sidebar_iframe.enter_frame().await?;
        frames.push(audit_current_frame(driver, "toc").await?);
    }

    // Walk the tab order strictly last, back in the Client's frame. The walk
    // edits the document: a doc block is a widget inside the CodeMirror
    // editor, so Tab reaches CodeMirror and inserts an indent. That leaves the
    // file in a state the Server's translation currently panics on -- a
    // finding in its own right, and the reason nothing may run after this.
    driver.enter_default_frame().await?;
    let client_iframe = driver.query(By::Css("#CodeChat-iframe")).first().await?;
    client_iframe.enter_frame().await?;
    let tab_order = tab_order(driver, TAB_ORDER_STEPS).await?;
    driver.enter_default_frame().await?;

    assert_audit_ran(scenario, &frames);
    write_report(
        scenario,
        &json!({
            "scenario": scenario,
            // Stamped so that \[`write_summary`\] can tell a report produced by
            // this run from one left behind by an earlier one.
            "generated": Local::now().to_rfc3339(),
            // The fixture copy this run used. Its name is random, and it
            // appears inside accessible names (the Client shows the open
            // file's directory), so \[`normalize`\] substitutes it out of the
            // summary to keep that file diffable between runs.
            "test_dir": test_dir.to_string_lossy(),
            "frames": frames,
            "client_editing_tab_order": tab_order,
            "client_editing_alt_f9_reaches": menubar_shortcut,
        }),
    );
    Ok(())
}

// How far \[`tab_order`\] walks. The Client's editing state offers well under
// a dozen controls, so this comfortably reaches the end of the tab order and
// wraps around, which is what shows whether anything was skipped.
const TAB_ORDER_STEPS: usize = 12;

// Check that the audit produced usable results, then log what it found. This
// is the only assertion the module makes; see the module comment for why.
fn assert_audit_ran(scenario: &str, frames: &[Value]) {
    for frame in frames {
        let name = &frame["frame"];
        assert!(
            frame["passed_rules"]
                .as_array()
                .is_some_and(|passed| !passed.is_empty()),
            "The audit of frame {name} in scenario `{scenario}` checked no rules at all, so its (empty) findings can't be trusted."
        );
        let findings: Vec<String> = frame["violations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|violation| {
                format!(
                    "{} ({}, {} node(s))",
                    violation["id"].as_str().unwrap_or("?"),
                    violation["impact"].as_str().unwrap_or("?"),
                    violation["nodes"].as_array().map_or(0, Vec::len)
                )
            })
            .collect();
        if findings.is_empty() {
            info!("a11y {scenario}/{name}: no violations.");
        } else {
            warn!("a11y {scenario}/{name}: {}.", findings.join(", "));
        }
    }
}

// Write a scenario's report under `server/target/`, which version control
// already excludes, since the report describes one browser version on one
// platform and so isn't reproducible enough to check in.
fn write_report(scenario: &str, report: &Value) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a11y");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{scenario}.json"));
    fs::write(&path, serde_json::to_string_pretty(report).unwrap()).unwrap();
    info!("a11y report written to {}.", path.display());
    write_summary(&dir);
}

// The summary
// -----------
//
// Rebuild `summary.md` from every scenario report in `dir`. This is the
// document a person reads when updating `docs/accessibility_review.md`: it
// collects what the JSON reports say into one page, so the review's prose can
// be compared against current measurements without opening four files.
//
// It's rebuilt from what's on disk rather than accumulated in memory, because
// each scenario is a separate `#[tokio::test]` and running one test alone must
// still produce a coherent summary. The cost is that a report left behind by an
// earlier run is included too, so the freshness column below exists to make
// that visible instead of silently mixing old measurements with new ones.
fn write_summary(dir: &Path) {
    // Collect every readable report. A report which can't be parsed is skipped
    // with a warning rather than failing the test: the summary is a
    // convenience, and losing it should never mask the audit's own result.
    let mut reports: Vec<Value> = Vec::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension() != Some("json".as_ref()) {
            continue;
        }
        if let Some(report) = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            reports.push(report);
        } else {
            warn!("Skipping unreadable a11y report {}.", path.display());
        }
    }
    reports.sort_by(|left, right| scenario_of(left).cmp(scenario_of(right)));

    let mut summary = String::new();
    write_summary_header(&mut summary, &reports);
    write_summary_violations(&mut summary, &reports);
    write_summary_controls(&mut summary, &reports);
    write_summary_keyboard(&mut summary, &reports);

    let path = dir.join("summary.md");
    fs::write(&path, summary).unwrap();
    info!("a11y summary written to {}.", path.display());
}

// The scenario a report describes, used to order the summary.
fn scenario_of(report: &Value) -> &str {
    report["scenario"].as_str().unwrap_or("(unnamed)")
}

// Replace the run's fixture directory with a fixed placeholder. Each run copies
// the fixtures to a freshly named temporary directory, and that name reaches
// the summary through the accessible names the Client derives from the open
// file's path. Without this substitution every line mentioning a path would
// differ between runs, and the diff which shows what actually changed would be
// buried.
fn normalize(text: &str, report: &Value) -> String {
    match report["test_dir"].as_str() {
        Some(test_dir) => text.replace(test_dir, "<test dir>"),
        None => text.to_string(),
    }
}

// A report is stale once it's this much older than the newest report present.
// The four scenarios take well under a minute in total, so anything older than
// this came from a different run of the suite.
const STALE_AFTER: chrono::TimeDelta = chrono::TimeDelta::minutes(5);

// Write the title and the table saying when each report was measured.
fn write_summary_header(summary: &mut String, reports: &[Value]) {
    let generated = |report: &Value| {
        DateTime::parse_from_rfc3339(report["generated"].as_str().unwrap_or_default()).ok()
    };
    let newest = reports.iter().filter_map(generated).max();

    writeln!(
        summary,
        "`summary.md` -- accessibility audit results\n\
         ==========================================\n\n\
         Generated by `overall_a11y.rs`; edits here are overwritten by the next\n\
         run. See [the review](../../../docs/accessibility_review.md) for what\n\
         these results mean and what to do about them.\n\n\
         Reports\n\
         -------\n"
    )
    .unwrap();
    writeln!(summary, "| Scenario | Measured | Freshness |").unwrap();
    writeln!(summary, "|----------|----------|-----------|").unwrap();
    for report in reports {
        // A report whose timestamp is missing or unparsable predates this
        // field, which makes it old by definition.
        let (measured, freshness) = match (generated(report), newest) {
            (Some(when), Some(newest)) if newest - when <= STALE_AFTER => (
                when.format("%Y-%m-%d %H:%M:%S").to_string(),
                "current".to_string(),
            ),
            (Some(when), Some(newest)) => (
                when.format("%Y-%m-%d %H:%M:%S").to_string(),
                format!(
                    "**stale** -- {} min older; re-run to refresh",
                    (newest - when).num_minutes()
                ),
            ),
            _ => (
                "unknown".to_string(),
                "**stale** -- re-run to refresh".to_string(),
            ),
        };
        writeln!(
            summary,
            "| `{}` | {measured} | {freshness} |",
            scenario_of(report)
        )
        .unwrap();
    }
    writeln!(summary).unwrap();
}

// Write the table of audit violations across all scenarios.
fn write_summary_violations(summary: &mut String, reports: &[Value]) {
    writeln!(
        summary,
        "Violations\n\
         ----------\n"
    )
    .unwrap();

    let mut rows = String::new();
    for report in reports {
        for frame in report["frames"].as_array().into_iter().flatten() {
            for violation in frame["violations"].as_array().into_iter().flatten() {
                // Of axe's tags, only the conformance ones tell a reader
                // whether a finding is a WCAG failure or a best practice; the
                // rest (`cat.aria` and friends) just categorize the rule.
                let tags: Vec<&str> = violation["tags"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|tag| tag.starts_with("wcag") || *tag == "best-practice")
                    .collect();
                writeln!(
                    rows,
                    "| `{}` | `{}` | `{}` | {} | {} | {} |",
                    scenario_of(report),
                    frame["frame"].as_str().unwrap_or("?"),
                    violation["id"].as_str().unwrap_or("?"),
                    violation["impact"].as_str().unwrap_or("?"),
                    violation["nodes"].as_array().map_or(0, Vec::len),
                    tags.join(", ")
                )
                .unwrap();
            }
        }
    }

    if rows.is_empty() {
        writeln!(summary, "None. Every rule axe applied passed.\n").unwrap();
    } else {
        writeln!(
            summary,
            "| Scenario | Frame | Rule | Impact | Nodes | Tags |"
        )
        .unwrap();
        writeln!(
            summary,
            "|----------|-------|------|--------|-------|------|"
        )
        .unwrap();
        write!(summary, "{rows}").unwrap();
        writeln!(
            summary,
            "\nEach violation's failing elements are in the matching\n\
             `<scenario>.json`, under the frame's `violations`.\n"
        )
        .unwrap();
    }
}

// Write the inventory of focusable controls and their accessible names. axe
// reports only that a name is missing where a rule demands one; this table
// shows what every control is actually called, which is how a change to a
// control's role, name, or `tabindex` becomes visible in a diff of this file.
fn write_summary_controls(summary: &mut String, reports: &[Value]) {
    writeln!(
        summary,
        "Focusable controls\n\
         ------------------\n\n\
         What a keyboard user can reach, and the name each control exposes.\n\
         Composite widgets manage their own focus, so their items are listed\n\
         separately from the controls in the tab order.\n"
    )
    .unwrap();

    writeln!(
        summary,
        "| Scenario | Frame | Element | Role | Tabindex | Accessible name |"
    )
    .unwrap();
    writeln!(
        summary,
        "|----------|-------|---------|------|----------|-----------------|"
    )
    .unwrap();
    for report in reports {
        for frame in report["frames"].as_array().into_iter().flatten() {
            let structure = &frame["structure"];
            let widget_items = structure["widgets"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|widget| widget["items"].as_array().into_iter().flatten());
            for (control, in_widget) in structure["focusable"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|control| (control, false))
                .chain(widget_items.map(|item| (item, true)))
            {
                writeln!(
                    summary,
                    "| `{}` | `{}` | `{}`{} | {} | {} | {} |",
                    scenario_of(report),
                    frame["frame"].as_str().unwrap_or("?"),
                    describe_control(control),
                    if in_widget { " (widget item)" } else { "" },
                    optional(&control["role"]),
                    optional(&control["tabindex"]),
                    summarize_name(&control["name"], report),
                )
                .unwrap();
            }
        }
    }
    writeln!(summary).unwrap();
}

// Render a control as a CSS-like selector, the shortest form which still
// identifies it in the page.
fn describe_control(control: &Value) -> String {
    let mut description = control["tag"].as_str().unwrap_or("?").to_string();
    if let Some(id) = control["id"].as_str() {
        description.push('#');
        description.push_str(id);
    } else if let Some(classes) = control["class"].as_str() {
        // Only the first class, which is the identifying one here; the rest are
        // state (`mce-edit-focus`) or layout (`cm-lineWrapping`).
        if let Some(first) = classes.split_whitespace().next() {
            description.push('.');
            description.push_str(first);
        }
    }
    description
}

// Render a JSON string which may be absent, for a table cell.
fn optional(value: &Value) -> &str {
    value.as_str().unwrap_or("--")
}

// Render an accessible name for a table cell: escape the `|` which would
// otherwise split the cell, and shorten a name long enough to distort the
// table. The full name is in the JSON report.
fn summarize_name(name: &Value, report: &Value) -> String {
    let name = name.as_str().unwrap_or_default();
    if name.is_empty() {
        return "**(none)**".to_string();
    }
    let escaped = normalize(name, report).replace('|', "\\|");
    format!("\"{}\"", shorten(&escaped, 60))
}

// Cut `text` to at most `limit` characters, marking any cut with an ellipsis.
// The full value is always in the JSON report.
fn shorten(text: &str, limit: usize) -> String {
    let mut shortened: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        shortened.push('…');
    }
    shortened
}

// Write what the keyboard probes found, per scenario. Unlike the violations
// above, none of this is something axe reports -- it's the measured behavior
// which findings about keyboard access rest on.
fn write_summary_keyboard(summary: &mut String, reports: &[Value]) {
    writeln!(
        summary,
        "Keyboard\n\
         --------\n\n\
         Measured in the Client's frame while a doc block is being edited.\n\n\
         The walk types: `Tab` reaches CodeMirror, which inserts an indent\n\
         rather than moving focus out of the editor. Later steps therefore\n\
         show the Client reacting to an edited -- and eventually invalid --\n\
         document, so a stray toast or error among them is the walk's own\n\
         doing rather than a finding.\n"
    )
    .unwrap();

    for report in reports {
        writeln!(summary, "### `{}`\n", scenario_of(report)).unwrap();
        writeln!(
            summary,
            "`Alt+F9` -- {}\n",
            normalize(
                report["client_editing_alt_f9_reaches"]
                    .as_str()
                    .unwrap_or("(not measured)"),
                report
            )
        )
        .unwrap();
        writeln!(summary, "Tab order:\n").unwrap();
        for (step, reached) in report["client_editing_tab_order"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            // `body` means focus left this document entirely -- it's in the
            // parent page or the browser's own chrome -- rather than landing on
            // an element of the Client.
            writeln!(
                summary,
                "{:2}. {}",
                step + 1,
                shorten(&normalize(reached.as_str().unwrap_or("?"), report), 100)
            )
            .unwrap();
        }
        writeln!(summary).unwrap();
    }
}

// Scenarios
// ---------
//
// Sample content exercising the constructs a reviewer cares about: headings
// (which form the document outline), a link, a list, and a table.
const MARKDOWN_SAMPLE: &str = indoc!(
    "
    Accessibility sample
    ====================

    A paragraph containing a [link](test.md) and `inline code`.

    Second heading
    --------------

    *   First item
    *   Second item

    | Column A | Column B |
    |----------|----------|
    | 1        | 2        |
    "
);

// The same constructs, but carried in comments, so that the Client renders doc
// blocks interleaved with CodeMirror code blocks.
const CODE_SAMPLE: &str = indoc!(
    "
    # Accessibility sample
    # ====================
    #
    # A paragraph containing a [link](test.py) and `inline code`.
    def sample():
        # An indented doc block, whose indent must line up with the indent of
        # the code below it.
        return 42

    # Second heading
    # --------------
    #
    # *   First item
    # *   Second item
    more_code()
    "
);

make_test!(test_markdown_standalone, test_markdown_standalone_core);

// ### A Markdown file outside a project
async fn test_markdown_standalone_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    load_and_audit(
        &codechat_server,
        &driver,
        &test_dir,
        "test.md",
        MARKDOWN_SAMPLE,
        "markdown_standalone",
        false,
    )
    .await
}

make_test!(test_code_standalone, test_code_standalone_core);

// ### A source file outside a project
async fn test_code_standalone_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    load_and_audit(
        &codechat_server,
        &driver,
        &test_dir,
        "test.py",
        CODE_SAMPLE,
        "code_standalone",
        false,
    )
    .await
}

make_test!(test_markdown_project, test_markdown_project_core);

// ### A Markdown file inside a project
async fn test_markdown_project_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    load_and_audit(
        &codechat_server,
        &driver,
        &test_dir,
        "test.md",
        MARKDOWN_SAMPLE,
        "markdown_project",
        true,
    )
    .await
}

make_test!(test_code_project, test_code_project_core);

// ### A source file inside a project
async fn test_code_project_core(
    codechat_server: CodeChatEditorServerLog,
    driver: WebDriver,
    test_dir: PathBuf,
) -> Result<(), WebDriverError> {
    load_and_audit(
        &codechat_server,
        &driver,
        &test_dir,
        "test.py",
        CODE_SAMPLE,
        "code_project",
        true,
    )
    .await
}

// The body shared by all four scenarios: load the file, wait for the Client to
// finish rendering it, then audit.
async fn load_and_audit(
    codechat_server: &CodeChatEditorServerLog,
    driver: &WebDriver,
    test_dir: &Path,
    // The fixture file to open.
    file_name: &str,
    // The contents to supply for it, in place of what's on disk.
    contents: &str,
    // See \[`audit_page`\].
    scenario: &str,
    is_project: bool,
) -> Result<(), WebDriverError> {
    let ide_version = 0.0;
    perform_loadfile(
        codechat_server,
        test_dir,
        file_name,
        Some((contents.to_string(), ide_version)),
        is_project,
        6.0,
    )
    .await;

    // Wait for the rendered content to appear before auditing: a doc-only file
    // becomes a single doc block, while a source file gains CodeMirror
    // editors. Querying inside the Client's frame both waits for the render
    // and confirms it happened.
    let client_iframe = driver.query(By::Css("#CodeChat-iframe")).first().await?;
    #[allow(deprecated)]
    client_iframe.enter_frame().await?;
    let rendered_css = if Path::new(file_name).extension() == Some("md".as_ref()) {
        DOC_BLOCK_CSS
    } else {
        ".CodeChat-CodeMirror .cm-line"
    };
    driver.query(By::Css(rendered_css)).first().await?;

    audit_page(driver, scenario, is_project, test_dir).await
}
