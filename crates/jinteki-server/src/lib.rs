//! jinteki-server library surface: the game channels (local vs bot, jnet
//! bridge) and the native account/deck subsystem (ACCOUNTS-AND-DECKS.md).
//! `main.rs` is a thin shell over this so integration tests can assemble
//! the same router against an in-memory database.

pub mod api;
pub mod auth;
pub mod bridge;
pub mod carddata;
pub mod cr;
pub mod crlog;
pub mod db;
pub mod deckcheck;
pub mod decks;
pub mod guard;
pub mod lobby;
pub mod local;
pub mod mail;
pub mod nrdb;
pub mod transcript;
