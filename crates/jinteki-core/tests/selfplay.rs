//! Self-play fuzz: random-walk bots on both sides, many seeds.
//!
//! This is the playable-milestone slice of the DESIGN.md differential suite:
//! - DP-3 (soundness): every enumerated action must be accepted by the executor.
//! - DP-6 (determinism): same seed => byte-identical logs.
//! - Structural invariants: cards live in exactly one zone; credits/clicks
//!   never negative; games terminate.

use jinteki_core::state::*;
use jinteki_core::types::*;
use jinteki_core::{process_command, random_walk_step};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn audit_zones(st: &GameState) {
    let mut seen = vec![0u32; st.cards.len()];
    let mut bump = |cid: Cid| seen[cid as usize] += 1;
    for i in 0..2 {
        for &c in &st.deck[i] {
            bump(c)
        }
        for &c in &st.hand[i] {
            bump(c)
        }
        for &c in &st.discard[i] {
            bump(c)
        }
        for &c in &st.scored[i] {
            bump(c)
        }
    }
    for (_, srv) in &st.servers {
        for &c in &srv.content {
            bump(c)
        }
        for &c in &srv.ices {
            bump(c)
        }
    }
    for &c in &st.rig.programs {
        bump(c)
    }
    for &c in &st.rig.hardware {
        bump(c)
    }
    for &c in &st.rig.resources {
        bump(c)
    }
    bump(st.identity[0]);
    bump(st.identity[1]);
    for (i, &n) in seen.iter().enumerate() {
        assert_eq!(
            n, 1,
            "card {i} ({}) is in {n} zones",
            st.card(i as Cid).title()
        );
    }
    assert!(st.credits(Side::Corp) >= 0 && st.credits(Side::Runner) >= 0);
    assert!(st.clicks(Side::Corp) >= 0 && st.clicks(Side::Runner) >= 0);
}

/// Play one full bot-vs-bot game; returns (finished, steps, log).
fn play_game(seed: u64, cap: usize) -> (bool, usize, String) {
    let mut st = GameState::new(seed);
    let mut rng_c = ChaCha8Rng::seed_from_u64(seed.wrapping_mul(2).wrapping_add(1));
    let mut rng_r = ChaCha8Rng::seed_from_u64(seed.wrapping_mul(2).wrapping_add(2));
    let mut steps = 0;
    while !st.game_over() && steps < cap {
        let mut acted = false;
        for side in [Side::Corp, Side::Runner] {
            let rng = match side {
                Side::Corp => &mut rng_c,
                Side::Runner => &mut rng_r,
            };
            if let Some(cmd) = random_walk_step(&st, side, rng) {
                // DP-3 soundness: enumerated implies executable.
                process_command(&mut st, side, cmd.clone()).unwrap_or_else(|e| {
                    panic!("seed {seed}: enumerated action rejected: {e} ({cmd:?})")
                });
                acted = true;
                steps += 1;
                audit_zones(&st);
                break;
            }
        }
        if !acted {
            panic!(
                "seed {seed}: deadlock at step {steps}: no side has any action \
                 (turn_state {:?}, active {:?}, run {:?}, prompts {})",
                st.turn_state,
                st.active,
                st.run.as_ref().map(|r| r.phase),
                st.prompts.len()
            );
        }
    }
    let log = st
        .log
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (st.game_over(), steps, log)
}

#[test]
fn selfplay_fuzz_many_seeds() {
    let games = 120;
    let cap = 4000;
    let mut finished = 0;
    for seed in 0..games {
        let (done, _steps, _log) = play_game(seed, cap);
        if done {
            finished += 1;
        }
    }
    assert!(
        finished * 10 >= games * 6,
        "too few games finished: {finished}/{games}"
    );
}

#[test]
fn selfplay_is_deterministic() {
    let (_, s1, log1) = play_game(424242, 4000);
    let (_, s2, log2) = play_game(424242, 4000);
    assert_eq!(s1, s2);
    assert_eq!(log1, log2, "same seed must produce identical logs");
}
