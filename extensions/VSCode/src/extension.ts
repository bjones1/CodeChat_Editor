// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version of the GNU
// General Public License.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// `extension.ts` - The CodeChat Editor Visual Studio Code extension
// =================================================================
//
// This extension creates a webview, then uses a websocket connection to the
// CodeChat Editor Server and Client to render editor text in that webview.
//
// Imports
// -------
//
// ### Node.js packages
import assert from "assert";
import process from "node:process";

// ### Third-party packages
import escape from "escape-html";
import vscode, {
    Range,
    TextDocument,
    TextEditor,
    TextEditorRevealType,
} from "vscode";
import { CodeChatEditorServer, initServer } from "./index.js";

// ### Local packages
import {
    auto_update_timeout_ms,
    CaptureEventWire,
    CaptureStatus,
    EditorMessage,
    EditorMessageContents,
    KeysOfRustEnum,
    MessageResult,
    rand,
    UpdateMessageContents,
} from "../../../client/src/shared.mjs";
import {
    DEBUG_ENABLED,
    MAX_MESSAGE_LENGTH,
    console_log,
} from "../../../client/src/debug_enabled.mjs";
import { ResultErrTypes } from "../../../client/src/rust-types/ResultErrTypes.js";
import {
    captureStatusFailureClearsIdentity,
    captureTokenCanRecord,
    captureTokenClearedState,
    CaptureTokenCurrentnessSnapshot,
    CaptureTokenPolicyStatus,
    captureTokenSnapshotStillCurrent,
    captureTokenStatusForStatusFailure,
    normalizeCaptureServiceBaseUrl,
    trustedCaptureServiceBaseUrl,
} from "./capture-policy.js";

import * as crypto from "crypto";
import * as http from "http";
import * as https from "https";

// Globals
// -------
enum CodeChatEditorClientLocation {
    html,
    browser,
}

// Create a unique session ID for logging
const CAPTURE_SESSION_ID = crypto.randomUUID();

// True on Windows, false on OS X / Linux.
const is_windows = process.platform === "win32";

// These globals are truly global: only one is needed for this entire plugin.
//
// Where the webclient resides: `html` for a webview panel embedded in VSCode;
// `browser` to use an external browser.
let codechat_client_location: CodeChatEditorClientLocation =
    CodeChatEditorClientLocation.html;
// True if the subscriptions to IDE change notifications have been registered.
let subscribed = false;

// A unique instance of these variables is required for each CodeChat panel.
// However, this code doesn't have a good UI way to deal with multiple panels,
// so only one is supported at this time.
//
// The webview panel used to display the CodeChat Client
let webview_panel: vscode.WebviewPanel | undefined;
// A timer used to wait for additional events (keystrokes, etc.) before
// performing a render.
let idle_timer: NodeJS.Timeout | undefined;
// The text editor containing the current file.
let current_editor: vscode.TextEditor | undefined;
// True to ignore the next change event, which is produced by applying an
// `Update` from the Client.
let ignore_text_document_change = false;
// True to ignore the next active editor change event, since a `CurrentFile`
// message from the Client caused this change.
let ignore_active_editor_change = false;
// True to ignore the next text selection change, since updates to the cursor or
// scroll position from the Client trigged this change.
let ignore_selection_change = false;
// True to not report the next error.
let quiet_next_error = false;
// True if the editor contents have changed (are dirty) from the perspective of
// the CodeChat Editor (not if the contents are saved to disk).
let is_dirty = false;
// The version of the current file.
let version = 0.0;

// An object to start/stop the CodeChat Editor Server.
let codeChatEditorServer: CodeChatEditorServer | undefined;
// Before using `CodeChatEditorServer`, we must initialize it.
{
    const ext = vscode.extensions.getExtension(
        "CodeChat.codechat-editor-client",
    );
    assert(ext !== undefined);
    initServer(ext.extensionPath);
}

// ---
//
// CAPTURE (Dissertation instrumentation)
// --------------------------------------

// Capture uses these helpers only for documentation-like files. Source files
// classify directly as code; Markdown/RST get a finer split so prose edits count
// as documentation activity while embedded snippets count as code activity.
function markdownFenceMarker(text: string): "`" | "~" | undefined {
    // Markdown fences may be indented up to three spaces. Do not trim, since a
    // blockquoted fence (`> ````) should not toggle the outer document state.
    const match = /^(?: {0,3})(`{3,}|~{3,})/.exec(text);
    if (match === null) {
        return undefined;
    }
    return match[1].startsWith("`") ? "`" : "~";
}

function isInMarkdownCodeFence(
    doc: vscode.TextDocument,
    line: number,
): boolean {
    // The fence delimiter itself is Markdown markup, not code content.
    if (markdownFenceMarker(doc.lineAt(line).text) !== undefined) {
        return false;
    }

    let activeFence: "`" | "~" | undefined;
    for (let i = 0; i < line; i++) {
        const marker = markdownFenceMarker(doc.lineAt(i).text);
        if (marker === undefined) {
            continue;
        }
        if (activeFence === undefined) {
            activeFence = marker;
        } else if (activeFence === marker) {
            activeFence = undefined;
        }
    }
    return activeFence !== undefined;
}

function isInRstCodeBlock(doc: vscode.TextDocument, line: number): boolean {
    // Heuristic: find the most recent ".. code-block::" (or "::") and see if
    // the current line belongs to its immediately following indented region.
    // A later non-indented paragraph closes the region, so don't keep scanning
    // past it and accidentally classify later indented prose as code.
    const cur = doc.lineAt(line).text;
    if (cur.trim().length === 0 || !/^\s+/.test(cur)) {
        return false;
    }

    for (let i = line - 1; i >= 0; i--) {
        const t = doc.lineAt(i).text;
        const tt = t.trim();
        if (tt.startsWith(".. code-block::") || tt === "::") {
            return true;
        }
        if (tt.length === 0 || /^\s+/.test(t)) {
            continue;
        }
        return false;
    }
    return false;
}

function classifyAtPosition(
    doc: vscode.TextDocument,
    pos: vscode.Position,
): ActivityKind {
    // These helpers are only for documentation-like documents that may embed
    // source snippets. Plain source files skip this branch and classify as
    // code.
    if (DOC_LANG_IDS.has(doc.languageId)) {
        if (doc.languageId === "markdown") {
            return isInMarkdownCodeFence(doc, pos.line) ? "code" : "doc";
        }
        if (doc.languageId === "restructuredtext") {
            return isInRstCodeBlock(doc, pos.line) ? "code" : "doc";
        }
        // Other doc types: default to doc
        return "doc";
    }
    return "code";
}

// Event-specific payload attached to a capture event. Study metadata such as
// group, course, assignment, and condition is intentionally excluded from the
// student-facing capture settings; analysis can join those values later from a
// researcher-managed participant/date mapping.
type CaptureEventData = Record<string, unknown>;

// Event names are generated from the Rust `CaptureEventType` enum, keeping the
// extension and server in sync without re-declaring the string union here.
type CaptureEventType = CaptureEventWire["event_type"];

// Student-facing capture settings. The setup is intentionally small: students
// give local consent, toggle recording, and paste a portal-issued capture
// token. The participant UUID comes from CaptureWebService token status.
interface StudySettings {
    // True when the student wants capture enabled for the current work session.
    enabled: boolean;
    // True after the student has consented to study capture.
    consentEnabled: boolean;
    // Pseudonymous UUID returned by CaptureWebService token status.
    participantId: string;
    // True only after CaptureWebService accepts the bearer token and capture is
    // allowed for that participant/instance.
    tokenAccepted: boolean;
    // Non-secret token status used for user-facing capture feedback.
    tokenStatus: CaptureTokenUiStatus;
}

type CaptureTokenUiStatus = CaptureTokenPolicyStatus;

interface CaptureServiceStatusResponse {
    participant_id: string;
    instance_id: string;
    study_id: string;
    capture_enabled: boolean;
    participant_status: string;
    consent_status: string;
    instance_status: string;
    token_expires_at?: string | null;
    server_time: string;
    service_version: string;
}

interface CaptureTokenRuntimeState {
    tokenStatus: CaptureTokenUiStatus;
    participantId: string;
    instanceId: string;
    studyId: string;
    captureEnabled?: boolean;
    participantStatus?: string;
    consentStatus?: string;
    instanceStatus?: string;
    tokenExpiresAt?: string | null;
    serviceVersion?: string;
    lastStatusCheckAt?: string;
    lastError?: string;
}

// Derived state for local consent, session recording, and remote token status.
// This is the single source of truth for whether events may be recorded.
type CaptureSettingsState =
    | "off"
    | "paused"
    | "recording"
    | "waitingForConsent"
    | "waitingForToken"
    | "captureDisabled";

const CAPTURE_SCHEMA_VERSION = 2;
const CAPTURE_EVENT_SOURCE = "vscode_extension";
const CAPTURE_SERVICE_DEFAULT_BASE_URL =
    "https://9m2nbv2rvc.execute-api.us-east-2.amazonaws.com/dev";
const CAPTURE_TOKEN_SECRET_KEY = "CodeChatEditor.Capture.Token";
const CAPTURE_PARTICIPANT_GLOBAL_KEY =
    "CodeChatEditor.Capture.ServiceParticipantId";
const CAPTURE_INSTANCE_GLOBAL_KEY = "CodeChatEditor.Capture.InstanceId";
const CAPTURE_STUDY_GLOBAL_KEY = "CodeChatEditor.Capture.StudyId";
const CAPTURE_ENABLED_GLOBAL_KEY = "CodeChatEditor.Capture.CaptureEnabled";
const CAPTURE_TOKEN_HASH_GLOBAL_KEY = "CodeChatEditor.Capture.TokenHash";
const CAPTURE_SERVICE_BASE_GLOBAL_KEY =
    "CodeChatEditor.Capture.IdentityServiceBaseUrl";
// Audit label for the user-facing recording toggle. This is intentionally not
// a persisted setting; recording is scoped to the current VS Code window.
const CAPTURE_RECORD_AUDIT_LABEL = "RecordStudyEvents";
const DEFAULT_REFLECTION_PROMPTS = [
    "What changed in your understanding of this code?",
    "What assumption are you making, and how could you test it?",
    "What would another developer need to know before maintaining this?",
];

// Output channel used for capture diagnostics that should not interrupt normal
// editor use.
let capture_output_channel: vscode.OutputChannel | undefined;
// Extension context provides SecretStorage and global non-secret token metadata.
let extension_context: vscode.ExtensionContext | undefined;
let captureTokenRuntimeState: CaptureTokenRuntimeState = {
    tokenStatus: "missing",
    participantId: "",
    instanceId: "",
    studyId: "",
};
// True after the first failed send is logged to the console, suppressing repeat
// console warnings while still writing detailed failures to the output channel.
let captureFailureLogged = false;
// True once the CodeChat Client and Server have completed enough startup
// handshake work for capture events to be accepted.
let captureTransportReady = false;
// True after a capture-enabled extension session has emitted `session_start`.
let extensionCaptureSessionStarted = false;
// Recording is intentionally scoped to this VS Code extension host session.
// Consent persists in settings, but recording must be
// re-enabled after VS Code restarts and can be toggled independently in each
// open VS Code window.
let sessionRecordStudyEvents = false;
// Monotonic per-extension event sequence number used to order events produced
// by this VS Code session.
let captureSequenceNumber = 0;
// Status bar item that reports capture health and opens the capture controls.
let capture_status_bar_item: vscode.StatusBarItem | undefined;
// Timer used to refresh capture status from the running server.
let capture_status_timer: NodeJS.Timeout | undefined;
// Last capture settings snapshot used to audit user-visible setting changes
// without double-logging when a command and VS Code's configuration event both
// observe the same transition.
let lastCaptureSettings: StudySettings | undefined;
// Token mutations are authoritative. They cancel refresh/status work, but
// refreshes must never cancel an enter or clear mutation.
let captureTokenMutationGeneration = 0;
let captureTokenRefreshGeneration = 0;
// Serialize token work so SecretStorage, persisted identity, and runtime state
// writes happen in a predictable order. Clear still invalidates immediately so
// it can stop Rust uploads ASAP.
let captureTokenOperationQueue: Promise<void> = Promise.resolve();

// Simple classification of what the user is currently doing. `doc` means
// prose/documentation activity, whether in a Markdown/RST document or a
// CodeChat doc block; write events from the server provide the more precise
// doc-block classification when it is available.
type ActivityKind = "doc" | "code" | "other";

// Language IDs that we treat as "documentation" for the dissertation metrics.
// You can refine this later if you want.
const DOC_LANG_IDS = new Set<string>([
    "markdown",
    "plaintext",
    "latex",
    "restructuredtext",
]);

// Track the last activity kind and when a reflective-writing (doc) session
// started.
let lastActivityKind: ActivityKind = "other";
let docSessionStart: number | null = null;
// Activity events can be generated by synchronous VS Code callbacks. Serialize
// their async capture sends so doc-session rows stay in causal order.
let captureActivityQueue: Promise<void> = Promise.resolve();

function optionalString(value: unknown): string | undefined {
    return typeof value === "string" && value.trim().length > 0
        ? value.trim()
        : undefined;
}

function captureServiceBaseUrl(): string {
    const inspected = vscode.workspace
        .getConfiguration("CodeChatEditor.Capture")
        .inspect<string>("ServiceBaseUrl");
    // This endpoint receives bearer tokens. Honor only application/user-level
    // settings so a repository cannot redirect a user's stored token.
    return trustedCaptureServiceBaseUrl(
        inspected,
        CAPTURE_SERVICE_DEFAULT_BASE_URL,
    );
}

function captureTokenStatusLabel(status: CaptureTokenUiStatus): string {
    switch (status) {
        case "missing":
            return "Missing";
        case "unverified":
            return "Stored, not verified";
        case "accepted":
            return "Accepted";
        case "rejected":
            return "Rejected";
        case "capture_disabled":
            return "Capture disabled";
        case "service_unavailable":
            return "Service unavailable";
    }
}

function loadStudySettings(): StudySettings {
    const config = vscode.workspace.getConfiguration("CodeChatEditor.Capture");
    const participantId = captureTokenRuntimeState.participantId;
    const tokenAccepted = captureTokenCanRecord(
        participantId,
        captureTokenRuntimeState.captureEnabled,
        captureTokenRuntimeState.tokenStatus,
    );
    return {
        // Recording is session-local so capture starts paused in every VS Code
        // window/restart. Consent persists in settings; token identity comes
        // from CaptureWebService status.
        enabled: sessionRecordStudyEvents,
        consentEnabled: config.get<boolean>("ConsentEnabled", false),
        participantId,
        tokenAccepted,
        tokenStatus: captureTokenRuntimeState.tokenStatus,
    };
}

// Convert raw settings into the explicit four-row state table. Keeping this as
// a separate helper prevents callers from inventing their own partial rules.
function captureSettingsState(settings: StudySettings): CaptureSettingsState {
    if (
        settings.tokenStatus === "capture_disabled" &&
        settings.consentEnabled &&
        settings.enabled
    ) {
        return "captureDisabled";
    }
    if (
        settings.consentEnabled &&
        settings.enabled &&
        !settings.tokenAccepted
    ) {
        return "waitingForToken";
    }
    if (settings.consentEnabled && settings.enabled) {
        return "recording";
    }
    if (settings.consentEnabled) {
        return "paused";
    }
    if (settings.enabled) {
        return "waitingForConsent";
    }
    return "off";
}

// Compare complete settings snapshots so command-triggered changes and VS Code
// configuration notifications do not emit duplicate audit rows.
function captureSettingsEqual(a: StudySettings, b: StudySettings): boolean {
    return (
        a.enabled === b.enabled &&
        a.consentEnabled === b.consentEnabled &&
        a.participantId === b.participantId &&
        a.tokenAccepted === b.tokenAccepted &&
        a.tokenStatus === b.tokenStatus
    );
}

// Human-readable labels used in status-bar tooltips and QuickPick details.
function captureStateDescription(state: CaptureSettingsState): string {
    switch (state) {
        case "recording":
            return "Capture records study events.";
        case "paused":
            return "Consent is retained, but recording is paused.";
        case "waitingForConsent":
            return "Capture waits for consent before recording.";
        case "waitingForToken":
            return "Capture waits for a valid portal token before recording.";
        case "captureDisabled":
            return "Capture is disabled by the portal or capture service.";
        case "off":
            return "Capture is off.";
    }
}

// Build the status bar text and tooltip from the same state table used for
// gating events. This keeps UI feedback and recording behavior aligned.
function captureSettingsStatus(settings: StudySettings): {
    label: string;
    tooltip: string;
    state: CaptureSettingsState;
} {
    const state = captureSettingsState(settings);
    let label: string;
    switch (state) {
        case "recording":
            label = "Capture: Recording";
            break;
        case "paused":
            label = "Capture: Paused";
            break;
        case "waitingForConsent":
            label = "Capture: Waiting for consent";
            break;
        case "waitingForToken":
            label = "Capture: Waiting for token";
            break;
        case "captureDisabled":
            label = "Capture: Disabled by portal";
            break;
        case "off":
            label = "Capture: Off";
            break;
    }

    return {
        label,
        state,
        tooltip: [
            `Consent Enabled: ${settings.consentEnabled ? "On" : "Off"}`,
            `Record Study Events: ${settings.enabled ? "On" : "Off"}`,
            `Token: ${captureTokenStatusLabel(settings.tokenStatus)}`,
            settings.participantId
                ? `Participant ID: ${settings.participantId}`
                : "Participant ID: unavailable",
            captureTokenRuntimeState.instanceId
                ? `Instance ID: ${captureTokenRuntimeState.instanceId}`
                : "",
            captureTokenRuntimeState.lastError
                ? `Capture Service: ${captureTokenRuntimeState.lastError}`
                : "",
            `State: ${captureStateDescription(state)}`,
        ]
            .filter((line) => line.length > 0)
            .join("\n"),
    };
}

// Normal capture events are allowed only in the `recording` row. Audit and
// control events can bypass this through explicit send options.
function captureDisabledReason(settings: StudySettings): string | undefined {
    const state = captureSettingsState(settings);
    if (state !== "recording") {
        return captureStateDescription(state);
    }
    return undefined;
}

async function updateCaptureSetting(
    name: string,
    value: string | boolean,
): Promise<void> {
    const config = vscode.workspace.getConfiguration("CodeChatEditor.Capture");
    await config.update(name, value, vscode.ConfigurationTarget.Global);
}

function getExtensionContext(): vscode.ExtensionContext {
    if (extension_context === undefined) {
        throw new Error("CodeChat extension context is not initialized.");
    }
    return extension_context;
}

async function getStoredCaptureToken(): Promise<string | undefined> {
    return optionalString(
        await getExtensionContext().secrets.get(CAPTURE_TOKEN_SECRET_KEY),
    );
}

async function storeCaptureToken(token: string): Promise<void> {
    await getExtensionContext().secrets.store(CAPTURE_TOKEN_SECRET_KEY, token);
}

async function deleteStoredCaptureToken(): Promise<void> {
    await getExtensionContext().secrets.delete(CAPTURE_TOKEN_SECRET_KEY);
}

function loadPersistedCaptureIdentity(context: vscode.ExtensionContext): void {
    const storedBaseUrl = optionalString(
        context.globalState.get(CAPTURE_SERVICE_BASE_GLOBAL_KEY),
    );
    let currentBaseUrl: string | undefined;
    try {
        currentBaseUrl = normalizeCaptureServiceBaseUrl(
            captureServiceBaseUrl(),
        );
    } catch {
        currentBaseUrl = undefined;
    }
    if (storedBaseUrl === undefined || storedBaseUrl !== currentBaseUrl) {
        captureTokenRuntimeState = {
            ...captureTokenRuntimeState,
            participantId: "",
            instanceId: "",
            studyId: "",
            captureEnabled: false,
        };
        return;
    }
    const captureEnabled = context.globalState.get<boolean>(
        CAPTURE_ENABLED_GLOBAL_KEY,
    );
    captureTokenRuntimeState = {
        ...captureTokenRuntimeState,
        participantId:
            optionalString(
                context.globalState.get(CAPTURE_PARTICIPANT_GLOBAL_KEY),
            ) ?? "",
        instanceId:
            optionalString(
                context.globalState.get(CAPTURE_INSTANCE_GLOBAL_KEY),
            ) ?? "",
        studyId:
            optionalString(context.globalState.get(CAPTURE_STUDY_GLOBAL_KEY)) ??
            "",
        captureEnabled:
            typeof captureEnabled === "boolean" ? captureEnabled : undefined,
    };
}

async function persistCaptureIdentity(
    status: CaptureServiceStatusResponse,
    tokenHash: string,
    serviceBaseUrl: string,
): Promise<void> {
    const context = getExtensionContext();
    await context.globalState.update(
        CAPTURE_PARTICIPANT_GLOBAL_KEY,
        status.participant_id,
    );
    await context.globalState.update(
        CAPTURE_INSTANCE_GLOBAL_KEY,
        status.instance_id,
    );
    await context.globalState.update(CAPTURE_STUDY_GLOBAL_KEY, status.study_id);
    await context.globalState.update(
        CAPTURE_ENABLED_GLOBAL_KEY,
        status.capture_enabled,
    );
    await context.globalState.update(CAPTURE_TOKEN_HASH_GLOBAL_KEY, tokenHash);
    await context.globalState.update(
        CAPTURE_SERVICE_BASE_GLOBAL_KEY,
        serviceBaseUrl,
    );
}

async function clearPersistedCaptureIdentity(): Promise<void> {
    const context = getExtensionContext();
    await context.globalState.update(CAPTURE_PARTICIPANT_GLOBAL_KEY, undefined);
    await context.globalState.update(CAPTURE_INSTANCE_GLOBAL_KEY, undefined);
    await context.globalState.update(CAPTURE_STUDY_GLOBAL_KEY, undefined);
    await context.globalState.update(CAPTURE_ENABLED_GLOBAL_KEY, undefined);
    await context.globalState.update(CAPTURE_TOKEN_HASH_GLOBAL_KEY, undefined);
    await context.globalState.update(
        CAPTURE_SERVICE_BASE_GLOBAL_KEY,
        undefined,
    );
}

function hashText(value: string): string {
    return crypto.createHash("sha256").update(value).digest("hex");
}

function buildFileFields(
    filePath: string | undefined,
): Pick<CaptureEventWire, "file_path" | "language_id"> {
    if (filePath === undefined) {
        return {
            language_id: vscode.window.activeTextEditor?.document.languageId,
        };
    }
    const document = get_document(filePath);
    return {
        // Send the path only to the local Rust server so it can apply the same
        // privacy-preserving hash rule used by server-generated capture events.
        file_path: filePath,
        language_id: document?.languageId,
    };
}

function captureLog(message: string): void {
    capture_output_channel?.appendLine(
        `${new Date().toISOString()} ${message}`,
    );
}

function capturePayloadSummary(payload: CaptureEventWire): string {
    return [
        `type=${payload.event_type}`,
        `event_id=${payload.event_id}`,
        `sequence=${payload.sequence_number?.toString()}`,
        `schema=${payload.schema_version}`,
        `user_id=${payload.user_id}`,
        `session_id=${payload.session_id}`,
        `source=${payload.event_source}`,
        `language=${payload.language_id ?? ""}`,
        payload.file_path ? "file_path=present" : "",
        payload.file_hash ? `file_hash=${payload.file_hash}` : "",
    ]
        .filter((part) => part.length > 0)
        .join(" ");
}

function captureStatusSummary(status: CaptureStatus): string {
    return [
        `state=${status.state}`,
        `token=${status.token_status}`,
        `enabled=${status.enabled}`,
        `queued=${status.queued_events}`,
        `spooled=${status.spooled_events}`,
        `uploaded=${status.uploaded_events}`,
        `quarantined=${status.quarantined_events}`,
        `failed=${status.failed_events}`,
        status.participant_id ? `participant_id=${status.participant_id}` : "",
        status.instance_id ? `instance_id=${status.instance_id}` : "",
        status.capture_enabled !== null
            ? `capture_enabled=${status.capture_enabled}`
            : "",
        status.service_version
            ? `service_version=${status.service_version}`
            : "",
        status.last_error ? `last_error=${status.last_error}` : "",
        status.spool_path ? `spool_path=${status.spool_path}` : "",
    ]
        .filter((part) => part.length > 0)
        .join(" ");
}

type CaptureStatusJson = Omit<
    CaptureStatus,
    | "queued_events"
    | "spooled_events"
    | "uploaded_events"
    | "failed_events"
    | "quarantined_events"
> & {
    queued_events: number;
    spooled_events: number;
    uploaded_events: number;
    failed_events: number;
    quarantined_events: number;
};

function parseCaptureStatus(json: string): CaptureStatus {
    const status = JSON.parse(json) as CaptureStatusJson;
    // Rust exports these counters as u64, which ts-rs maps to bigint. JSON
    // carries them as numbers, so convert them immediately after parsing to
    // keep the runtime value aligned with the generated TypeScript type.
    return {
        ...status,
        queued_events: BigInt(status.queued_events),
        spooled_events: BigInt(status.spooled_events),
        uploaded_events: BigInt(status.uploaded_events),
        failed_events: BigInt(status.failed_events),
        quarantined_events: BigInt(status.quarantined_events),
    };
}

function captureStatusUrl(serviceBaseUrl: string): string {
    return `${serviceBaseUrl}/v1/capture/status`;
}

function leadingHttpStatusCode(message: string): number | undefined {
    const match = /^(\d{3})(?:\s|$)/.exec(message);
    return match === null ? undefined : Number.parseInt(match[1], 10);
}

function requestCaptureStatus(
    token: string,
    serviceBaseUrl: string,
): Promise<CaptureServiceStatusResponse> {
    const url = new URL(captureStatusUrl(serviceBaseUrl));
    const client = url.protocol === "http:" ? http : https;

    return new Promise((resolve, reject) => {
        const req = client.request(
            url,
            {
                method: "GET",
                headers: {
                    Accept: "application/json",
                    Authorization: `Bearer ${token}`,
                },
            },
            (res) => {
                const chunks: Buffer[] = [];
                res.on("data", (chunk: Buffer) => chunks.push(chunk));
                res.on("end", () => {
                    const body = Buffer.concat(chunks).toString("utf8");
                    const statusCode = res.statusCode ?? 0;
                    if (statusCode < 200 || statusCode >= 300) {
                        reject(
                            new Error(
                                `${statusCode} ${
                                    body ||
                                    res.statusMessage ||
                                    "request failed"
                                }`,
                            ),
                        );
                        return;
                    }
                    try {
                        resolve(
                            JSON.parse(body) as CaptureServiceStatusResponse,
                        );
                    } catch (err) {
                        reject(
                            new Error(
                                `Unable to parse capture status response: ${
                                    err instanceof Error
                                        ? err.message
                                        : String(err)
                                }`,
                            ),
                        );
                    }
                });
            },
        );
        req.setTimeout(10000, () => {
            req.destroy(new Error("capture status request timed out"));
        });
        req.on("error", reject);
        req.end();
    });
}

async function applyCaptureServiceStatus(
    status: CaptureServiceStatusResponse,
    tokenHash: string,
    serviceBaseUrl: string,
): Promise<void> {
    captureTokenRuntimeState = {
        tokenStatus: status.capture_enabled ? "accepted" : "capture_disabled",
        participantId: status.participant_id,
        instanceId: status.instance_id,
        studyId: status.study_id,
        captureEnabled: status.capture_enabled,
        participantStatus: status.participant_status,
        consentStatus: status.consent_status,
        instanceStatus: status.instance_status,
        tokenExpiresAt: status.token_expires_at,
        serviceVersion: status.service_version,
        lastStatusCheckAt: new Date().toISOString(),
        lastError: status.capture_enabled
            ? undefined
            : "Capture is disabled by the portal or capture service.",
    };
    await persistCaptureIdentity(status, tokenHash, serviceBaseUrl);
}

async function configureRustCaptureService(
    token: string | undefined,
    serviceBaseUrl?: string,
): Promise<boolean> {
    if (codeChatEditorServer === undefined) {
        return true;
    }
    try {
        if (token === undefined) {
            codeChatEditorServer.clearCaptureToken();
            return true;
        }
        if (serviceBaseUrl === undefined) {
            throw new Error("Capture service URL is not configured.");
        }
        codeChatEditorServer.configureCaptureService(serviceBaseUrl, token);
        return true;
    } catch (err) {
        reportCaptureFailure(
            `capture service configuration failed: ${
                err instanceof Error ? err.message : String(err)
            }`,
        );
        return false;
    }
}

type CaptureServiceConfigurationResult = "configured" | "stale" | "failed";

function beginCaptureTokenMutation(): CaptureTokenCurrentnessSnapshot {
    captureTokenMutationGeneration += 1;
    captureTokenRefreshGeneration += 1;
    return {
        mutationGeneration: captureTokenMutationGeneration,
    };
}

function beginCaptureTokenRefresh(): CaptureTokenCurrentnessSnapshot {
    captureTokenRefreshGeneration += 1;
    return {
        mutationGeneration: captureTokenMutationGeneration,
        refreshGeneration: captureTokenRefreshGeneration,
    };
}

function captureTokenOperationSnapshotIsCurrent(
    snapshot: CaptureTokenCurrentnessSnapshot,
): boolean {
    return captureTokenSnapshotStillCurrent(
        snapshot,
        captureTokenMutationGeneration,
        captureTokenRefreshGeneration,
    );
}

function enqueueCaptureTokenOperation(
    operation: () => Promise<void>,
): Promise<void> {
    const task = captureTokenOperationQueue
        .catch(() => undefined)
        .then(operation);
    captureTokenOperationQueue = task.catch(() => undefined);
    return task;
}

async function captureTokenOperationStillCurrent(
    snapshot: CaptureTokenCurrentnessSnapshot,
    expectedTokenHash?: string,
    expectedBaseUrl?: string,
): Promise<boolean> {
    if (!captureTokenOperationSnapshotIsCurrent(snapshot)) {
        return false;
    }
    let storedTokenHash: string | undefined;
    if (expectedTokenHash !== undefined) {
        const token = await getStoredCaptureToken();
        storedTokenHash = token === undefined ? undefined : hashText(token);
    }
    let currentBaseUrl: string | undefined;
    if (expectedBaseUrl !== undefined) {
        try {
            currentBaseUrl = normalizeCaptureServiceBaseUrl(
                captureServiceBaseUrl(),
            );
        } catch {
            currentBaseUrl = undefined;
        }
    }
    return captureTokenSnapshotStillCurrent(
        snapshot,
        captureTokenMutationGeneration,
        captureTokenRefreshGeneration,
        storedTokenHash,
        expectedTokenHash,
        currentBaseUrl,
        expectedBaseUrl,
    );
}

async function configureRustCaptureServiceIfCurrent(
    snapshot: CaptureTokenCurrentnessSnapshot,
    token: string | undefined,
    serviceBaseUrl?: string,
    expectedTokenHash?: string,
    expectedBaseUrl?: string,
): Promise<CaptureServiceConfigurationResult> {
    if (
        !(await captureTokenOperationStillCurrent(
            snapshot,
            expectedTokenHash,
            expectedBaseUrl,
        ))
    ) {
        return "stale";
    }
    if (!captureTokenOperationSnapshotIsCurrent(snapshot)) {
        return "stale";
    }
    return (await configureRustCaptureService(token, serviceBaseUrl))
        ? "configured"
        : "failed";
}

async function refreshCaptureTokenState(
    notify: boolean = false,
): Promise<void> {
    const operationSnapshot = beginCaptureTokenRefresh();
    await enqueueCaptureTokenOperation(() =>
        refreshCaptureTokenStateInner(operationSnapshot, notify),
    );
}

async function refreshCaptureTokenStateInner(
    operationSnapshot: CaptureTokenCurrentnessSnapshot,
    notify: boolean,
): Promise<void> {
    const token = await getStoredCaptureToken();
    if (!(await captureTokenOperationStillCurrent(operationSnapshot))) {
        return;
    }

    if (token === undefined) {
        const configurationResult = await configureRustCaptureServiceIfCurrent(
            operationSnapshot,
            undefined,
        );
        if (configurationResult === "stale") {
            return;
        }
        captureTokenRuntimeState = captureTokenClearedState();
        await clearPersistedCaptureIdentity();
        await refreshCaptureStatus();
        return;
    }

    let expectedBaseUrl: string;
    try {
        expectedBaseUrl = normalizeCaptureServiceBaseUrl(
            captureServiceBaseUrl(),
        );
    } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        const configurationResult = await configureRustCaptureServiceIfCurrent(
            operationSnapshot,
            undefined,
        );
        if (configurationResult === "stale") {
            return;
        }
        captureTokenRuntimeState = {
            ...captureTokenRuntimeState,
            tokenStatus: "service_unavailable",
            captureEnabled: false,
            lastError: message,
        };
        captureLog(`capture token status failed: ${message}`);
        if (notify) {
            vscode.window.showWarningMessage(
                `CodeChat capture token could not be verified: ${message}`,
            );
        }
        await refreshCaptureStatus();
        return;
    }

    const expectedTokenHash = hashText(token);
    const configurationResult = await configureRustCaptureServiceIfCurrent(
        operationSnapshot,
        token,
        expectedBaseUrl,
        expectedTokenHash,
        expectedBaseUrl,
    );
    if (configurationResult === "stale") {
        return;
    }
    if (configurationResult === "failed") {
        captureTokenRuntimeState = {
            ...captureTokenRuntimeState,
            tokenStatus: "service_unavailable",
            captureEnabled: false,
            lastError: "Capture service configuration failed.",
        };
        await refreshCaptureStatus();
        return;
    }

    const context = getExtensionContext();
    const storedTokenHash = optionalString(
        context.globalState.get(CAPTURE_TOKEN_HASH_GLOBAL_KEY),
    );
    const storedBaseUrl = optionalString(
        context.globalState.get(CAPTURE_SERVICE_BASE_GLOBAL_KEY),
    );
    if (
        !(await captureTokenOperationStillCurrent(
            operationSnapshot,
            expectedTokenHash,
            expectedBaseUrl,
        ))
    ) {
        return;
    }
    if (
        storedTokenHash !== expectedTokenHash ||
        storedBaseUrl !== expectedBaseUrl
    ) {
        await clearPersistedCaptureIdentity();
        if (
            !(await captureTokenOperationStillCurrent(
                operationSnapshot,
                expectedTokenHash,
                expectedBaseUrl,
            ))
        ) {
            return;
        }
        captureTokenRuntimeState = {
            ...captureTokenRuntimeState,
            participantId: "",
            instanceId: "",
            studyId: "",
            captureEnabled: false,
        };
    }

    captureTokenRuntimeState = {
        ...captureTokenRuntimeState,
        tokenStatus: "unverified",
        lastError: undefined,
    };

    try {
        const status = await requestCaptureStatus(token, expectedBaseUrl);
        if (
            !(await captureTokenOperationStillCurrent(
                operationSnapshot,
                expectedTokenHash,
                expectedBaseUrl,
            ))
        ) {
            return;
        }
        await applyCaptureServiceStatus(
            status,
            expectedTokenHash,
            expectedBaseUrl,
        );
        captureLog(
            `capture token status: ${captureTokenStatusLabel(
                captureTokenRuntimeState.tokenStatus,
            )} participant_id=${status.participant_id} instance_id=${status.instance_id}`,
        );
        if (notify) {
            vscode.window.showInformationMessage(
                status.capture_enabled
                    ? "CodeChat capture token accepted."
                    : "CodeChat capture is disabled by the portal.",
            );
        }
    } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        const statusCode = leadingHttpStatusCode(message);
        if (
            !(await captureTokenOperationStillCurrent(
                operationSnapshot,
                expectedTokenHash,
                expectedBaseUrl,
            ))
        ) {
            return;
        }
        const clearIdentity = captureStatusFailureClearsIdentity(statusCode);
        if (clearIdentity) {
            await clearPersistedCaptureIdentity();
            if (
                !(await captureTokenOperationStillCurrent(
                    operationSnapshot,
                    expectedTokenHash,
                    expectedBaseUrl,
                ))
            ) {
                return;
            }
        }
        const tokenStatus = captureTokenStatusForStatusFailure(statusCode);
        captureTokenRuntimeState = {
            ...(clearIdentity
                ? {
                      participantId: "",
                      instanceId: "",
                      studyId: "",
                      captureEnabled: false,
                  }
                : captureTokenRuntimeState),
            tokenStatus,
            lastError: message,
        };
        captureLog(`capture token status failed: ${message}`);
        if (notify) {
            vscode.window.showWarningMessage(
                `CodeChat capture token could not be verified: ${message}`,
            );
        }
    }

    await refreshCaptureStatus();
}

interface CaptureSendOptions {
    // Permit audit/control events even when normal capture is paused or waiting
    // for consent.
    ignoreCaptureSettings?: boolean;
    // Update server-side capture state without spooling this event for upload.
    controlOnly?: boolean;
    // Explicit active flag carried to the server so it can enable/disable
    // translation-generated write events.
    captureActive?: boolean;
    // Audit rows for consent being turned off still need the participant ID
    // that existed before the setting changed.
    userId?: string;
}

// Helper to send a capture event to the Rust server.
async function sendCaptureEvent(
    eventType: CaptureEventType,
    filePath?: string,
    data: CaptureEventData = {},
    options: CaptureSendOptions = {},
): Promise<void> {
    const settings = loadStudySettings();
    const disabledReason = captureDisabledReason(settings);
    // User activity events stop here unless both consent and recording are on.
    if (!options.ignoreCaptureSettings && disabledReason !== undefined) {
        captureLog(`capture skipped: ${eventType} (${disabledReason})`);
        const status = captureSettingsStatus(settings);
        updateCaptureStatusBar(status.label, status.tooltip);
        return;
    }
    // Control-only messages may run after consent is off, so they must not
    // generate a fresh participant ID.
    const participantId = options.userId
        ? options.userId
        : options.controlOnly
          ? settings.participantId || "capture_control"
          : settings.participantId;
    if (!options.controlOnly && participantId.length === 0) {
        captureLog(`capture skipped: ${eventType} (no verified capture token)`);
        updateCaptureStatusBar(
            "Capture: Waiting for token",
            "Enter and verify a portal-issued capture token before recording.",
        );
        return;
    }
    const fileFields = buildFileFields(filePath);
    // The server uses `capture_active` to decide whether it may generate
    // classified write_doc/write_code rows from translated edits.
    const captureActive =
        options.captureActive ??
        (eventType !== "session_end" &&
            captureSettingsState(settings) === "recording");
    const payload: CaptureEventWire = {
        event_id: crypto.randomUUID(),
        sequence_number: BigInt(++captureSequenceNumber),
        schema_version: CAPTURE_SCHEMA_VERSION,
        user_id: participantId,
        session_id: CAPTURE_SESSION_ID,
        event_source: CAPTURE_EVENT_SOURCE,
        ...fileFields,
        event_type: eventType,
        client_tz_offset_min: new Date().getTimezoneOffset(),
        data: {
            ...data,
            capture_active: captureActive,
            // A control-only event updates the server's capture context but is
            // intentionally not inserted into capture storage.
            ...(options.controlOnly ? { capture_control_only: true } : {}),
        },
    };

    if (codeChatEditorServer === undefined) {
        captureLog(
            `capture skipped: ${capturePayloadSummary(payload)} (server not running)`,
        );
        reportCaptureFailure("CodeChat server is not running");
        return;
    }
    if (!captureTransportReady) {
        captureLog(
            `capture skipped before server handshake: ${capturePayloadSummary(payload)}`,
        );
        return;
    }

    try {
        const messageId = await codeChatEditorServer.sendCaptureEvent(
            stringifyCapturePayload(payload),
        );
        captureFailureLogged = false;
        captureLog(
            `${options.controlOnly ? "capture control queued" : "capture queued"} message_id=${messageId}: ${capturePayloadSummary(payload)}`,
        );
        await refreshCaptureStatus();
    } catch (err) {
        reportCaptureFailure(err instanceof Error ? err.message : String(err));
    }
}

function stringifyCapturePayload(payload: CaptureEventWire): string {
    return JSON.stringify(payload, (_key, value) =>
        typeof value === "bigint" ? Number(value) : value,
    );
}

function reportCaptureFailure(message: string) {
    captureLog(`capture send failed: ${message}`);
    updateCaptureStatusBar("Capture: Error", message);
    if (captureFailureLogged) {
        return;
    }
    captureFailureLogged = true;
    console.warn(`CodeChat capture event was not queued: ${message}`);
}

function updateCaptureStatusBar(text: string, tooltip?: string) {
    if (capture_status_bar_item === undefined) {
        return;
    }
    capture_status_bar_item.text = text;
    capture_status_bar_item.tooltip = tooltip;
    capture_status_bar_item.show();
}

async function refreshCaptureStatus(): Promise<void> {
    const settings = loadStudySettings();
    const settingsStatus = captureSettingsStatus(settings);
    // When the settings are not in the recording row, the settings state is the
    // authoritative status regardless of the server upload worker state.
    if (settingsStatus.state !== "recording") {
        updateCaptureStatusBar(settingsStatus.label, settingsStatus.tooltip);
        return;
    }
    if (codeChatEditorServer === undefined) {
        updateCaptureStatusBar(
            "Capture: Waiting",
            `${settingsStatus.tooltip}\nServer: CodeChat server is not running`,
        );
        return;
    }

    try {
        const status = parseCaptureStatus(
            codeChatEditorServer.getCaptureStatus(),
        );
        let label: string;
        switch (status.state) {
            case "remote":
                label = "Capture: Remote";
                break;
            case "spooling":
                label = "Capture: Queued";
                break;
            case "uploading":
                label = "Capture: Uploading";
                break;
            case "starting":
                label = "Capture: Starting";
                break;
            case "auth_failed":
                label = "Capture: Token rejected";
                break;
            case "capture_disabled":
                label = "Capture: Disabled by portal";
                break;
            case "service_unavailable":
                label = "Capture: Service unavailable";
                break;
            default:
                label = "Capture: Off";
                break;
        }
        updateCaptureStatusBar(
            label,
            [
                settingsStatus.tooltip,
                captureStatusSummary(status).split(" ").join("\n"),
            ].join("\n"),
        );
    } catch (err) {
        updateCaptureStatusBar(
            "Capture: Error",
            err instanceof Error ? err.message : String(err),
        );
    }
}

// A status-bar QuickPick action. Each item owns the async work needed after the
// student chooses it, keeping the capture UI small and easy to scan.
interface CaptureStatusAction extends vscode.QuickPickItem {
    run: () => Promise<void>;
}

function captureStatusDetails(): string {
    const tooltip = capture_status_bar_item?.tooltip;
    return typeof tooltip === "string"
        ? tooltip
        : (tooltip?.value ?? "Capture status unavailable");
}

async function setRecordStudyEvents(enabled: boolean): Promise<void> {
    // Save the previous settings before updating so the audit event can record
    // exactly what changed.
    const previousSettings = loadStudySettings();
    sessionRecordStudyEvents = enabled;
    await reconcileCaptureSettings(
        "manage_capture_record_study_events",
        previousSettings,
    );

    const updatedSettings = loadStudySettings();
    if (enabled && captureSettingsState(updatedSettings) === "recording") {
        vscode.window.showInformationMessage(
            "CodeChat capture is recording study events.",
        );
    } else if (enabled) {
        vscode.window.showInformationMessage(
            captureStateDescription(captureSettingsState(updatedSettings)),
        );
    } else {
        vscode.window.showInformationMessage(
            "CodeChat capture recording is paused.",
        );
    }
}

async function setCaptureConsent(enabled: boolean): Promise<void> {
    // Save the previous settings before updating so the audit event can record
    // consent transitions, including consent being turned off.
    const previousSettings = loadStudySettings();

    await updateCaptureSetting("ConsentEnabled", enabled);
    await reconcileCaptureSettings(
        "manage_capture_consent_enabled",
        previousSettings,
    );

    const updatedSettings = loadStudySettings();
    if (enabled && captureSettingsState(updatedSettings) === "recording") {
        vscode.window.showInformationMessage(
            "CodeChat capture consent is recorded and recording is on.",
        );
    } else if (enabled) {
        vscode.window.showInformationMessage(
            "CodeChat capture consent is recorded.",
        );
    } else {
        vscode.window.showInformationMessage(
            "CodeChat capture consent is off.",
        );
    }
}

async function giveConsentAndRecordStudyEvents(): Promise<void> {
    // This command intentionally changes both user-facing settings together,
    // then lets the common reconcile path emit one combined audit event.
    const previousSettings = loadStudySettings();

    await updateCaptureSetting("ConsentEnabled", true);
    sessionRecordStudyEvents = true;
    await reconcileCaptureSettings(
        "manage_capture_give_consent_and_record",
        previousSettings,
    );
    vscode.window.showInformationMessage(
        "CodeChat capture consent is recorded and recording is on.",
    );
}

async function sendCaptureSettingsChangedEvent(
    previous: StudySettings,
    current: StudySettings,
    changedBy: string,
    filePath?: string,
): Promise<void> {
    // Only the consent and recording checkboxes are study-state transitions.
    // Other capture settings, such as path hashing, should not create audit
    // rows in the dissertation event stream.
    const changedSettings: string[] = [];
    if (previous.consentEnabled !== current.consentEnabled) {
        changedSettings.push("ConsentEnabled");
    }
    if (previous.enabled !== current.enabled) {
        changedSettings.push(CAPTURE_RECORD_AUDIT_LABEL);
    }
    if (changedSettings.length === 0) {
        return;
    }

    // Prefer the current participant ID, but fall back to the previous value so
    // turning consent off can still be attributed to the participant who opted
    // out.
    const participantId = current.participantId || previous.participantId;
    if (participantId.length === 0) {
        captureLog(
            `capture settings change skipped: ${changedSettings.join(",")} (no participant id)`,
        );
        return;
    }

    const previousState = captureSettingsState(previous);
    const currentState = captureSettingsState(current);
    // This audit event is deliberately allowed even when capture is no longer
    // active, because the transition itself is analytically important.
    await sendCaptureEvent(
        "capture_settings_changed",
        filePath,
        {
            changed_by: changedBy,
            changed_settings: changedSettings,
            previous_state: previousState,
            new_state: currentState,
            previous_consent_enabled: previous.consentEnabled,
            new_consent_enabled: current.consentEnabled,
            previous_record_study_events: previous.enabled,
            new_record_study_events: current.enabled,
            capture_active_before: previousState === "recording",
            capture_active_after: currentState === "recording",
        },
        {
            ignoreCaptureSettings: true,
            captureActive: currentState === "recording",
            userId: participantId,
        },
    );
}

async function reconcileCaptureSettings(
    changedBy: string = "settings_ui",
    previousSettings?: StudySettings,
): Promise<void> {
    const active = vscode.window.activeTextEditor;
    const filePath = active?.document.fileName;
    const settings = loadStudySettings();
    // The first reconciliation after activation uses the snapshot captured at
    // activation; command callers may also provide the pre-change snapshot.
    const previous = lastCaptureSettings ?? previousSettings;

    // Commands update settings and VS Code then fires a configuration event.
    // This guard keeps the capture audit trail to one row per actual transition.
    if (
        lastCaptureSettings !== undefined &&
        captureSettingsEqual(lastCaptureSettings, settings)
    ) {
        await refreshCaptureStatus();
        return;
    }

    // Write the audit row before changing the server active flag, so turning
    // capture off records the transition but not any later edit events.
    if (previous !== undefined) {
        await sendCaptureSettingsChangedEvent(
            previous,
            settings,
            changedBy,
            filePath,
        );
    }

    const updatedSettings = loadStudySettings();
    // Recording starts only when both checkboxes are on.
    if (captureSettingsState(updatedSettings) === "recording") {
        await startExtensionCaptureSession(filePath);
    } else if (
        // If capture was active before this transition, send a control-only stop
        // so the Rust translation layer stops emitting write_doc/write_code
        // events from stale context.
        extensionCaptureSessionStarted ||
        (previous !== undefined &&
            captureSettingsState(previous) === "recording")
    ) {
        await endExtensionCaptureSession(filePath, changedBy, {
            controlOnly: true,
        });
    } else {
        // A stop-control is harmless when a server is present and keeps the
        // server context inactive after settings-only transitions.
        await sendCaptureStopControl(filePath, changedBy);
    }

    // Refresh the dedupe snapshot after any participant ID generation or audit
    // send that may have touched settings.
    lastCaptureSettings = loadStudySettings();
    await refreshCaptureStatus();
}

async function copyParticipantId(): Promise<void> {
    const participantId = captureTokenRuntimeState.participantId;
    if (participantId.length === 0) {
        vscode.window.showWarningMessage(
            "Enter and verify a capture token before copying a participant ID.",
        );
        return;
    }
    await vscode.env.clipboard.writeText(participantId);
    vscode.window.showInformationMessage(
        "CodeChat capture participant ID copied.",
    );
}

async function enterCaptureToken(): Promise<void> {
    const previousSettings = loadStudySettings();
    const token = await vscode.window.showInputBox({
        title: "Enter CodeChat Capture Token",
        prompt: "Paste the portal-issued capture token.",
        password: true,
        ignoreFocusOut: true,
        validateInput: (value) =>
            value.trim().length === 0 ? "Capture token is required." : null,
    });
    if (token === undefined) {
        return;
    }
    const operationSnapshot = beginCaptureTokenMutation();
    const tokenText = token.trim();
    let mutationApplied = false;
    await enqueueCaptureTokenOperation(async () => {
        if (!(await captureTokenOperationStillCurrent(operationSnapshot))) {
            return;
        }
        await storeCaptureToken(tokenText);
        if (!(await captureTokenOperationStillCurrent(operationSnapshot))) {
            return;
        }
        captureTokenRuntimeState = {
            ...captureTokenRuntimeState,
            tokenStatus: "unverified",
            lastError: undefined,
        };
        mutationApplied = true;
    });
    if (
        mutationApplied &&
        captureTokenOperationSnapshotIsCurrent(operationSnapshot)
    ) {
        await refreshCaptureTokenState(true);
    }
    await reconcileCaptureSettings(
        "manage_capture_enter_token",
        previousSettings,
    );
}

async function clearCaptureToken(): Promise<void> {
    const operationSnapshot = beginCaptureTokenMutation();
    // Reserve clear's queue position before awaiting Rust reconfiguration so
    // later refreshes cannot observe the pre-clear token as current state.
    const clearTask = enqueueCaptureTokenOperation(async () => {
        if (!(await captureTokenOperationStillCurrent(operationSnapshot))) {
            return;
        }
        await deleteStoredCaptureToken();
        if (!(await captureTokenOperationStillCurrent(operationSnapshot))) {
            return;
        }
        await clearPersistedCaptureIdentity();
        if (!(await captureTokenOperationStillCurrent(operationSnapshot))) {
            return;
        }
        captureTokenRuntimeState = captureTokenClearedState();
        await reconcileCaptureSettings("manage_capture_clear_token");
        vscode.window.showInformationMessage("CodeChat capture token cleared.");
    });
    await Promise.all([
        clearTask,
        configureRustCaptureServiceIfCurrent(operationSnapshot, undefined),
    ]);
}

async function validateCaptureToken(): Promise<void> {
    const previousSettings = loadStudySettings();
    await refreshCaptureTokenState(true);
    await reconcileCaptureSettings(
        "manage_capture_validate_token",
        previousSettings,
    );
}

async function showCaptureStatus(): Promise<void> {
    await refreshCaptureStatus();
    const settings = loadStudySettings();
    const settingsStatus = captureSettingsStatus(settings);
    // The QuickPick exposes the same two independent switches as Settings, plus
    // one convenience action that turns both on at once.
    const actions: CaptureStatusAction[] = [
        {
            label: "Show Current Capture State",
            description: captureStateDescription(settingsStatus.state),
            detail: settingsStatus.tooltip,
            run: async () => {
                captureLog(`capture status: ${settingsStatus.tooltip}`);
                vscode.window.showInformationMessage(settingsStatus.tooltip);
            },
        },
    ];

    if (!settings.consentEnabled || !settings.enabled) {
        actions.push({
            label: "Give Consent and Record Study Events",
            description: "Turn both capture settings on.",
            run: giveConsentAndRecordStudyEvents,
        });
    }

    actions.push({
        label: "Enter Capture Token",
        description: `Token: ${captureTokenStatusLabel(settings.tokenStatus)}`,
        run: enterCaptureToken,
    });

    if (settings.tokenStatus !== "missing") {
        actions.push({
            label: "Validate Capture Token",
            description: "Check token status with CaptureWebService.",
            run: validateCaptureToken,
        });
    }

    actions.push({
        label: settings.consentEnabled ? "Turn Consent Off" : "Turn Consent On",
        description: settings.consentEnabled
            ? "Stop recording if active; keep the recording setting unchanged."
            : "Record participant consent; keep the recording setting unchanged.",
        run: () => setCaptureConsent(!settings.consentEnabled),
    });

    actions.push({
        label: settings.enabled
            ? "Turn Record Study Events Off"
            : "Turn Record Study Events On",
        description: settings.enabled
            ? "Stop recording; keep consent unchanged."
            : "Start recording only if consent is already on.",
        run: () => setRecordStudyEvents(!settings.enabled),
    });

    if (settings.tokenStatus !== "missing") {
        actions.push({
            label: "Clear Capture Token",
            description: "Remove the locally stored portal token.",
            run: clearCaptureToken,
        });
    }

    actions.push(
        {
            label: "Copy Participant ID",
            description: settings.participantId || "Verify a token first.",
            run: copyParticipantId,
        },
        {
            label: "Show Capture Details",
            description: captureStatusDetails().split("\n")[0],
            run: async () => {
                captureLog(`capture status: ${captureStatusDetails()}`);
                vscode.window.showInformationMessage(captureStatusDetails());
            },
        },
    );

    const selected = await vscode.window.showQuickPick(actions, {
        placeHolder: "Manage CodeChat capture",
    });
    if (selected !== undefined) {
        await selected.run();
    }
}

async function recordStudyLifecycleEvent(
    eventType: CaptureEventType,
): Promise<void> {
    if (captureDisabledReason(loadStudySettings()) !== undefined) {
        return;
    }
    const active = vscode.window.activeTextEditor;
    await sendCaptureEvent(eventType, active?.document.fileName, {
        command: eventType,
        languageId: active?.document.languageId,
    });
}

function reflectionPromptText(languageId: string, prompt: string): string {
    if (languageId === "markdown") {
        return `\n\n### Reflection\n\n${prompt}\n\n`;
    }
    if (languageId === "restructuredtext") {
        return `\n\nReflection\n----------\n\n${prompt}\n\n`;
    }
    if (languageId === "plaintext" || languageId === "latex") {
        return `\n${prompt}\n`;
    }
    const commentPrefix =
        languageId === "python" ||
        languageId === "shellscript" ||
        languageId === "powershell" ||
        languageId === "ruby"
            ? "#"
            : "//";
    return `\n${commentPrefix} Reflection: ${prompt}\n`;
}

async function insertReflectionPrompt(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (editor === undefined) {
        vscode.window.showInformationMessage("Open a text editor first.");
        return;
    }
    const prompt = await vscode.window.showQuickPick(
        DEFAULT_REFLECTION_PROMPTS,
        {
            placeHolder: "Select a reflection prompt",
        },
    );
    if (prompt === undefined) {
        return;
    }

    await editor.insertSnippet(
        new vscode.SnippetString(
            reflectionPromptText(editor.document.languageId, prompt),
        ),
    );
    await sendCaptureEvent(
        "reflection_prompt_inserted",
        editor.document.fileName,
        {
            prompt_hash: hashText(prompt),
            prompt_length: prompt.length,
            languageId: editor.document.languageId,
        },
    );
}

async function startExtensionCaptureSession(filePath?: string) {
    if (extensionCaptureSessionStarted) {
        return;
    }
    if (captureDisabledReason(loadStudySettings()) !== undefined) {
        return;
    }
    // Mark this before sending so recursive status refreshes do not emit a
    // second session_start for the same extension session.
    extensionCaptureSessionStarted = true;
    await sendCaptureEvent("session_start", filePath, {
        mode: "vscode_extension",
    });
}

async function endExtensionCaptureSession(
    filePath: string | undefined,
    closedBy: string,
    options: { controlOnly?: boolean } = {},
): Promise<void> {
    if (!extensionCaptureSessionStarted) {
        return;
    }
    if (options.controlOnly) {
        // Consent/recording changes must stop server-side write classification
        // without inserting a synthetic session_end row after the user opted
        // out or paused recording.
        docSessionStart = null;
        await sendCaptureStopControl(filePath, closedBy);
        extensionCaptureSessionStarted = false;
        return;
    }
    await closeDocSession(filePath, closedBy);
    await sendCaptureEvent("session_end", filePath, {
        mode: "vscode_extension",
        closed_by: closedBy,
    });
    extensionCaptureSessionStarted = false;
}

async function sendCaptureStopControl(
    filePath: string | undefined,
    closedBy: string,
): Promise<void> {
    if (codeChatEditorServer === undefined || !captureTransportReady) {
        return;
    }
    // This message is sent through the normal capture channel so the server can
    // clear its active capture context, but `capture_control_only` prevents it
    // from being spooled for upload.
    await sendCaptureEvent(
        "session_end",
        filePath,
        {
            mode: "vscode_extension",
            closed_by: closedBy,
        },
        {
            ignoreCaptureSettings: true,
            controlOnly: true,
            captureActive: false,
        },
    );
}

async function closeDocSession(
    filePath: string | undefined,
    closedBy: string,
): Promise<void> {
    if (docSessionStart === null) {
        return;
    }

    const durationMs = Date.now() - docSessionStart;
    docSessionStart = null;
    await sendCaptureEvent("doc_session", filePath, {
        duration_ms: durationMs,
        duration_seconds: durationMs / 1000.0,
        closed_by: closedBy,
    });
    await sendCaptureEvent("session_end", filePath, {
        mode: "doc",
        closed_by: closedBy,
    });
}

// Update activity state and emit switch/doc-session events. Markdown/RST prose
// and CodeChat doc-block edits are both documentation activity for analysis;
// server-side write events classify CodeChat doc-block edits precisely, while
// this extension-side activity tracker uses the best cursor/file context
// available before translation.
async function noteActivity(kind: ActivityKind, filePath?: string) {
    const now = Date.now();

    // Handle entering / leaving a "doc" session.
    if (kind === "doc") {
        if (docSessionStart === null) {
            // Starting a new reflective-writing session.
            docSessionStart = now;
            await sendCaptureEvent("session_start", filePath, {
                mode: "doc",
            });
        }
    } else {
        if (docSessionStart !== null) {
            // Ending a reflective-writing session.
            const closedBy =
                kind === "code" ? "switch_to_code" : "activity_change";
            const durationMs = now - docSessionStart;
            docSessionStart = null;
            await sendCaptureEvent("doc_session", filePath, {
                duration_ms: durationMs,
                duration_seconds: durationMs / 1000.0,
                closed_by: closedBy,
            });
            await sendCaptureEvent("session_end", filePath, {
                mode: "doc",
                closed_by: closedBy,
            });
        }
    }

    // If we switched between doc and code, log a switch\_pane event.
    const docOrCode = (k: ActivityKind) => k === "doc" || k === "code";
    if (
        docOrCode(lastActivityKind) &&
        docOrCode(kind) &&
        kind !== lastActivityKind
    ) {
        await sendCaptureEvent("switch_pane", filePath, {
            from: lastActivityKind,
            to: kind,
        });
    }

    lastActivityKind = kind;
}

function queueActivityCapture(kind: ActivityKind, filePath?: string): void {
    captureActivityQueue = captureActivityQueue
        .then(() => noteActivity(kind, filePath))
        .catch((err: unknown) => {
            reportCaptureFailure(
                `activity capture failed: ${
                    err instanceof Error ? err.message : String(err)
                }`,
            );
        });
}

// Activation/deactivation
// -----------------------
//
// This is invoked when the extension is activated. It either creates a new
// CodeChat Editor Server instance or reveals the currently running one.
export const activate = (context: vscode.ExtensionContext) => {
    extension_context = context;
    loadPersistedCaptureIdentity(context);
    lastCaptureSettings = loadStudySettings();
    capture_output_channel =
        vscode.window.createOutputChannel("CodeChat Capture");
    context.subscriptions.push(capture_output_channel);
    capture_status_bar_item = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Left,
        100,
    );
    capture_status_bar_item.command = "extension.codeChatCaptureStatus";
    context.subscriptions.push(capture_status_bar_item);
    capture_status_timer = setInterval(() => {
        refreshCaptureStatus();
    }, 5000);
    context.subscriptions.push({
        dispose: () => {
            if (capture_status_timer !== undefined) {
                clearInterval(capture_status_timer);
                capture_status_timer = undefined;
            }
        },
    });
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration(async (event) => {
            if (event.affectsConfiguration("CodeChatEditor.Capture")) {
                await refreshCaptureTokenState();
                await reconcileCaptureSettings("settings_ui");
            }
        }),
    );
    refreshCaptureTokenState();
    refreshCaptureStatus();

    context.subscriptions.push(
        vscode.commands.registerCommand(
            "extension.codeChatCaptureStatus",
            showCaptureStatus,
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureEnterToken",
            enterCaptureToken,
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureValidateToken",
            validateCaptureToken,
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureClearToken",
            clearCaptureToken,
        ),
        vscode.commands.registerCommand(
            "extension.codeChatInsertReflectionPrompt",
            insertReflectionPrompt,
        ),
        // Study lifecycle commands are registered for optional study
        // automation/keybindings, but they are not contributed to the Command
        // Palette. Normal users should only see status and reflection commands.
        vscode.commands.registerCommand(
            "extension.codeChatCaptureTaskStart",
            () => recordStudyLifecycleEvent("task_start"),
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureTaskSubmit",
            () => recordStudyLifecycleEvent("task_submit"),
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureDebugTaskStart",
            () => recordStudyLifecycleEvent("debug_task_start"),
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureDebugTaskSubmit",
            () => recordStudyLifecycleEvent("debug_task_submit"),
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureHandoffStart",
            () => recordStudyLifecycleEvent("handoff_start"),
        ),
        vscode.commands.registerCommand(
            "extension.codeChatCaptureHandoffEnd",
            () => recordStudyLifecycleEvent("handoff_end"),
        ),
        vscode.commands.registerCommand(
            "extension.codeChatEditorDeactivate",
            deactivate,
        ),
        vscode.commands.registerCommand(
            "extension.codeChatEditorActivate",
            async () => {
                console_log("CodeChat Editor extension: starting.");

                if (!subscribed) {
                    subscribed = true;

                    // Render when the text is changed by listening for the
                    // correct `event
                    // <https://code.visualstudio.com/docs/extensionAPI/vscode-api#Event>`\_.
                    context.subscriptions.push(
                        vscode.workspace.onDidChangeTextDocument((event) => {
                            // VSCode sends empty change events -- ignore these.
                            if (event.contentChanges.length === 0) {
                                return;
                            }
                            if (ignore_text_document_change) {
                                ignore_text_document_change = false;
                                return;
                            }
                            console_log(
                                `CodeChat Editor extension: text changed - ${
                                    event.reason
                                }, ${format_struct(event.contentChanges)}.`,
                            );

                            // CAPTURE: update session/switch state. The server
                            // classifies write_* events after parsing.
                            if (
                                captureDisabledReason(loadStudySettings()) ===
                                undefined
                            ) {
                                const doc = event.document;
                                const firstChange = event.contentChanges[0];
                                const pos = firstChange.range.start;
                                const kind = classifyAtPosition(doc, pos);

                                const filePath = doc.fileName;
                                // Update our notion of current activity + doc
                                // session.
                                queueActivityCapture(kind, filePath);
                            }

                            send_update(true);
                        }),
                    );

                    // Render when the active editor changes.
                    context.subscriptions.push(
                        vscode.window.onDidChangeActiveTextEditor((event) => {
                            // If no text editor is active (for example, the
                            // CodeChat Editor has focus), ignore this update.
                            if (event === undefined) {
                                return;
                            }
                            if (ignore_active_editor_change) {
                                ignore_active_editor_change = false;
                                return;
                            }
                            // Skip an update if we've already sent a
                            // `CurrentFile` for this editor.
                            if (
                                current_editor ===
                                vscode.window.activeTextEditor
                            ) {
                                return;
                            }

                            // CAPTURE: update activity + possible
                            // switch\_pane/doc\_session.
                            if (
                                captureDisabledReason(loadStudySettings()) ===
                                undefined
                            ) {
                                const doc = event.document;
                                const pos =
                                    event.selection?.active ??
                                    new vscode.Position(0, 0);
                                const kind = classifyAtPosition(doc, pos);

                                const filePath = doc.fileName;
                                queueActivityCapture(kind, filePath);
                            }

                            send_update(true);
                        }),
                    );

                    context.subscriptions.push(
                        vscode.window.onDidChangeTextEditorSelection(
                            (event) => {
                                if (ignore_selection_change) {
                                    ignore_selection_change = false;
                                    return;
                                }

                                console_log(
                                    "CodeChat Editor extension: sending updated cursor/scroll position.",
                                );

                                // CAPTURE: treat a selection change as "activity"
                                // in this document.
                                if (
                                    captureDisabledReason(
                                        loadStudySettings(),
                                    ) === undefined
                                ) {
                                    const doc = event.textEditor.document;
                                    const pos =
                                        event.selections?.[0]?.active ??
                                        event.textEditor.selection.active;
                                    const kind = classifyAtPosition(doc, pos);
                                    const filePath = doc.fileName;
                                    queueActivityCapture(kind, filePath);
                                }

                                send_update(false);
                            },
                        ),
                    );

                    // CAPTURE: listen for file saves.
                    context.subscriptions.push(
                        vscode.workspace.onDidSaveTextDocument((doc) => {
                            if (
                                captureDisabledReason(loadStudySettings()) !==
                                undefined
                            ) {
                                return;
                            }
                            sendCaptureEvent("save", doc.fileName, {
                                reason: "manual_save",
                                languageId: doc.languageId,
                                lineCount: doc.lineCount,
                            });
                        }),
                    );

                    // CAPTURE: start and end of a debug/run session.
                    context.subscriptions.push(
                        vscode.debug.onDidStartDebugSession((session) => {
                            if (
                                captureDisabledReason(loadStudySettings()) !==
                                undefined
                            ) {
                                return;
                            }
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            sendCaptureEvent("run", filePath, {
                                sessionName: session.name,
                                sessionType: session.type,
                            });
                        }),
                        vscode.debug.onDidTerminateDebugSession((session) => {
                            if (
                                captureDisabledReason(loadStudySettings()) !==
                                undefined
                            ) {
                                return;
                            }
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            sendCaptureEvent("run_end", filePath, {
                                sessionName: session.name,
                                sessionType: session.type,
                            });
                        }),
                    );

                    // CAPTURE: start and end compile/build events via VS Code
                    // tasks.
                    context.subscriptions.push(
                        vscode.tasks.onDidStartTaskProcess((e) => {
                            if (
                                captureDisabledReason(loadStudySettings()) !==
                                undefined
                            ) {
                                return;
                            }
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            const task = e.execution.task;
                            sendCaptureEvent("compile", filePath, {
                                taskName: task.name,
                                taskSource: task.source,
                                definition: task.definition,
                                processId: e.processId,
                            });
                        }),
                        vscode.tasks.onDidEndTaskProcess((e) => {
                            if (
                                captureDisabledReason(loadStudySettings()) !==
                                undefined
                            ) {
                                return;
                            }
                            const active = vscode.window.activeTextEditor;
                            const filePath = active?.document.fileName;
                            const task = e.execution.task;
                            sendCaptureEvent("compile_end", filePath, {
                                taskName: task.name,
                                taskSource: task.source,
                                exitCode: e.exitCode,
                            });
                        }),
                    );
                }

                // Get the CodeChat Client's location from the VSCode
                // configuration.
                const codechat_client_location_str = vscode.workspace
                    .getConfiguration("CodeChatEditor.Server")
                    .get("ClientLocation");
                assert(typeof codechat_client_location_str === "string");
                switch (codechat_client_location_str) {
                    case "html":
                        codechat_client_location =
                            CodeChatEditorClientLocation.html;
                        break;

                    case "browser":
                        codechat_client_location =
                            CodeChatEditorClientLocation.browser;
                        break;

                    default:
                        assert(false);
                }

                // Create or reveal the webview panel; if this is an external
                // browser, we'll open it after the client is created.
                if (
                    codechat_client_location ===
                    CodeChatEditorClientLocation.html
                ) {
                    if (webview_panel !== undefined) {
                        // As below, don't take the focus when revealing.
                        webview_panel.reveal(undefined, true);
                    } else {
                        // Create a webview panel.
                        webview_panel = vscode.window.createWebviewPanel(
                            "CodeChat Editor",
                            "CodeChat Editor",
                            {
                                // Without this, the focus becomes this webview;
                                // setting this allows the code window open
                                // before this command was executed to retain
                                // the focus and be immediately rendered.
                                preserveFocus: true,
                                // Put this in the column beside the current
                                // column.
                                viewColumn: vscode.ViewColumn.Beside,
                            },
                            // See
                            // [WebViewOptions](https://code.visualstudio.com/api/references/vscode-api#WebviewOptions).
                            {
                                enableScripts: true,
                                // Without this, the websocket connection is
                                // dropped when the panel is hidden.
                                retainContextWhenHidden: true,
                            },
                        );
                        webview_panel.onDidDispose(async () => {
                            // Shut down the render client when the webview
                            // panel closes.
                            console_log(
                                "CodeChat Editor extension: shut down webview.",
                            );
                            // Closing the webview abruptly closes the Client,
                            // which produces an error. Don't report it.
                            quiet_next_error = true;
                            webview_panel = undefined;
                            await stop_client();
                        });
                    }
                }

                // Provide a simple status display while the CodeChat Editor
                // Server is starting up.
                if (webview_panel !== undefined) {
                    // If we have an ID, then the GUI is already running; don't
                    // replace it.
                    webview_panel.webview.html =
                        "<h1>CodeChat Editor</h1><p>Loading...</p>";
                } else {
                    vscode.window.showInformationMessage(
                        "The CodeChat Editor is loading in an external browser...",
                    );
                }

                // Start the server.
                console_log("CodeChat Editor extension: starting server.");
                codeChatEditorServer = new CodeChatEditorServer();
                captureFailureLogged = false;
                captureTransportReady = false;
                extensionCaptureSessionStarted = false;
                await refreshCaptureTokenState();
                refreshCaptureStatus();

                const hosted_in_ide =
                    codechat_client_location ===
                    CodeChatEditorClientLocation.html;
                console_log(
                    `CodeChat Editor extension: sending message Opened(${hosted_in_ide}).`,
                );
                await codeChatEditorServer.sendMessageOpened(hosted_in_ide);

                // For the external browser, we can immediately send the
                // `CurrentFile` message. For the WebView, we must first wait to
                // receive the HTML for the WebView (the `ClientHtml` message).
                if (
                    codechat_client_location ===
                    CodeChatEditorClientLocation.browser
                ) {
                    captureTransportReady = true;
                    const active = vscode.window.activeTextEditor;
                    await startExtensionCaptureSession(
                        active?.document.fileName,
                    );
                    send_update(false);
                }

                while (codeChatEditorServer) {
                    const message_raw = await codeChatEditorServer.getMessage();
                    if (message_raw === null) {
                        console_log("CodeChat Editor extension: queue closed.");
                        break;
                    }

                    // Parse the data into a message.
                    const { id, message } = JSON.parse(
                        message_raw,
                    ) as EditorMessage;
                    console_log(
                        `CodeChat Editor extension: Received data id = ${id}, message = ${format_struct(
                            message,
                        )}.`,
                    );
                    assert(id !== undefined);
                    assert(message !== undefined);
                    if (message === "Closed") {
                        break;
                    }
                    const keys = Object.keys(message);
                    assert(keys.length === 1);
                    const key =
                        keys[0] as KeysOfRustEnum<EditorMessageContents>;
                    const value = Object.values(message)[0];

                    // Process this message.
                    switch (key) {
                        case "Update": {
                            const current_update =
                                value as UpdateMessageContents;
                            const doc = get_document(current_update.file_path);
                            if (doc === undefined) {
                                await sendResult(id, {
                                    NoOpenDocument: current_update.file_path,
                                });
                                break;
                            }
                            if (current_update.contents !== undefined) {
                                const source = current_update.contents.source;

                                // This will produce a change event, which we'll
                                // ignore. The change may also produce a
                                // selection change, which should also be
                                // ignored.
                                ignore_text_document_change = true;
                                ignore_selection_change = true;

                                // Use a workspace edit, since calls to
                                // `TextEditor.edit` must be made to the active
                                // editor only.
                                const wse = new vscode.WorkspaceEdit();

                                // Is this plain text, or a diff?
                                if ("Plain" in source) {
                                    wse.replace(
                                        doc.uri,
                                        doc.validateRange(
                                            new vscode.Range(
                                                0,
                                                0,
                                                doc.lineCount,
                                                0,
                                            ),
                                        ),
                                        source.Plain.doc,
                                    );
                                } else {
                                    assert("Diff" in source);

                                    // If this diff was not made against the
                                    // text we currently have, reject it.
                                    if (source.Diff.version !== version) {
                                        await sendResult(id, {
                                            OutOfSync: [
                                                version,
                                                source.Diff.version,
                                            ],
                                        });
                                        // Send an `Update` with the full text to
                                        // re-sync the Client.
                                        console_log(
                                            "CodeChat Editor extension: sending update because Client is out of sync.",
                                        );
                                        send_update(true);
                                        break;
                                    }
                                    const diffs = source.Diff.doc;
                                    for (const diff of diffs) {
                                        // Convert from character offsets from
                                        // the beginning of the document to a
                                        // `Position` (line, then offset on that
                                        // line) needed by VSCode.
                                        const from = doc.positionAt(diff.from);
                                        if (diff.to === undefined) {
                                            // This is an insert.
                                            wse.insert(
                                                doc.uri,
                                                from,
                                                diff.insert,
                                            );
                                        } else {
                                            // This is a replace or delete.
                                            const to = doc.positionAt(diff.to);
                                            wse.replace(
                                                doc.uri,
                                                new Range(from, to),
                                                diff.insert,
                                            );
                                        }
                                    }
                                }
                                await vscode.workspace.applyEdit(wse);
                                ignore_text_document_change = false;
                                ignore_selection_change = false;

                                // Now that we've updated our text, update the
                                // associated version as well.
                                version = current_update.contents.version;
                            }

                            // Update the cursor and scroll position if
                            // provided.
                            const editor = get_text_editor(doc);

                            const scroll_line = current_update.scroll_position;
                            if (scroll_line !== undefined && editor) {
                                // Don't set `ignore_selection_change` here:
                                // `revealRange` doesn't change the editor's
                                // text selection.
                                const scroll_position = new vscode.Position(
                                    // The VSCode line is zero-based; the
                                    // CodeMirror line is one-based.
                                    scroll_line - 1,
                                    0,
                                );
                                editor.revealRange(
                                    new vscode.Range(
                                        scroll_position,
                                        scroll_position,
                                    ),
                                    // This is still not the top of the
                                    // viewport, but a bit below it.
                                    TextEditorRevealType.AtTop,
                                );
                            }

                            const cursor_position =
                                current_update.cursor_position;
                            if (
                                cursor_position !== undefined &&
                                typeof cursor_position === "object" &&
                                "Line" in cursor_position &&
                                editor
                            ) {
                                const cursor_line = (
                                    cursor_position as { Line: number }
                                ).Line;
                                ignore_selection_change = true;
                                const vscode_cursor_position =
                                    new vscode.Position(
                                        // The VSCode line is zero-based; the
                                        // CodeMirror line is one-based.
                                        cursor_line - 1,
                                        0,
                                    );
                                editor.selections = [
                                    new vscode.Selection(
                                        vscode_cursor_position,
                                        vscode_cursor_position,
                                    ),
                                ];
                                // I'd prefer to set `ignore_selection_change =
                                // false` here, but even doing so after a
                                // `setTimeout(..., 0)` doesn't work; evidently,
                                // the event is generated at some later time.
                                // Instead, depend on the event to always clear
                                // this flag (a source of potential bugs).
                            }
                            if (
                                cursor_position !== undefined &&
                                typeof cursor_position === "object" &&
                                "DomLocation" in cursor_position
                            ) {
                                // VS Code can only apply line-based cursor
                                // locations. DOM locations should be converted
                                // by the server before reaching the extension.
                                console_log(
                                    "CodeChat Editor extension: ignoring DOM cursor location in VS Code update.",
                                );
                            }
                            await sendResult(id);
                            break;
                        }

                        case "CurrentFile": {
                            const current_file = value[0] as string;
                            const is_text = value[1] as boolean | undefined;
                            if (is_text) {
                                let document;
                                try {
                                    document =
                                        await vscode.workspace.openTextDocument(
                                            current_file,
                                        );
                                } catch (e) {
                                    await sendResult(id, {
                                        OpenFileFailed: [
                                            current_file,
                                            (e as Error).toString(),
                                        ],
                                    });
                                    continue;
                                }
                                ignore_active_editor_change = true;
                                current_editor =
                                    await vscode.window.showTextDocument(
                                        document,
                                        current_editor?.viewColumn,
                                    );
                                ignore_active_editor_change = false;
                                await sendResult(id);
                            } else {
                                // TODO: open using a custom document editor.
                                // See
                                // [openCustomDocument](https://code.visualstudio.com/api/references/vscode-api#CustomEditorProvider.openCustomDocument),
                                // which can evidently be called
                                // [indirectly](https://stackoverflow.com/a/65101181/4374935).
                                // See also
                                // [Built-in Commands](https://code.visualstudio.com/api/references/commands).
                                // For now, simply respond with an OK, since the
                                // following doesn't work.
                                /**
                                    commands
                                        .executeCommand(
                                            "vscode.open",
                                            vscode.Uri.file(current_file),
                                            { viewColumn: current_editor?.viewColumn },
                                        )
                                        .then(
                                            async () => await sendResult(id),
                                            async (reason) =>
                                                await sendResult(id, {
                                                    OpenFileFailed: [current_file, reason],
                                                }),
                                        );
                                */
                                await sendResult(id);
                            }
                            break;
                        }

                        case "Result": {
                            // Report if this was an error.
                            const result_contents = value as MessageResult;
                            if ("Err" in result_contents) {
                                const err = result_contents["Err"];
                                if (
                                    err instanceof Object &&
                                    "OutOfSync" in err
                                ) {
                                    // Send an update to re-sync the Client.
                                    console.warn(
                                        "Client is out of sync; resyncing.",
                                    );
                                    send_update(true);
                                } else {
                                    // Report the error.
                                    show_error(
                                        `Error in message ${id}: ${JSON.stringify(err)}`,
                                    );
                                }
                            }
                            break;
                        }

                        case "LoadFile": {
                            const [load_file, is_client_current] = value as [
                                string,
                                boolean,
                            ];
                            // Look through all open documents to see if we have
                            // the requested file.
                            const doc = get_document(load_file);
                            // If we have this file and the request is for the
                            // current file to edit/view in the Client, assign a
                            // version.
                            const is_current_ide =
                                doc !== undefined && is_client_current;
                            if (is_current_ide) {
                                version = rand();
                            }
                            const load_file_result: null | [string, number] =
                                doc === undefined
                                    ? null
                                    : [doc.getText(), version];
                            console_log(
                                `CodeChat Editor extension: Result(LoadFile(id = ${id}, ${format_struct(load_file_result)}))`,
                            );
                            await codeChatEditorServer.sendResultLoadfile(
                                id,
                                load_file_result,
                            );
                            // If this is the currently active file in VSCode,
                            // send its cursor location that VSCode
                            // automatically restores.
                            if (is_current_ide) {
                                send_update(false);
                            }
                            break;
                        }

                        case "ClientHtml": {
                            const client_html = value as string;
                            assert(webview_panel !== undefined);
                            webview_panel.webview.html = client_html;
                            await sendResult(id);
                            captureTransportReady = true;
                            const active = vscode.window.activeTextEditor;
                            await startExtensionCaptureSession(
                                active?.document.fileName,
                            );
                            // Now that the Client is loaded, send the editor's
                            // current file to the server.
                            send_update(false);
                            break;
                        }

                        default:
                            console.error(
                                `Unhandled message ${key}(${format_struct(value)}`,
                            );
                            break;
                    }
                }
            },
        ),
    );
};

// On deactivation, close everything down.
export const deactivate = async () => {
    console_log("CodeChat Editor extension: deactivating.");

    const active = vscode.window.activeTextEditor;
    await endExtensionCaptureSession(
        active?.document.fileName,
        "extension_deactivate",
    );

    await stop_client();
    webview_panel?.dispose();
    console_log("CodeChat Editor extension: deactivated.");
};

// Supporting functions
// --------------------
//
// Format a complex data structure as a string when in debug mode.
/*eslint-disable-next-line @typescript-eslint/no-explicit-any */
const format_struct = (complex_data_structure: any): string =>
    DEBUG_ENABLED
        ? JSON.stringify(
              // If the struct is `undefined`, print an empty string.
              complex_data_structure ?? "null/undefined",
          ).substring(0, MAX_MESSAGE_LENGTH)
        : "";

// Send a result (a response to a message from the server) back to the server.
const sendResult = async (id: number, result?: ResultErrTypes) => {
    assert(codeChatEditorServer);
    console_log(
        `CodeChat Editor extension: sending Result(id = ${id}, ${format_struct(
            result,
        )}).`,
    );
    try {
        await codeChatEditorServer.sendResult(
            id,
            result === undefined ? undefined : JSON.stringify(result),
        );
    } catch (e) {
        show_error(`Error in sendResult for id ${id}: ${e}.`);
    }
};

// This is called after an event such as an edit, when the CodeChat panel
// becomes visible, or when the current editor changes. Wait a bit in case any
// other events occur, then request a render.
const send_update = (this_is_dirty: boolean) => {
    is_dirty ||= this_is_dirty;
    if (can_render()) {
        // Render after some inactivity: cancel any existing timer, then ...
        if (idle_timer !== undefined) {
            clearTimeout(idle_timer);
        }
        // ... schedule a render after an auto update timeout.
        idle_timer = setTimeout(async () => {
            if (can_render()) {
                const ate = vscode.window.activeTextEditor;
                if (ate !== undefined && ate !== current_editor) {
                    // Send a new current file after a short delay; this allows
                    // the user to rapidly cycle through several editors without
                    // needing to reload the Client with each cycle.
                    current_editor = ate;
                    const current_file = ate.document.fileName;
                    console_log(
                        `CodeChat Editor extension: sending CurrentFile(${current_file}}).`,
                    );
                    try {
                        await codeChatEditorServer!.sendMessageCurrentFile(
                            current_file,
                        );
                    } catch (e) {
                        show_error(`Error sending CurrentFile message: ${e}.`);
                    }
                    // Since we just requested a new file, the contents are
                    // clean by definition.
                    is_dirty = false;
                    // Don't send an updated cursor position until this file is
                    // loaded.
                    return;
                }

                // The
                // [Position](https://code.visualstudio.com/api/references/vscode-api#Position)
                // encodes the line as a zero-based value. In contrast,
                // CodeMirror
                // [Text.line](https://codemirror.net/docs/ref/#state.Text.line)
                // is 1-based.
                const cursor_position =
                    current_editor!.selection.active.line + 1;
                const scroll_position =
                    current_editor!.visibleRanges[0].start.line + 1;
                const file_path = current_editor!.document.fileName;

                // Send contents only if necessary.
                const option_contents: null | [string, number] = is_dirty
                    ? [current_editor!.document.getText(), (version = rand())]
                    : null;
                is_dirty = false;

                console_log(
                    `CodeChat Editor extension: sending Update(${file_path}, ${cursor_position}, ${scroll_position}, ${format_struct(
                        option_contents,
                    )})`,
                );
                await codeChatEditorServer!.sendMessageUpdatePlain(
                    file_path,
                    option_contents,
                    cursor_position,
                    scroll_position,
                );
            }
        }, auto_update_timeout_ms);
    }
};

// Gracefully shut down the render client if possible. Shut down the client as
// well.
const stop_client = async () => {
    console_log("CodeChat Editor extension: stopping client.");
    const active = vscode.window.activeTextEditor;
    await endExtensionCaptureSession(
        active?.document.fileName,
        "client_stopped",
    );
    if (codeChatEditorServer !== undefined) {
        console_log("CodeChat Editor extension: stopping server.");
        await codeChatEditorServer.stopServer();
        codeChatEditorServer = undefined;
    }
    captureTransportReady = false;
    await refreshCaptureStatus();

    if (idle_timer !== undefined) {
        clearTimeout(idle_timer);
        idle_timer = undefined;
    }

    current_editor = undefined;
};

// Provide an error message in the panel if possible.
const show_error = (message: string) => {
    if (quiet_next_error) {
        quiet_next_error = false;
        return;
    }
    console.error(`CodeChat Editor extension: ${message}`);
    if (webview_panel !== undefined) {
        if (
            !webview_panel.webview.html.startsWith("<h1>CodeChat Editor</h1>")
        ) {
            webview_panel.webview.html = "<h1>CodeChat Editor</h1>";
        }
        webview_panel.webview.html += `<p style="white-space: pre-wrap;">${escape(
            message,
        )}</p><p>See the <a href="https://github.com/bjones1/CodeChat_Editor" target="_blank" rel="noreferrer noopener">docs</a>.</p>`;
    } else {
        vscode.window.showErrorMessage(
            message + "\nSee https://github.com/bjones1/CodeChat_Editor.",
        );
    }
};

// Only render if the window and editor are active, we have a valid render
// client, and the webview is visible.
const can_render = () => {
    return (
        (vscode.window.activeTextEditor !== undefined ||
            current_editor !== undefined) &&
        codeChatEditorServer !== undefined &&
        // TODO: I don't think these matter -- the Server is in charge of
        // sending output to the Client.
        (codechat_client_location === CodeChatEditorClientLocation.browser ||
            webview_panel !== undefined)
    );
};

const get_document = (file_path: string) => {
    // Look through all open documents to see if we have the requested file.
    for (const doc of vscode.workspace.textDocuments) {
        // Make the possibly incorrect assumption that only Windows filesystems
        // are case-insensitive; I don't know how to easily determine the
        // case-sensitivity of the current filesystem without extra probing code
        // (write a file in mixed case, try to open it in another mixed case.)
        // Per
        // [How to Work with Different Filesystems](https://nodejs.org/en/learn/manipulating-files/working-with-different-filesystems#filesystem-behavior),
        // "Be wary of inferring filesystem behavior from `process.platform`.
        // For example, do not assume that because your program is running on
        // Darwin that you are therefore working on a case-insensitive
        // filesystem (HFS+), as the user may be using a case-sensitive
        // filesystem (HFSX)."
        //
        // The same article
        // [recommends](https://nodejs.org/en/learn/manipulating-files/working-with-different-filesystems#be-prepared-for-slight-differences-in-comparison-functions)
        // using `toUpperCase` for case-insensitive filename comparisons.
        if (
            (!is_windows && doc.fileName === file_path) ||
            (is_windows &&
                doc.fileName.toUpperCase() === file_path.toUpperCase())
        ) {
            return doc;
        }
    }
    return undefined;
};

const get_text_editor = (doc: TextDocument): TextEditor | undefined => {
    for (const editor of vscode.window.visibleTextEditors) {
        if (editor.document === doc) return editor;
    }
};
