//! Each submodule is named after a color space, and contains color
//! conversion matrices for that space.
//!
//! Every submodule contains the following matrices:
//! - `TO_XYZ` and `FROM_XYZ`, which convert to/from CIE 1931 XYZ.
//! - `TO_XYZ_D65` and `FROM_XYZ_D65`, which convert to/from a
//!    non-standard XYZ space with a D65 whitepoint.
//! - `TO_ACES` and `FROM_ACES`, which convert to/from ACES2065-1.

// Generated conversion matrices.
include!(concat!(env!("OUT_DIR"), "/rec709_inc.rs"));
include!(concat!(env!("OUT_DIR"), "/rec2020_inc.rs"));
include!(concat!(env!("OUT_DIR"), "/aces_2065_1_inc.rs"));
include!(concat!(env!("OUT_DIR"), "/aces_cg_inc.rs"));
