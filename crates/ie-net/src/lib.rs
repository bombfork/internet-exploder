//! # ie-net
//!
//! Networking stack for Internet Exploder.
//! Handles HTTP/1.1, HTTP/2, and TLS via rustls.
//! No background prefetch — every request is explicitly initiated.

pub mod client;

pub use client::Client;
