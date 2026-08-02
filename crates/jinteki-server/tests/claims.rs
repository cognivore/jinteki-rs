//! Claim lifecycle, adoption/merge semantics, and the anon GC — the
//! SYS-A-3/A-5/A-6 verification matrix, against an in-memory SQLite.
//! Time travel is plain SQL: expiry rows are rewritten into the past.

use jinteki_server::auth::{self, VerifyOutcome};
use jinteki_server::db::{sha256_hex, Db};

fn db() -> Db {
    Db::open_in_memory().expect("in-memory db")
}

#[test]
fn token_is_hashed_at_rest_and_single_use() {
    let db = db();
    let conn = db.blocking_lock();
    let su = auth::mint_anon(&conn).unwrap();
    let raw = auth::create_claim(&conn, &su.session_id, &su.user_id, "a@example.com").unwrap();

    // At rest: only sha256(raw); the raw token appears nowhere in the DB.
    let stored: String = conn
        .query_row("SELECT token_hash FROM claims", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, sha256_hex(&raw));
    assert_ne!(stored, raw);

    // First presentation consumes and upgrades.
    let out = auth::verify_claim(&conn, &raw).unwrap();
    let VerifyOutcome::Ok { user_id, .. } = &out else {
        panic!("first verify succeeds, got {out:?}");
    };
    assert_eq!(user_id, &su.user_id, "case A: same user id, upgraded in place");

    // Second presentation: the row is gone.
    assert_eq!(auth::verify_claim(&conn, &raw).unwrap(), VerifyOutcome::Invalid);
    let n: i64 = conn.query_row("SELECT count(*) FROM claims", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn expired_token_is_burned_on_presentation() {
    let db = db();
    let conn = db.blocking_lock();
    let su = auth::mint_anon(&conn).unwrap();
    let raw = auth::create_claim(&conn, &su.session_id, &su.user_id, "a@example.com").unwrap();
    conn.execute(
        "UPDATE claims SET expires_at = datetime('now', '-1 minute')",
        [],
    )
    .unwrap();
    // Expired is distinct from invalid, and the row burns anyway
    // (draftroom auth.go:305-318).
    assert_eq!(auth::verify_claim(&conn, &raw).unwrap(), VerifyOutcome::Expired);
    let n: i64 = conn.query_row("SELECT count(*) FROM claims", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "expired presentation still consumed the row");
    assert_eq!(
        auth::verify_claim(&conn, &raw).unwrap(),
        VerifyOutcome::Invalid,
        "no retry after expiry"
    );
}

#[test]
fn reissue_tombstones_previous_claim_for_same_session_email() {
    let db = db();
    let conn = db.blocking_lock();
    let su = auth::mint_anon(&conn).unwrap();
    let raw1 = auth::create_claim(&conn, &su.session_id, &su.user_id, "a@example.com").unwrap();
    let _raw2 = auth::create_claim(&conn, &su.session_id, &su.user_id, "a@example.com").unwrap();
    let n: i64 = conn.query_row("SELECT count(*) FROM claims", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "one pending claim per (session, email)");
    assert_eq!(
        auth::verify_claim(&conn, &raw1).unwrap(),
        VerifyOutcome::Invalid,
        "the tombstoned token is dead"
    );
}

#[test]
fn upgrade_in_place_keeps_ownership_without_migration() {
    let db = db();
    let conn = db.blocking_lock();
    let su = auth::mint_anon(&conn).unwrap();
    // Anon user owns a deck before claiming.
    conn.execute(
        "INSERT INTO decks (id, owner_id, name, side, identity_title, cards_json,
                            created_at, updated_at)
         VALUES ('d1', ?1, 'mine', 'runner', 'x', '[]', datetime('now'), datetime('now'))",
        [&su.user_id],
    )
    .unwrap();
    let raw = auth::create_claim(&conn, &su.session_id, &su.user_id, "a@example.com").unwrap();
    auth::verify_claim(&conn, &raw).unwrap();
    let (kind, email): (String, String) = conn
        .query_row("SELECT kind, email FROM users WHERE id = ?1", [&su.user_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(kind, "claimed");
    assert_eq!(email, "a@example.com");
    let owner: String = conn
        .query_row("SELECT owner_id FROM decks WHERE id='d1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(owner, su.user_id, "adoption is a no-op: the deck never moved");
}

/// The full §4.6 merge: A's belongings move to E, A's sessions are DELETED
/// (not re-pointed), A tombstones, a merge row lands, and only the clicking
/// browser gets a session.
#[test]
fn merge_reassigns_belongings_and_deletes_anon_sessions() {
    let db = db();
    let conn = db.blocking_lock();

    // E: existing claimed account with its own session.
    let e = auth::mint_anon(&conn).unwrap();
    let raw_e = auth::create_claim(&conn, &e.session_id, &e.user_id, "owner@example.com").unwrap();
    let VerifyOutcome::Ok { session_id: e_session, .. } =
        auth::verify_claim(&conn, &raw_e).unwrap()
    else {
        panic!("E claims first");
    };

    // A: fresh anon on another device with a deck and a game.
    let a = auth::mint_anon(&conn).unwrap();
    conn.execute(
        "INSERT INTO decks (id, owner_id, name, side, identity_title, cards_json,
                            created_at, updated_at)
         VALUES ('da', ?1, 'anon deck', 'corp', 'x', '[]', datetime('now'), datetime('now'))",
        [&a.user_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO games (id, owner_id, side, started_at)
         VALUES ('ga', ?1, 'corp', datetime('now'))",
        [&a.user_id],
    )
    .unwrap();

    // A claims the same email → merge into E.
    let raw_a = auth::create_claim(&conn, &a.session_id, &a.user_id, "owner@example.com").unwrap();
    let VerifyOutcome::Ok { session_id: clicker_session, user_id: final_user } =
        auth::verify_claim(&conn, &raw_a).unwrap()
    else {
        panic!("merge verify succeeds");
    };
    assert_eq!(final_user, e.user_id, "the clicker lands on E");
    assert_ne!(clicker_session, a.session_id, "fresh session, never the pre-set one");

    // Belongings moved.
    let owner: String = conn
        .query_row("SELECT owner_id FROM decks WHERE id='da'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(owner, e.user_id);
    let gowner: String = conn
        .query_row("SELECT owner_id FROM games WHERE id='ga'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(gowner, e.user_id);

    // A tombstoned + audit merge row.
    let (kind, merged_into): (String, String) = conn
        .query_row(
            "SELECT kind, merged_into FROM users WHERE id = ?1",
            [&a.user_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(kind, "merged");
    assert_eq!(merged_into, e.user_id);
    let merges: i64 = conn
        .query_row(
            "SELECT count(*) FROM merges WHERE source_user_id=?1 AND target_user_id=?2",
            [&a.user_id, &e.user_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(merges, 1);

    // Hostile-claim closure: A's original session is GONE (deleted, not
    // re-pointed to E) — the requesting browser gains nothing.
    let a_sess: i64 = conn
        .query_row("SELECT count(*) FROM sessions WHERE id = ?1", [&a.session_id], |r| r.get(0))
        .unwrap();
    assert_eq!(a_sess, 0, "the anon requester's session must be deleted");
    // E's other sessions untouched; the clicker's fresh session exists.
    for sid in [&e_session, &clicker_session] {
        let n: i64 = conn
            .query_row("SELECT count(*) FROM sessions WHERE id = ?1", [sid], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
    // A merged user's dead session no longer validates (client re-bootstraps).
    assert!(auth::validate_session(&conn, &a.session_id).is_none());
}

#[test]
fn claimed_user_with_different_email_conflicts() {
    let db = db();
    let conn = db.blocking_lock();
    let su = auth::mint_anon(&conn).unwrap();
    let raw = auth::create_claim(&conn, &su.session_id, &su.user_id, "one@example.com").unwrap();
    auth::verify_claim(&conn, &raw).unwrap();
    // Now claimed as one@; a claim for two@ must refuse (§4.5 case C).
    let raw2 = auth::create_claim(&conn, &su.session_id, &su.user_id, "two@example.com").unwrap();
    assert_eq!(auth::verify_claim(&conn, &raw2).unwrap(), VerifyOutcome::Conflict);
    // And re-claiming the SAME email is a harmless re-login.
    let raw3 = auth::create_claim(&conn, &su.session_id, &su.user_id, "one@example.com").unwrap();
    assert!(matches!(auth::verify_claim(&conn, &raw3).unwrap(), VerifyOutcome::Ok { .. }));
}

#[test]
fn gc_prunes_idle_empty_anons_but_claims_and_decks_veto() {
    let db = db();
    let conn = db.blocking_lock();
    let idle = auth::mint_anon(&conn).unwrap(); // idle, empty → pruned
    let with_deck = auth::mint_anon(&conn).unwrap(); // idle but owns a deck → kept
    let with_claim = auth::mint_anon(&conn).unwrap(); // idle but mid-claim → kept
    conn.execute(
        "INSERT INTO decks (id, owner_id, name, side, identity_title, cards_json,
                            created_at, updated_at)
         VALUES ('dk', ?1, 'keep', 'runner', 'x', '[]', datetime('now'), datetime('now'))",
        [&with_deck.user_id],
    )
    .unwrap();
    auth::create_claim(&conn, &with_claim.session_id, &with_claim.user_id, "c@example.com")
        .unwrap();
    // Everyone idle for 91 days.
    conn.execute("UPDATE users SET last_seen_at = datetime('now', '-91 days')", []).unwrap();
    let pruned = auth::gc_sweep(&conn).unwrap();
    assert_eq!(pruned, 1, "only the empty idle anon goes");
    for (uid, expect) in [
        (&idle.user_id, 0i64),
        (&with_deck.user_id, 1),
        (&with_claim.user_id, 1),
    ] {
        let n: i64 = conn
            .query_row("SELECT count(*) FROM users WHERE id = ?1", [uid], |r| r.get(0))
            .unwrap();
        assert_eq!(n, expect, "GC verdict for {uid}");
    }
}

#[test]
fn email_normalization() {
    assert_eq!(
        auth::normalize_email("  Person Name <A.B@Example.COM> "),
        Some("a.b@example.com".into())
    );
    assert_eq!(auth::normalize_email("PLAIN@X.ORG"), Some("plain@x.org".into()));
    assert_eq!(auth::normalize_email("not-an-email"), None);
    assert_eq!(auth::normalize_email("a@nodot"), None);
    assert_eq!(auth::normalize_email("a b@x.org"), None);
}

#[test]
fn display_name_rules() {
    assert_eq!(auth::valid_display_name("  wyrm  "), Some("wyrm".into()));
    assert!(auth::valid_display_name("").is_none());
    assert!(auth::valid_display_name("   ").is_none());
    assert!(auth::valid_display_name("https://spam.example").is_none());
    assert!(auth::valid_display_name("</div>").is_none());
    assert!(auth::valid_display_name("123456789012345678901").is_none()); // 21 cp
    assert!(auth::valid_display_name("12345678901234567890").is_some()); // 20 cp
}
