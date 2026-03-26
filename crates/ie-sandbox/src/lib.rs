//! # ie-sandbox
//!
//! Multi-process architecture and sandboxing.
//!
//! Process model:
//! - **Browser process**: UI, tab management, bookmarks, navigation
//! - **Renderer process**: one per tab — HTML parsing, CSS, layout, JS, painting
//! - **Network process**: single process handling all HTTP traffic
//!
//! Each renderer process runs in a restricted sandbox with minimal syscall access.
//! IPC between processes uses length-prefixed JSON messages over Unix domain sockets
//! (or named pipes on Windows).

pub mod channel;
pub mod error;
pub mod message;
pub mod process;

pub use channel::IpcChannel;
pub use error::IpcError;
pub use message::IpcMessage;
pub use process::{ChildHandle, ProcessKind, spawn_child, spawn_child_with_exe};
