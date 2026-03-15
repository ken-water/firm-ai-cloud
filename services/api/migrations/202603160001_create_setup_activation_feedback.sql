CREATE TABLE IF NOT EXISTS setup_activation_feedback_events (
    id BIGSERIAL PRIMARY KEY,
    step_key TEXT NOT NULL,
    template_key TEXT NULL,
    feedback_kind TEXT NOT NULL,
    comment TEXT NULL,
    context JSONB NOT NULL DEFAULT '{}'::jsonb,
    actor TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_setup_activation_feedback_step_created
    ON setup_activation_feedback_events (step_key, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_setup_activation_feedback_template_created
    ON setup_activation_feedback_events (template_key, created_at DESC);
