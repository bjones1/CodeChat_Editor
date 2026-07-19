// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor.

export type CaptureTokenPolicyStatus =
    | "missing"
    | "unverified"
    | "accepted"
    | "rejected"
    | "capture_disabled"
    | "service_unavailable";

export interface CaptureServiceBaseUrlInspection {
    globalValue?: unknown;
}

export interface CaptureTokenClearedState {
    tokenStatus: "missing";
    participantId: "";
    instanceId: "";
    studyId: "";
    captureEnabled: false;
    lastError: string;
}

export interface CaptureTokenCurrentnessSnapshot {
    mutationGeneration: number;
    refreshGeneration?: number;
}

const CAPTURE_SERVICE_ROUTE_SUFFIXES = [
    "/v1/capture/events",
    "/v1/capture/status",
    "/v1/health",
];

function optionalString(value: unknown): string | undefined {
    return typeof value === "string" && value.trim().length > 0
        ? value.trim()
        : undefined;
}

export function trustedCaptureServiceBaseUrl(
    inspected: CaptureServiceBaseUrlInspection | undefined,
    defaultBaseUrl: string,
): string {
    return optionalString(inspected?.globalValue) ?? defaultBaseUrl;
}

function isLocalHttpCaptureService(url: URL): boolean {
    const hostname = url.hostname.toLowerCase();
    return (
        url.protocol === "http:" &&
        (hostname === "localhost" ||
            hostname === "127.0.0.1" ||
            hostname === "::1" ||
            hostname === "[::1]")
    );
}

export function normalizeCaptureServiceBaseUrl(value: string): string {
    let rawUrl = value.trim().replace(/\/+$/, "");
    if (rawUrl.length === 0) {
        throw new Error("capture service URL must not be empty");
    }

    for (const suffix of CAPTURE_SERVICE_ROUTE_SUFFIXES) {
        if (rawUrl.endsWith(suffix)) {
            rawUrl = rawUrl.slice(0, -suffix.length).replace(/\/+$/, "");
            break;
        }
    }

    let url: URL;
    try {
        url = new URL(rawUrl);
    } catch {
        throw new Error("capture service URL must be an absolute URL");
    }

    if (url.username.length > 0 || url.password.length > 0) {
        throw new Error("capture service URL must not include credentials");
    }
    if (url.protocol !== "https:" && !isLocalHttpCaptureService(url)) {
        throw new Error(
            "capture service URL must use https:// except for localhost",
        );
    }

    url.hash = "";
    url.search = "";
    url.pathname = url.pathname.replace(/\/+$/, "");
    return url.toString().replace(/\/+$/, "");
}

export function captureTokenCanRecord(
    participantId: string,
    captureEnabled: boolean | undefined,
    tokenStatus: CaptureTokenPolicyStatus,
): boolean {
    return (
        participantId.length > 0 &&
        captureEnabled === true &&
        (tokenStatus === "accepted" || tokenStatus === "service_unavailable")
    );
}

export function captureTokenStatusForStatusFailure(
    statusCode: number | undefined,
): CaptureTokenPolicyStatus {
    switch (statusCode) {
        case 401:
            return "rejected";
        case 403:
            return "capture_disabled";
        default:
            return "service_unavailable";
    }
}

export function captureStatusFailureClearsIdentity(
    statusCode: number | undefined,
): boolean {
    return statusCode === 401 || statusCode === 403;
}

export function captureRefreshStillCurrentSnapshot(
    refreshGeneration: number,
    currentGeneration: number,
    storedTokenHash?: string,
    expectedTokenHash?: string,
    currentBaseUrl?: string,
    expectedBaseUrl?: string,
): boolean {
    return captureTokenSnapshotStillCurrent(
        {
            mutationGeneration: refreshGeneration,
            refreshGeneration,
        },
        currentGeneration,
        currentGeneration,
        storedTokenHash,
        expectedTokenHash,
        currentBaseUrl,
        expectedBaseUrl,
    );
}

export function captureTokenSnapshotStillCurrent(
    snapshot: CaptureTokenCurrentnessSnapshot,
    currentMutationGeneration: number,
    currentRefreshGeneration: number,
    storedTokenHash?: string,
    expectedTokenHash?: string,
    currentBaseUrl?: string,
    expectedBaseUrl?: string,
): boolean {
    if (snapshot.mutationGeneration !== currentMutationGeneration) {
        return false;
    }
    if (
        snapshot.refreshGeneration !== undefined &&
        snapshot.refreshGeneration !== currentRefreshGeneration
    ) {
        return false;
    }
    if (
        expectedTokenHash !== undefined &&
        storedTokenHash !== expectedTokenHash
    ) {
        return false;
    }
    if (expectedBaseUrl !== undefined && currentBaseUrl !== expectedBaseUrl) {
        return false;
    }
    return true;
}

export function appendSerializedCaptureOperation(
    queue: Promise<void>,
    operation: () => Promise<void>,
    onFailure: (error: unknown) => void,
): Promise<void> {
    return queue.then(operation).catch(onFailure);
}

export function captureTokenClearedState(): CaptureTokenClearedState {
    return {
        tokenStatus: "missing",
        participantId: "",
        instanceId: "",
        studyId: "",
        captureEnabled: false,
        lastError: "Capture token is not configured.",
    };
}
