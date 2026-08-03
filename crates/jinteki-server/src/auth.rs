//! Identity: anonymous sessions, magic-link claims, verify with
//! upgrade-or-merge, and the anon GC (ACCOUNTS-AND-DECKS.md §3-§4).
//!
//! All times are SQLite `datetime('now')` UTC strings; comparisons are
//! lexicographic, which ISO-8601 makes correct.

use crate::db::{audit, new_token, sha256_hex};
use rusqlite::{params, Connection, OptionalExtension};

/// Sliding-window session TTLs (§3.1): 14 days claimed (draftroom
/// `sessionTTL`), 400 days anonymous (the cookie IS the identity; expiring
/// it orphans the decks — draftroom guest-cookie precedent).
const TTL_CLAIMED: &str = "+14 days";
const TTL_ANON: &str = "+400 days";

#[derive(Debug, Clone)]
pub struct SessionUser {
    pub session_id: String,
    pub user_id: String,
    pub kind: String,
    pub display_name: String,
    pub email: Option<String>,
}

fn ttl_for(kind: &str) -> &'static str {
    if kind == "claimed" {
        TTL_CLAIMED
    } else {
        TTL_ANON
    }
}

/// Mint a fresh anonymous user + session. Returns the session cookie value.
pub fn mint_anon(conn: &Connection) -> rusqlite::Result<SessionUser> {
    let user_id = new_token();
    // guest-<4 hex> (§3.1) — derived from the id but hex-clean for display.
    let display = format!("guest-{}", &sha256_hex(&user_id)[..4]);
    conn.execute(
        "INSERT INTO users (id, kind, display_name, created_at, last_seen_at)
         VALUES (?1, 'anon', ?2, datetime('now'), datetime('now'))",
        params![user_id, display],
    )?;
    let session_id = mint_session(conn, &user_id, "anon")?;
    Ok(SessionUser {
        session_id,
        user_id,
        kind: "anon".into(),
        display_name: display,
        email: None,
    })
}

/// Mint a session row for an existing user.
pub fn mint_session(conn: &Connection, user_id: &str, kind: &str) -> rusqlite::Result<String> {
    let session_id = new_token();
    conn.execute(
        &format!(
            "INSERT INTO sessions (id, user_id, created_at, expires_at, last_activity_at)
             VALUES (?1, ?2, datetime('now'), datetime('now', '{}'), datetime('now'))",
            ttl_for(kind)
        ),
        params![session_id, user_id],
    )?;
    Ok(session_id)
}

/// Validate a session cookie value: live, unexpired, user not merged.
/// Applies the sliding-window touch at most once per hour (draftroom
/// `ValidateSession`, auth.go:353-395) and the coarse daily last_seen touch
/// the anon GC keys on.
pub fn validate_session(conn: &Connection, session_id: &str) -> Option<SessionUser> {
    let row = conn
        .query_row(
            "SELECT s.id, s.user_id, u.kind, u.display_name, u.email,
                    (s.last_activity_at < datetime('now', '-1 hour')) AS stale,
                    (u.last_seen_at < datetime('now', '-1 day')) AS seen_stale
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.id = ?1 AND s.expires_at > datetime('now')",
            [session_id],
            |r| {
                Ok((
                    SessionUser {
                        session_id: r.get(0)?,
                        user_id: r.get(1)?,
                        kind: r.get(2)?,
                        display_name: r.get(3)?,
                        email: r.get(4)?,
                    },
                    r.get::<_, bool>(5)?,
                    r.get::<_, bool>(6)?,
                ))
            },
        )
        .optional()
        .ok()??;
    let (su, stale, seen_stale) = row;
    // A merged/suspended user's session is dead: the client re-bootstraps
    // as a fresh anon (§4.6 "stale tab" row).
    if su.kind == "merged" || su.kind == "suspended" {
        return None;
    }
    if stale {
        let _ = conn.execute(
            &format!(
                "UPDATE sessions SET last_activity_at = datetime('now'),
                        expires_at = datetime('now', '{}')
                 WHERE id = ?1",
                ttl_for(&su.kind)
            ),
            [session_id],
        );
    }
    if seen_stale {
        let _ = conn.execute(
            "UPDATE users SET last_seen_at = datetime('now') WHERE id = ?1",
            [&su.user_id],
        );
    }
    Some(su)
}

pub fn delete_session(conn: &Connection, session_id: &str) {
    let _ = conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id]);
}

/// Lowercase-normalize an email, stripping a `Name <a@b>` wrapper
/// (draftroom `normaliseEmail`, auth.go:119-130). Returns None if the
/// result does not look like an address.
pub fn normalize_email(raw: &str) -> Option<String> {
    let mut s = raw.trim();
    if let (Some(lt), Some(gt)) = (s.rfind('<'), s.rfind('>')) {
        if lt < gt {
            s = s[lt + 1..gt].trim();
        }
    }
    let s = s.to_lowercase();
    let at = s.find('@')?;
    if at == 0 || at + 1 >= s.len() || !s[at + 1..].contains('.') || s.contains(char::is_whitespace)
    {
        return None;
    }
    Some(s)
}

/// Create a claim: one pending claim per (session, email) — issuing a new
/// one tombstones the old (§4.2). Returns the RAW link token; the caller
/// mails it and must never store or echo it.
pub fn create_claim(
    conn: &Connection,
    session_id: &str,
    user_id: &str,
    email: &str,
) -> rusqlite::Result<String> {
    let token = new_token();
    let hash = sha256_hex(&token);
    conn.execute(
        "DELETE FROM claims WHERE session_id = ?1 AND email = ?2",
        params![session_id, email],
    )?;
    conn.execute(
        "INSERT INTO claims (token_hash, session_id, user_id, email, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now', '+30 minutes'))",
        params![hash, session_id, user_id, email],
    )?;
    audit(
        conn,
        Some(user_id),
        "claim_requested",
        &serde_json::json!({ "email_hash": &sha256_hex(email)[..16] }),
    );
    Ok(token)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// Session cookie value for the clicking browser, and the resulting user.
    Ok { session_id: String, user_id: String },
    Invalid,
    Expired,
    /// Requesting user is already claimed under a different email (§4.5 C).
    Conflict,
}

/// Consume a magic-link token: burn-on-presentation (expired rows are
/// deleted too — draftroom auth.go:305-318), then upgrade (case A), merge
/// (case B) or refuse (case C), minting a fresh session ONLY for the
/// clicking browser (session-fixation hygiene + the hostile-claim rule,
/// §4.5-§4.6). One transaction.
pub fn verify_claim(conn: &Connection, raw_token: &str) -> rusqlite::Result<VerifyOutcome> {
    let hash = sha256_hex(raw_token);
    let tx = conn.unchecked_transaction()?;
    // Single-use consume: delete first, act on the returned row.
    let claim = tx
        .query_row(
            "DELETE FROM claims WHERE token_hash = ?1
             RETURNING session_id, user_id, email,
                       (expires_at <= datetime('now')) AS expired",
            [&hash],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((_claim_session, claimer_id, email, expired)) = claim else {
        tx.commit()?;
        return Ok(VerifyOutcome::Invalid);
    };
    if expired {
        // Burned anyway; "expired" is distinct from "invalid" (§4.2).
        tx.commit()?;
        return Ok(VerifyOutcome::Expired);
    }

    let claimer: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT kind, email FROM users WHERE id = ?1",
            [&claimer_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((claimer_kind, claimer_email)) = claimer else {
        tx.commit()?;
        return Ok(VerifyOutcome::Invalid);
    };
    if claimer_kind == "merged" || claimer_kind == "suspended" {
        tx.commit()?;
        return Ok(VerifyOutcome::Invalid);
    }
    // Case C: a claimed account cannot side-effect an email change (§4.5).
    if claimer_kind == "claimed" && claimer_email.as_deref() != Some(email.as_str()) {
        tx.commit()?;
        return Ok(VerifyOutcome::Conflict);
    }

    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT id, kind FROM users WHERE email = ?1",
            [&email],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let (final_user, case) = match existing {
        None => {
            // Case A — first claim: upgrade in place. The user keeps their
            // id; every deck and game row already points at it (§4.5 A).
            tx.execute(
                "UPDATE users SET kind='claimed', email=?1, email_verified_at=datetime('now')
                 WHERE id = ?2",
                params![email, claimer_id],
            )?;
            (claimer_id.clone(), "upgrade")
        }
        Some((target_id, _)) if target_id == claimer_id => {
            // Re-claiming your own account — harmless re-login (§4.5 B note).
            (claimer_id.clone(), "reclaim")
        }
        Some((target_id, target_kind)) => {
            if target_kind == "suspended" {
                // A suspended account refuses login; indistinguishable from
                // a dead link for the outside observer.
                tx.commit()?;
                return Ok(VerifyOutcome::Invalid);
            }
            // Case B — merge anon A into existing E (§4.6). A's sessions are
            // DELETED, not re-pointed: only the clicking browser (inbox
            // possession) ends up signed in to E — this closes the
            // hostile-claim fixation entirely.
            tx.execute(
                "UPDATE decks SET owner_id = ?1 WHERE owner_id = ?2",
                params![target_id, claimer_id],
            )?;
            tx.execute(
                "UPDATE games SET owner_id = ?1 WHERE owner_id = ?2",
                params![target_id, claimer_id],
            )?;
            tx.execute("DELETE FROM sessions WHERE user_id = ?1", [&claimer_id])?;
            tx.execute(
                "UPDATE users SET kind='merged', merged_into=?1, merged_at=datetime('now')
                 WHERE id = ?2",
                params![target_id, claimer_id],
            )?;
            tx.execute(
                "INSERT INTO merges (id, source_user_id, target_user_id, at, via)
                 VALUES (?1, ?2, ?3, datetime('now'), 'claim')",
                params![new_token(), claimer_id, target_id],
            )?;
            (target_id, "merge")
        }
    };

    // Fresh session for the clicking browser only (§4.5 step 3).
    let kind: String = tx.query_row("SELECT kind FROM users WHERE id = ?1", [&final_user], |r| {
        r.get(0)
    })?;
    let session_id = new_token();
    tx.execute(
        &format!(
            "INSERT INTO sessions (id, user_id, created_at, expires_at, last_activity_at)
             VALUES (?1, ?2, datetime('now'), datetime('now', '{}'), datetime('now'))",
            ttl_for(&kind)
        ),
        params![session_id, final_user],
    )?;
    audit(
        &tx,
        Some(&final_user),
        "claim_verified",
        &serde_json::json!({ "email_hash": &sha256_hex(&email)[..16], "case": case }),
    );
    tx.commit()?;
    Ok(VerifyOutcome::Ok { session_id, user_id: final_user })
}

/// The name a user is known by — what a lobby row and a seat label show.
/// Missing user (deleted, or a cookieless visitor) has no name to show.
pub fn display_name(conn: &Connection, user_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT display_name FROM users WHERE id = ?1",
        params![user_id],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

/// Display-name rules (§3.3, DESIGN.md §B.10): non-empty after trim, at
/// most 20 code points, no "://", no "</".
pub fn valid_display_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > 20
        || name.contains("://")
        || name.contains("</")
    {
        return None;
    }
    Some(name.to_string())
}

/// Daily sweep (§5.2): expired sessions and claims go; anonymous users idle
/// for 90 days with no decks, no games and NO LIVE CLAIM go with their
/// sessions. Ownership emptiness is a GC criterion; claim state is a GC
/// veto — both halves of the Cubehall 2026-06-11 lesson (SYS-A-6).
pub fn gc_sweep(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM sessions WHERE expires_at <= datetime('now')", [])?;
    conn.execute("DELETE FROM claims WHERE expires_at <= datetime('now', '-1 day')", [])?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM sessions WHERE user_id IN (
           SELECT u.id FROM users u
           WHERE u.kind = 'anon' AND u.last_seen_at < datetime('now', '-90 days')
             AND NOT EXISTS (SELECT 1 FROM decks d WHERE d.owner_id = u.id)
             AND NOT EXISTS (SELECT 1 FROM games g WHERE g.owner_id = u.id)
             AND NOT EXISTS (SELECT 1 FROM claims c WHERE c.user_id = u.id))",
        [],
    )?;
    let n = tx.execute(
        "DELETE FROM users WHERE kind = 'anon' AND last_seen_at < datetime('now', '-90 days')
           AND NOT EXISTS (SELECT 1 FROM decks d WHERE d.owner_id = users.id)
           AND NOT EXISTS (SELECT 1 FROM games g WHERE g.owner_id = users.id)
           AND NOT EXISTS (SELECT 1 FROM claims c WHERE c.user_id = users.id)",
        [],
    )?;
    if n > 0 {
        audit(&tx, None, "gc_anon", &serde_json::json!({ "pruned": n }));
    }
    tx.commit()?;
    Ok(n)
}

/// System user that adopts games of deleted accounts and owns the seeded
/// starter decks (§6.4, §12.1). Created idempotently at boot.
pub const SYSTEM_USER_ID: &str = "system";

pub fn ensure_system_user(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO users (id, kind, display_name, created_at, last_seen_at)
         VALUES (?1, 'claimed', 'jinteki-rs', datetime('now'), datetime('now'))",
        [SYSTEM_USER_ID],
    )?;
    Ok(())
}

/// Account deletion (§12.1): sessions and claims deleted; decks deleted
/// (forks survive independently); games re-pointed to the system user;
/// users row reduced to a tombstone with the email NULLed.
pub fn delete_account(conn: &Connection, user_id: &str) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])?;
    tx.execute("DELETE FROM claims WHERE user_id = ?1", [user_id])?;
    tx.execute("DELETE FROM decks WHERE owner_id = ?1", [user_id])?;
    tx.execute(
        "UPDATE games SET owner_id = ?1 WHERE owner_id = ?2",
        params![SYSTEM_USER_ID, user_id],
    )?;
    tx.execute(
        "UPDATE users SET kind='merged', email=NULL, email_verified_at=NULL,
                display_name='deleted', merged_into=NULL, merged_at=datetime('now')
         WHERE id = ?1",
        [user_id],
    )?;
    audit(&tx, Some(user_id), "account_deleted", &serde_json::json!({}));
    tx.commit()?;
    Ok(())
}
