//TODO: clippy::cargo
#![warn(clippy::pedantic, clippy::as_conversions, clippy::clone_on_ref_ptr)]

pub mod rc;
pub mod sync;

const ANCHOR_DROPPED: &str = "Anchor dropped";
const ANCHOR_STILL_IN_USE: &str = "Anchor still in use";
