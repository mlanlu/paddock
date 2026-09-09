//! The app's logic, with no dependency on `tauri`.
//!
//! Everything the desktop app knows how to do lives here: where the `paddock`
//! binary is, what its JSON means, what its exit codes mean, and how to run it.
//! The Tauri crate above is a thin layer of `#[tauri::command]` wrappers.
//!
//! The split is not decoration. Agents develop paddock from inside a paddock
//! sandbox, which has no macOS and no WebKit, so a crate that depends on
//! `tauri` cannot even be compiled there. This one can, and is unit-tested.

pub mod contract;
pub mod exit;
pub mod proc;
pub mod resolve;

pub use contract::{Info, Policy, Port, Sandbox};
pub use exit::Exit;
pub use resolve::{Resolved, Source, SystemProbe};
