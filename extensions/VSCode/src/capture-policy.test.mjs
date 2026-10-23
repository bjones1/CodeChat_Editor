// The CodeChat Editor, a word processor for software developers.<br>Copyright
// (C) 2024-2026 Bryan A. Jones.
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
//
// `capture-policy.test.mjs` -- Tests for the Capture feature
// ==========================================================
import assert from "node:assert/strict";
import test from "node:test";

import {
    appendSerializedCaptureOperation,
    captureRefreshStillCurrentSnapshot,
    captureStatusFailureClearsIdentity,
    captureTokenCanRecord,
    captureTokenClearedState,
    captureTokenSnapshotStillCurrent,
    captureTokenStatusForStatusFailure,
    normalizeCaptureServiceBaseUrl,
    trustedCaptureServiceBaseUrl,
} from "../.test-output/capture-policy.test.mjs";

test("capture operations run serially and recover from rejection", async () => {
    const calls = [];
    let releaseFirst;
    const firstCanFinish = new Promise((resolve) => {
        releaseFirst = resolve;
    });

    let queue = appendSerializedCaptureOperation(
        Promise.resolve(),
        async () => {
            calls.push("first-start");
            await firstCanFinish;
            calls.push("first-end");
        },
        (error) => calls.push(`unexpected: ${String(error)}`),
    );
    queue = appendSerializedCaptureOperation(
        queue,
        async () => {
            calls.push("second");
            throw new Error("expected failure");
        },
        (error) => calls.push(error.message),
    );
    queue = appendSerializedCaptureOperation(
        queue,
        async () => {
            calls.push("third");
        },
        (error) => calls.push(`unexpected: ${String(error)}`),
    );

    await Promise.resolve();
    assert.deepEqual(calls, ["first-start"]);
    releaseFirst();
    await queue;
    assert.deepEqual(calls, [
        "first-start",
        "first-end",
        "second",
        "expected failure",
        "third",
    ]);
});

test("capture service URL normalization strips known routes", () => {
    assert.equal(
        normalizeCaptureServiceBaseUrl(
            "https://capture.example/dev/v1/capture/events/",
        ),
        "https://capture.example/dev",
    );
    assert.equal(
        normalizeCaptureServiceBaseUrl(
            "http://localhost:8787/v1/capture/status",
        ),
        "http://localhost:8787",
    );
});

test("capture service URL normalization rejects unsafe token destinations", () => {
    assert.throws(
        () => normalizeCaptureServiceBaseUrl("http://capture.example/dev"),
        /https:\/\/ except for localhost/,
    );
    assert.throws(
        () => normalizeCaptureServiceBaseUrl("http://localhost.evil/dev"),
        /https:\/\/ except for localhost/,
    );
    assert.throws(
        () => normalizeCaptureServiceBaseUrl("postgres://capture.example/dev"),
        /https:\/\/ except for localhost/,
    );
    assert.throws(
        () =>
            normalizeCaptureServiceBaseUrl("https://user:pass@example.com/dev"),
        /must not include credentials/,
    );
});

test("offline recording requires a cached capture-enabled token", () => {
    assert.equal(captureTokenCanRecord("participant", true, "accepted"), true);
    assert.equal(
        captureTokenCanRecord("participant", true, "service_unavailable"),
        true,
    );
    assert.equal(
        captureTokenCanRecord("participant", false, "service_unavailable"),
        false,
    );
    assert.equal(
        captureTokenCanRecord("participant", undefined, "service_unavailable"),
        false,
    );
    assert.equal(captureTokenCanRecord("", true, "accepted"), false);
});

test("capture status HTTP failures map to service contract states", () => {
    assert.equal(captureTokenStatusForStatusFailure(401), "rejected");
    assert.equal(captureTokenStatusForStatusFailure(403), "capture_disabled");
    assert.equal(
        captureTokenStatusForStatusFailure(500),
        "service_unavailable",
    );
    assert.equal(
        captureTokenStatusForStatusFailure(undefined),
        "service_unavailable",
    );
    assert.equal(captureStatusFailureClearsIdentity(401), true);
    assert.equal(captureStatusFailureClearsIdentity(403), true);
    assert.equal(captureStatusFailureClearsIdentity(500), false);
    assert.equal(captureStatusFailureClearsIdentity(undefined), false);
});

test("capture service URL selection ignores workspace values", () => {
    assert.equal(
        trustedCaptureServiceBaseUrl(
            {
                globalValue: "https://trusted.example/dev",
                workspaceValue: "https://workspace.example/dev",
                workspaceFolderValue: "https://folder.example/dev",
            },
            "https://default.example/dev",
        ),
        "https://trusted.example/dev",
    );
    assert.equal(
        trustedCaptureServiceBaseUrl(
            { workspaceValue: "https://workspace.example/dev" },
            "https://default.example/dev",
        ),
        "https://default.example/dev",
    );
});

test("capture state changes invalidate an in-flight refresh", () => {
    const inFlightGeneration = 7;
    const generationAfterClear = 8;

    assert.equal(
        captureRefreshStillCurrentSnapshot(
            inFlightGeneration,
            generationAfterClear,
            "token-hash",
            "token-hash",
            "https://trusted.example/dev",
            "https://trusted.example/dev",
        ),
        false,
    );
    assert.equal(
        captureRefreshStillCurrentSnapshot(
            generationAfterClear,
            generationAfterClear,
            undefined,
            "token-hash",
            "https://trusted.example/dev",
            "https://trusted.example/dev",
        ),
        false,
    );
    assert.equal(
        captureRefreshStillCurrentSnapshot(
            generationAfterClear,
            generationAfterClear,
            "token-hash",
            "token-hash",
            "https://other.example/dev",
            "https://trusted.example/dev",
        ),
        false,
    );
    assert.deepEqual(captureTokenClearedState(), {
        tokenStatus: "missing",
        participantId: "",
        instanceId: "",
        studyId: "",
        captureEnabled: false,
        lastError: "Capture token is not configured.",
    });
});

test("refresh churn does not invalidate a token mutation", () => {
    assert.equal(
        captureTokenSnapshotStillCurrent(
            { mutationGeneration: 3 },
            3,
            10,
            "token-hash",
            "token-hash",
            "https://trusted.example/dev",
            "https://trusted.example/dev",
        ),
        true,
    );
    assert.equal(
        captureTokenSnapshotStillCurrent({ mutationGeneration: 3 }, 4, 10),
        false,
    );
});

test("new refreshes and token mutations invalidate older refreshes", () => {
    assert.equal(
        captureTokenSnapshotStillCurrent(
            { mutationGeneration: 3, refreshGeneration: 7 },
            3,
            8,
        ),
        false,
    );
    assert.equal(
        captureTokenSnapshotStillCurrent(
            { mutationGeneration: 3, refreshGeneration: 8 },
            4,
            8,
        ),
        false,
    );
    assert.equal(
        captureTokenSnapshotStillCurrent(
            { mutationGeneration: 3, refreshGeneration: 8 },
            3,
            8,
        ),
        true,
    );
});
