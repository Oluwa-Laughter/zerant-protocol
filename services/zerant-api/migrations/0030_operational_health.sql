CREATE TABLE IF NOT EXISTS maintenance_job_state (
    job TEXT PRIMARY KEY CHECK (job IN ('retention')),
    last_success_at TIMESTAMPTZ,
    last_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO maintenance_job_state(job)
VALUES ('retention')
ON CONFLICT (job) DO NOTHING;
