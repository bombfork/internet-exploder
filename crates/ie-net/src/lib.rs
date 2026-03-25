//! # ie-net
//!
//! Networking stack for Internet Exploder.
//! Handles HTTP/1.1, HTTP/2, and TLS via rustls.
//! No background prefetch — every request is explicitly initiated.

pub mod client;
pub mod error;
pub mod response;

pub use client::Client;
pub use error::NetError;
pub use response::Response;
