-- Native chat feature (Phase 1): organization_api_keys, chat_conversations,
-- chat_messages. See Questions.md for the full design writeup, especially
-- the encryption-at-rest approach and why `chat_messages.redacted_content`
-- means something slightly different for role='user' vs role='assistant'
-- rows.
--
-- organization_api_keys: one shared OpenAI key per organisation, provisioned
-- by a compliance_admin (Phase 3), never stored in plaintext (see
-- backend/src/crypto.rs — AES-256-GCM). `provider` is a TEXT CHECK rather
-- than a fixed set of columns so a second provider can be added later with a
-- migration that only widens this CHECK constraint, matching this
-- codebase's existing CHECK-constraint-as-enum convention (see
-- audit_log.decision, chat_messages.role/decision below).
CREATE TABLE organization_api_keys (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,
    provider          TEXT NOT NULL DEFAULT 'openai' CHECK (provider IN ('openai')),
    -- AES-256-GCM ciphertext (nonce || ciphertext, hex-encoded as one
    -- string) — see crypto.rs::encrypt_secret/decrypt_secret. Never the raw
    -- key, never logged.
    encrypted_key     TEXT NOT NULL,
    -- Last 4 characters of the real key, stored in the clear specifically so
    -- Phase 3's UI can render "sk-...ab12" without decrypting the real key
    -- just to display a confirmation string. A 4-character fragment on its
    -- own is not a meaningful secret.
    last_four         TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    rotated_at        TIMESTAMPTZ,
    created_by        UUID REFERENCES users(id),
    -- One key per (organisation, provider): rotation UPDATEs this row
    -- rather than inserting a second one, so "this organisation's OpenAI
    -- key" is always a single unambiguous lookup, never an ORDER BY/LIMIT 1
    -- guess among several.
    UNIQUE (organisation_id, provider)
);

CREATE INDEX idx_organization_api_keys_org_id ON organization_api_keys(organisation_id);

-- chat_conversations: one row per native-chat thread. `title` is nullable —
-- Phase 1 leaves auto-generated titling as a future enhancement, not
-- required for the feature to work end to end.
CREATE TABLE chat_conversations (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id   UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title             TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_chat_conversations_org_id ON chat_conversations(organisation_id);
CREATE INDEX idx_chat_conversations_user_id ON chat_conversations(user_id);

-- chat_messages: redacted content ONLY, matching this codebase's existing
-- zero-raw-prompt-storage principle (audit_log never stores a raw prompt,
-- only original_prompt_hash + redacted_prompt — see migration 0004/0015).
-- No `organisation_id` column here, deliberately, following the same
-- precedent as `sessions` (migration 0010's own note): a message's
-- organisation is always reachable via
-- chat_messages.conversation_id -> chat_conversations.organisation_id, and
-- nothing queries chat_messages directly by tenant without already having
-- the conversation row in hand — adding a column nothing reads independently
-- would just be another value that could drift out of sync.
--
-- `redacted_content` means something different depending on `role`, worth
-- stating plainly rather than leaving implicit: for role='user' it is
-- `scan_prompt`'s `redacted_prompt` — the user's raw message is NEVER
-- stored, matching the audit_log principle exactly. For role='assistant' it
-- is the AI's response text as returned by the provider, stored verbatim —
-- `scan_response` never redacts a response (see detection/scan.rs's own doc
-- comment on why: silently rewriting content already shown to the user is a
-- materially different, more concerning intervention than redacting an
-- outgoing prompt before it's sent). Unlike the browser extension (which
-- never needs to persist a response, since it's already rendered on the
-- third-party site's own page), this native chat surface IS the only place
-- responsible for redisplaying that response later, so it must be
-- retrievable — there is no "raw version" of an assistant message distinct
-- from what's stored here.
CREATE TABLE chat_messages (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id   UUID NOT NULL REFERENCES chat_conversations(id) ON DELETE CASCADE,
    role              TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    redacted_content  TEXT NOT NULL,
    -- Populated for role='user' (the prompt scan's outcome) and, once the
    -- async response scan completes, for role='assistant' too (the response
    -- scan's own risk_score) — NULL until then. Always NULL for a
    -- role='assistant' row before its response scan finishes.
    risk_score        REAL CHECK (risk_score IS NULL OR (risk_score >= 0 AND risk_score <= 1)),
    -- Only meaningful for role='user' rows (the prompt-scan decision) — a
    -- response scan has no forward/block decision to make (see
    -- detection::scan::scan_response's own doc comment), so this is always
    -- NULL on a role='assistant' row.
    decision          TEXT CHECK (decision IS NULL OR decision IN
                          ('cleared_no_entities', 'blocked_low_confidence',
                           'redacted_and_forwarded', 'redacted_low_confidence_review')),
    -- NULL until the async response scan completes (role='assistant' rows
    -- only) — see routes/chat.rs. Always NULL on a role='user' row.
    response_flagged  BOOLEAN,
    -- Correlates a role='assistant' row back to the exact audit_log row its
    -- turn's prompt scan created, so the background response-scan task
    -- (Phase 2) can update both tables without the browser extension's own
    -- "most recent scan id" correlation heuristic (see
    -- extension/content/response-scanner.js's own doc comment on that
    -- limitation) — the backend already knows the precise id here, so no
    -- guess is needed. NULL on role='user' rows until the corresponding
    -- audit_log INSERT completes within the same request.
    audit_log_id      UUID REFERENCES audit_log(id),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_chat_messages_conversation_id ON chat_messages(conversation_id);
CREATE INDEX idx_chat_messages_audit_log_id ON chat_messages(audit_log_id);
