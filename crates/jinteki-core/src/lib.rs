//! jinteki-core: a sans-IO Netrunner rules engine for the jinteki-rs playable milestone.
//!
//! Architecture notes (traceable to DESIGN.md):
//! - Cards are DATA: `carddb` holds printed stats plus declarative behavior enums
//!   (the denotation layer the future designer DSL compiles into).
//! - Effect sequencing is defunctionalized: pending decisions live in the prompt
//!   queue as `PromptContext` values, never as host-language continuations
//!   (DESIGN TBC-4, resolved here pragmatically for the playable milestone).
//! - All randomness flows through the seeded RNG held by `GameState` (ChaCha8,
//!   stable across platforms); no wall clock, no IO anywhere in this crate.
//! - The external surface mirrors jinteki.net: commands are jnet command strings
//!   with jnet-shaped args, and `view::render_state` emits jnet-shaped state JSON
//!   with per-viewpoint redaction, so the same UI drives this engine and the
//!   reference server through the bridge.

pub mod bot;
pub mod carddb;
pub mod engine;
pub mod printed;
pub mod runs;
pub mod state;
pub mod types;
pub mod view;

pub use bot::{enumerate_actions, random_walk_step, Action};
pub use engine::process_command;
pub use state::GameState;
pub use types::*;
