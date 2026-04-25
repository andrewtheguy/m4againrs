//! Bare-minimum AAC/M4A `global_gain` rewriter.
//!
//! Public entry points:
//!   - [`aac_apply_gain_file`] – path-based file rewrite.
//!   - [`aac_apply_gain_to_writer`] – seekable input → forward-only writer.

pub const GAIN_STEP_DB: f64 = 1.5;

mod aac;
mod aac_codebooks;
mod bits;
mod error;
mod gain;
mod mp4;

pub use error::{Error, Result};
pub use gain::{aac_apply_gain_file, aac_apply_gain_to_writer};
