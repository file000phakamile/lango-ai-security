//! Cross-tenant isolation for the native chat feature's new tables
//! (Phase 1 of the chat feature: schema only — `routes::chat` and the
//! organisation API-key management routes don't exist yet, see Phase 2/3).
//! Same discipline as `tests/multi_tenant_isolation.rs`: a second, fully
//! independent organisation with its own users and data, proving a caller
//! scoped to organisation A can never read organisation B's chat
//! conversations, chat messages, or OpenAI API key — no exceptions.
//!
//! Since the route handlers these tables will serve don't exist until
//! Phase 2/3, this file exercises the exact org-scoped query SHAPES those
//! handlers will use (see routes::chat and routes::organization_api_keys
//! once they exist) directly against the schema, proving the isolation
//! guarantee is structurally sound ahead of the handlers being wired up.
//! Phase 2/3 add their own additional handler-level isolation coverage on
//! top of this, the same way every other route in this codebase has its own
//! dedicated isolation test in `multi_tenant_isolation.rs`.
//!
//! Uses `#[sqlx::test]`: each test function gets a freshly migrated,
//! throwaway Postgres database. Run with
//! `cargo test --test chat_multi_tenant_isolation` (requires a real
//! Postgres reachable via `DATABASE_URL`).

use sqlx::PgPool;
use uuid::Uuid;

fn test_encryption_key() -> String {
    "a".repeat(64)
}

async fn insert_org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("insert organisation")
}

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, role: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', 'General', $2, $3, now(), 'v1') RETURNING id",
    )
    .bind(email)
    .bind(role)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn insert_api_key(pool: &PgPool, org_id: Uuid, plaintext: &str, created_by: Uuid) {
    let encrypted = lango_backend::crypto::encrypt_secret(plaintext, &test_encryption_key()).unwrap();
    let last_four = lango_backend::crypto::last_four(plaintext);
    sqlx::query(
        "INSERT INTO organization_api_keys (organisation_id, provider, encrypted_key, last_four, created_by) \
         VALUES ($1, 'openai', $2, $3, $4)",
    )
    .bind(org_id)
    .bind(encrypted)
    .bind(last_four)
    .bind(created_by)
    .execute(pool)
    .await
    .expect("insert organization_api_keys row");
}

async fn insert_conversation(pool: &PgPool, org_id: Uuid, user_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO chat_conversations (organisation_id, user_id, title) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(org_id)
    .bind(user_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .expect("insert chat_conversations row")
}

async fn insert_message(pool: &PgPool, conversation_id: Uuid, role: &str, redacted_content: &str) {
    sqlx::query(
        "INSERT INTO chat_messages (conversation_id, role, redacted_content) VALUES ($1, $2, $3)",
    )
    .bind(conversation_id)
    .bind(role)
    .bind(redacted_content)
    .execute(pool)
    .await
    .expect("insert chat_messages row");
}

/// Mirrors the org-scoped "does this API key belong to my organisation"
/// lookup a real `GET`/`POST` handler under `routes::organization_api_keys`
/// will run (Phase 3).
async fn load_api_key_for_org(pool: &PgPool, org_id: Uuid) -> Option<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT encrypted_key, last_four FROM organization_api_keys WHERE organisation_id = $1 AND provider = 'openai'",
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .expect("query organization_api_keys")
}

/// Mirrors the org+user-scoped conversation-ownership check `routes::chat`
/// will run before ever touching a conversation's messages (Phase 2) — see
/// `routes/response_scan.rs`'s identical "real WHERE clause, not just a role
/// check" ownership-check pattern this deliberately copies.
async fn conversation_belongs_to(pool: &PgPool, conversation_id: Uuid, org_id: Uuid, user_id: Uuid) -> bool {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM chat_conversations WHERE id = $1 AND organisation_id = $2 AND user_id = $3",
    )
    .bind(conversation_id)
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .expect("query chat_conversations")
    .is_some()
}

/// Mirrors the org-scoped conversation list `routes::chat` will run for
/// "my conversations" (Phase 2) — user+org scoped, since a native chat
/// conversation is private to the user who started it, not org-wide visible
/// the way audit_log rows are (see Questions.md's judgment call on this:
/// compliance oversight of chat content happens via the existing Audit Log
/// dashboard view, which every chat turn also writes to, not by one user
/// browsing another user's live conversation).
async fn list_conversation_titles_for(pool: &PgPool, org_id: Uuid, user_id: Uuid) -> Vec<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT title FROM chat_conversations WHERE organisation_id = $1 AND user_id = $2 ORDER BY created_at",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .expect("query chat_conversations")
}

async fn list_message_content_for(pool: &PgPool, conversation_id: Uuid) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT redacted_content FROM chat_messages WHERE conversation_id = $1 ORDER BY created_at",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await
    .expect("query chat_messages")
}

#[sqlx::test]
async fn organisation_api_key_is_never_visible_to_another_organisation(pool: PgPool) {
    let org_a = insert_org(&pool, "Chat Isolation Org A").await;
    let org_b = insert_org(&pool, "Chat Isolation Org B").await;
    let admin_a = insert_user(&pool, org_a, "admin@chat-org-a.test", "compliance_admin").await;

    insert_api_key(&pool, org_a, "sk-org-a-real-secret-key-0000000000", admin_a).await;

    let org_a_key = load_api_key_for_org(&pool, org_a).await;
    assert!(org_a_key.is_some(), "org A must see its own key");
    let (encrypted, last_four) = org_a_key.unwrap();
    assert!(!encrypted.contains("sk-org-a"), "stored value must be ciphertext, never the raw key");
    assert_eq!(last_four, "0000");

    // The real assertion: org B, which never provisioned a key, must get
    // NOTHING back for its own organisation_id — not org A's key, not any
    // partial leak of it.
    let org_b_key = load_api_key_for_org(&pool, org_b).await;
    assert!(org_b_key.is_none(), "org B must never see org A's OpenAI key");
}

#[sqlx::test]
async fn chat_conversations_are_never_visible_across_organisations(pool: PgPool) {
    let org_a = insert_org(&pool, "Chat Isolation Org A").await;
    let org_b = insert_org(&pool, "Chat Isolation Org B").await;
    let user_a = insert_user(&pool, org_a, "user@chat-org-a.test", "staff").await;
    let user_b = insert_user(&pool, org_b, "user@chat-org-b.test", "staff").await;

    insert_conversation(&pool, org_a, user_a, "ORG_A_CONVERSATION").await;
    let conv_b = insert_conversation(&pool, org_b, user_b, "ORG_B_CONVERSATION").await;

    let org_a_titles = list_conversation_titles_for(&pool, org_a, user_a).await;
    assert_eq!(org_a_titles, vec![Some("ORG_A_CONVERSATION".to_string())]);
    assert!(
        !org_a_titles.contains(&Some("ORG_B_CONVERSATION".to_string())),
        "org A's conversation list must never contain org B's conversation"
    );

    // Org A's admin cannot pass off org B's conversation id as their own,
    // even if they somehow learned the UUID (e.g. guessed it, or it leaked
    // in a log) — the ownership check is a real WHERE clause on BOTH
    // organisation_id and user_id, not merely "does this row exist".
    let can_org_a_user_access_org_b_conversation =
        conversation_belongs_to(&pool, conv_b, org_a, user_a).await;
    assert!(
        !can_org_a_user_access_org_b_conversation,
        "org A's user must never be able to claim ownership of org B's conversation"
    );

    // And the legitimate owner still can.
    let can_org_b_user_access_own_conversation =
        conversation_belongs_to(&pool, conv_b, org_b, user_b).await;
    assert!(can_org_b_user_access_own_conversation);
}

#[sqlx::test]
async fn chat_messages_redacted_content_is_never_visible_across_organisations(pool: PgPool) {
    let org_a = insert_org(&pool, "Chat Isolation Org A").await;
    let org_b = insert_org(&pool, "Chat Isolation Org B").await;
    let user_a = insert_user(&pool, org_a, "user@chat-org-a.test", "staff").await;
    let user_b = insert_user(&pool, org_b, "user@chat-org-b.test", "staff").await;

    let conv_a = insert_conversation(&pool, org_a, user_a, "Org A thread").await;
    let conv_b = insert_conversation(&pool, org_b, user_b, "Org B thread").await;

    insert_message(&pool, conv_a, "user", "[REDACTED:NATIONAL_ID] — ORG_A_MESSAGE").await;
    insert_message(&pool, conv_a, "assistant", "Sure, I can help with that. ORG_A_REPLY").await;
    insert_message(&pool, conv_b, "user", "ORG_B_MESSAGE, completely unrelated").await;

    let org_a_messages = list_message_content_for(&pool, conv_a).await;
    assert_eq!(org_a_messages.len(), 2);
    assert!(org_a_messages.iter().any(|m| m.contains("ORG_A_MESSAGE")));
    assert!(org_a_messages.iter().any(|m| m.contains("ORG_A_REPLY")));
    assert!(
        !org_a_messages.iter().any(|m| m.contains("ORG_B_MESSAGE")),
        "org A's conversation must never contain org B's message content"
    );

    // A caller must own the conversation (checked via conversation_belongs_to
    // above) before this query ever runs in a real handler — this second
    // assertion proves that even querying org B's conversation id directly
    // returns ONLY org B's content, i.e. there is no cross-conversation
    // bleed at the chat_messages level either.
    let org_b_messages = list_message_content_for(&pool, conv_b).await;
    assert_eq!(org_b_messages, vec!["ORG_B_MESSAGE, completely unrelated".to_string()]);
}

#[sqlx::test]
async fn a_second_openai_key_for_the_same_organisation_and_provider_is_rejected(pool: PgPool) {
    // Locks in the UNIQUE (organisation_id, provider) constraint (migration
    // 0017) — rotation must UPDATE the existing row (Phase 3), never leave
    // two ambiguous rows for the same organisation+provider behind.
    let org_a = insert_org(&pool, "Chat Isolation Org A").await;
    let admin_a = insert_user(&pool, org_a, "admin@chat-org-a.test", "compliance_admin").await;
    insert_api_key(&pool, org_a, "sk-first-key-0000000000000000000000", admin_a).await;

    let encrypted = lango_backend::crypto::encrypt_secret("sk-second-key-1111111111111111111111", &test_encryption_key()).unwrap();
    let result = sqlx::query(
        "INSERT INTO organization_api_keys (organisation_id, provider, encrypted_key, last_four, created_by) \
         VALUES ($1, 'openai', $2, '1111', $3)",
    )
    .bind(org_a)
    .bind(encrypted)
    .bind(admin_a)
    .execute(&pool)
    .await;

    assert!(result.is_err(), "a second openai key row for the same organisation must be rejected");
}
