//! jinteki-cr — the Comprehensive Rules virtual machine (P1.5, SYS-F-9/10/11).
//!
//! The CR v26.03 (docs/rules/) implemented as such: timing structures are
//! data-driven step tables (§11), abilities are instruction procedures (§9),
//! priority windows are the scheduler (§9.2), the checkpoint (§10.3) is the
//! innermost loop over a change buffer, and interrupts/replacements (§9.9)
//! rewrite expected effects between imminence and resolution. Sans-IO: no
//! clock, no network; entropy only via the injected seeded RNG (ChaCha8).
//!
//! Every primitive cites its rule with `cite!("rule_id")`; the traceability
//! test validates all cited ids against docs/rules/cr-index.json (DP-7b).

#[macro_use]
pub mod cite;

pub mod ability;
pub mod change;
pub mod checkpoint;
pub mod decision;
pub mod effects;
pub mod frames;
pub mod instr;
pub mod lingering;
pub mod object;
pub mod testkit;
pub mod timing;
pub mod vm;
pub mod window;

/// Embedded sources for the static citation registry (see `cite`).
pub const EMBEDDED_SOURCES: &[(&str, &str)] = &[
    ("ability.rs", include_str!("ability.rs")),
    ("change.rs", include_str!("change.rs")),
    ("checkpoint.rs", include_str!("checkpoint.rs")),
    ("decision.rs", include_str!("decision.rs")),
    ("effects.rs", include_str!("effects.rs")),
    ("frames.rs", include_str!("frames.rs")),
    ("instr.rs", include_str!("instr.rs")),
    ("lingering.rs", include_str!("lingering.rs")),
    ("object.rs", include_str!("object.rs")),
    ("testkit.rs", include_str!("testkit.rs")),
    ("timing.rs", include_str!("timing.rs")),
    ("vm.rs", include_str!("vm.rs")),
    ("window.rs", include_str!("window.rs")),
];

pub use decision::{DecisionAnswer, DecisionSpec, GameResult, Yield};
pub use object::{ObjectId, ServerId, Side};
pub use vm::{GameSetup, Vm};
