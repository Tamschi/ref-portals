//TODO: clippy::cargo
#![warn(clippy::pedantic)]

pub mod rc;
pub mod sync;

const ANCHOR_DROPPED: &str = "Anchor dropped";
const ANCHOR_STILL_IN_USE: &str = "Anchor still in use";
