//! # ie-render
//!
//! Rendering engine for Internet Exploder.
//! GPU-accelerated via wgpu, with software fallback for tests.

pub mod chrome;
pub mod gpu;
pub mod paint;
pub mod software;
pub mod text;

pub use chrome::{
    AddressBarOverlay, BookmarkEntry, BookmarkListOverlay, ChromeOverlay, TabEntry, TabListOverlay,
    build_chrome_display_list,
};
pub use gpu::GpuRenderer;
pub use paint::{Color, PaintCommand, build_display_list};
pub use software::{SoftwareTextMeasure, render_to_buffer};
pub use text::{GlyphonTextMeasure, TextRenderer};
