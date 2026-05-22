-- Capture Events Schema
-- =====================
--
-- CodeChat capture event schema for dissertation analysis.
--
-- This script updates an existing legacy `events` table to the lean capture
-- schema used for dissertation telemetry. It converts `timestamp` and `data` to
-- analysis-friendly PostgreSQL types and backfills typed telemetry from
-- older JSON payloads where possible. New capture code writes known telemetry
-- metadata to first-class columns and reserves `data` for event-specific
-- details. Study metadata such as course, group,
-- assignment, condition, and task is intentionally omitted: those values are
-- joined during analysis from researcher-managed participant/date mappings.

BEGIN;

CREATE TABLE IF NOT EXISTS public.events (
    id                  BIGSERIAL PRIMARY KEY,
    event_id            TEXT,
    sequence_number     BIGINT,
    schema_version      INTEGER,
    user_id             TEXT NOT NULL,
    session_id          TEXT,
    event_source        TEXT,
    language_id         TEXT,
    file_hash           TEXT,
    event_type          TEXT NOT NULL,
    "timestamp"         TIMESTAMPTZ NOT NULL DEFAULT now(),
    client_tz_offset_min INTEGER,
    data                JSONB NOT NULL DEFAULT '{}'::jsonb,
    inserted_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.events ADD COLUMN IF NOT EXISTS event_id TEXT;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS sequence_number BIGINT;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS schema_version INTEGER;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS session_id TEXT;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS event_source TEXT;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS language_id TEXT;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS file_hash TEXT;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS client_tz_offset_min INTEGER;
ALTER TABLE public.events ADD COLUMN IF NOT EXISTS inserted_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE public.events DROP COLUMN IF EXISTS assignment_id;
ALTER TABLE public.events DROP COLUMN IF EXISTS group_id;
ALTER TABLE public.events DROP COLUMN IF EXISTS condition;
ALTER TABLE public.events DROP COLUMN IF EXISTS course_id;
ALTER TABLE public.events DROP COLUMN IF EXISTS task_id;
ALTER TABLE public.events DROP COLUMN IF EXISTS capture_mode;
ALTER TABLE public.events DROP COLUMN IF EXISTS file_path;
ALTER TABLE public.events DROP COLUMN IF EXISTS path_privacy;
ALTER TABLE public.events DROP COLUMN IF EXISTS server_timestamp_ms;
ALTER TABLE public.events DROP COLUMN IF EXISTS client_timestamp_ms;

DO $$
DECLARE
    current_type TEXT;
BEGIN
    SELECT data_type INTO current_type
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'events'
      AND column_name = 'timestamp';

    IF current_type IS DISTINCT FROM 'timestamp with time zone' THEN
        ALTER TABLE public.events
            ALTER COLUMN "timestamp" TYPE TIMESTAMPTZ
            USING COALESCE(NULLIF("timestamp"::text, '')::timestamptz, now());
    END IF;
END $$;

DO $$
DECLARE
    current_type TEXT;
BEGIN
    SELECT data_type INTO current_type
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'events'
      AND column_name = 'data';

    IF current_type IS DISTINCT FROM 'jsonb' THEN
        ALTER TABLE public.events
            ALTER COLUMN data TYPE JSONB
            USING CASE
                WHEN data IS NULL OR btrim(data::text) = '' THEN '{}'::jsonb
                ELSE data::jsonb
            END;
    END IF;
END $$;

UPDATE public.events
SET data = '{}'::jsonb
WHERE data IS NULL;

ALTER TABLE public.events ALTER COLUMN data SET DEFAULT '{}'::jsonb;
ALTER TABLE public.events ALTER COLUMN data SET NOT NULL;
ALTER TABLE public.events ALTER COLUMN "timestamp" SET DEFAULT now();
ALTER TABLE public.events ALTER COLUMN "timestamp" SET NOT NULL;

UPDATE public.events
SET
    event_id = COALESCE(event_id, NULLIF(data->>'event_id', '')),
    sequence_number = COALESCE(
        sequence_number,
        CASE
            WHEN data->>'sequence_number' ~ '^-?[0-9]+$'
            THEN (data->>'sequence_number')::bigint
        END
    ),
    schema_version = COALESCE(
        schema_version,
        CASE
            WHEN data->>'schema_version' ~ '^-?[0-9]+$'
            THEN (data->>'schema_version')::integer
        END
    ),
    session_id = COALESCE(session_id, NULLIF(data->>'session_id', '')),
    event_source = COALESCE(event_source, NULLIF(data->>'event_source', '')),
    language_id = COALESCE(
        language_id,
        NULLIF(data->>'language_id', ''),
        NULLIF(data->>'languageId', '')
    ),
    file_hash = COALESCE(file_hash, NULLIF(data->>'file_hash', '')),
    client_tz_offset_min = COALESCE(
        client_tz_offset_min,
        CASE
            WHEN data->>'client_tz_offset_min' ~ '^-?[0-9]+$'
            THEN (data->>'client_tz_offset_min')::integer
        END
    );

CREATE INDEX IF NOT EXISTS events_timestamp_idx
    ON public.events ("timestamp");

CREATE INDEX IF NOT EXISTS events_type_timestamp_idx
    ON public.events (event_type, "timestamp");

CREATE INDEX IF NOT EXISTS events_participant_session_idx
    ON public.events (user_id, session_id);

CREATE INDEX IF NOT EXISTS events_file_hash_idx
    ON public.events (file_hash)
    WHERE file_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS events_event_id_idx
    ON public.events (event_id)
    WHERE event_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS events_data_gin_idx
    ON public.events USING GIN (data);

COMMENT ON TABLE public.events IS
    'CodeChat dissertation capture events. Course, group, assignment, condition, and task context are joined during analysis from participant/date mappings.';
COMMENT ON COLUMN public.events.user_id IS 'Pseudonymous participant UUID generated or supplied by the VS Code extension.';
COMMENT ON COLUMN public.events.session_id IS 'Capture session UUID emitted by the VS Code extension.';
COMMENT ON COLUMN public.events.file_hash IS 'SHA-256 hash of the local file path; raw local paths are not stored.';
COMMENT ON COLUMN public.events."timestamp" IS 'Server receive/record timestamp in UTC.';
COMMENT ON COLUMN public.events.data IS 'Event-specific JSON payload. Known telemetry metadata lives in typed columns.';

-- Least-privilege deployment guidance:
-- students or classroom machines should use a dedicated writer account, not a
-- database owner or administrator account. After replacing the placeholder
-- password/database/user names, a database administrator can grant only the
-- permissions needed for capture inserts:
--
-- CREATE ROLE codechat_capture_writer LOGIN PASSWORD 'replace-with-secret';
-- GRANT CONNECT ON DATABASE codechat_capture TO codechat_capture_writer;
-- GRANT USAGE ON SCHEMA public TO codechat_capture_writer;
-- GRANT INSERT ON public.events TO codechat_capture_writer;
-- GRANT USAGE ON SEQUENCE public.events_id_seq TO codechat_capture_writer;
--
-- Do not grant SELECT, UPDATE, DELETE, CREATE, or ownership privileges to the
-- writer account used in `capture_config.json`.

COMMIT;
