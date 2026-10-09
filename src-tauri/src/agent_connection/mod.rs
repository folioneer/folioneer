//! The agent connection (AGT spec, TODO-051): an interface beside the window, like the command
//! line. An agent client starts the program with `--mcp`; that bridge holds no data and
//! passes each tool call to the running application over a local channel (ADR-023). The
//! application asks its owner before serving a connection, and runs every tool through the
//! queries the window uses.

// Only the Unix channel serves connections today (AGT-023): on a system without a channel
// the application's side of it compiles and is not called.
#![cfg_attr(not(unix), allow(dead_code))]

#[cfg(feature = "app")]
pub mod api;
pub mod bridge;
pub mod channel;
pub mod connections;
mod definitions;
pub mod gate;
mod mcp;
pub mod server;
pub mod tools;
mod wire;

#[cfg(all(test, unix))]
mod end_to_end_tests;
