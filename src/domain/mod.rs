//! Typed domain helpers shared by commands and views — the pieces the Python
//! version did with fragile string logic.
//!
//! * [`quantity`] — total CPU/memory parsing (replaces `convert_to_milicore` /
//!   `convert_to_mi`; migration.md O4).
//! * [`container_state`] — exhaustive container-state derivation (replaces the
//!   collapsed `or`-chain; O8).

pub mod container_state;
pub mod quantity;
