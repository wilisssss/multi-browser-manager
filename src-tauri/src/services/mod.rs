//! Business-logic layer (architecture recommendation A3).
//!
//! `commands/` stays the IPC layer: parse arguments, lock state, delegate.
//! Anything with real logic — especially disk moves, transactions and
//! crypto — lives here as plain functions over `&Connection` / paths, so it
//! is unit-testable without the Tauri runtime.

pub mod backup;
pub mod crypto;
pub mod resources;
pub mod trash;
pub mod usage;
