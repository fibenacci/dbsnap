//! One module per subcommand. Each exposes a single `run` entry point that
//! wires the engine and renders the result; no domain logic lives here.

pub mod commit;
pub mod diff;
pub mod export;
pub mod init;
pub mod log;
pub mod report;
pub mod status;
pub mod verify;
