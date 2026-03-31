//! # ie-render
//!
//! Rendering engine for Internet Exploder.
//! Currently uses software rendering via softbuffer.
//! GPU rendering (wgpu) will be added later.

pub mod paint;
pub mod software;

pub use paint::{Color, PaintCommand, build_display_list};
pub use software::{SoftwareTextMeasure, render_to_buffer};
