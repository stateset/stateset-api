-- Transactional Outbox table for reliable event delivery
CREATE TABLE IF NOT EXISTS outbox_events (
    id UUID PRIMARY KEY,
    aggregate_type VARCHAR(100) NOT NULL,
    aggregate_id UUID,
    event_type VARCHAR(200) NOT NULL,
    payload JSONB NOT NULL,
    headers JSONB,
    status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending|processing|delivered|failed
    attempts INTEGER NOT NULL DEFAULT 0,
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    error_message TEXT,
    partition_key VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_outbox_status_available ON outbox_events(status, available_at);
CREATE INDEX IF NOT EXISTS idx_outbox_created_at ON outbox_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_outbox_event_type ON outbox_events(event_type);
