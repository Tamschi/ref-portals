//TODO: clippy::cargo, clippy::missing_docs_in_private_items
#![warn(
    clippy::as_conversions,
    clippy::clone_on_ref_ptr,
    clippy::fallible_impl_from,
    clippy::missing_const_for_fn,
    clippy::multiple_crate_versions,
    clippy::needless_borrow,
    clippy::pedantic,
    clippy::use_self,
    clippy::wrong_pub_self_convention
)]
#![allow(clippy::wildcard_imports)]
#![deny(clippy::wildcard_dependencies)]
// Debug cleanup. Uncomment before committing.
#![forbid(
    clippy::dbg_macro,
    clippy::print_stdout,
    clippy::todo,
    clippy::unimplemented
)]

pub mod rc;
pub mod sync;

const ANCHOR_DROPPED: &str = "Anchor dropped";
const ANCHOR_STILL_IN_USE: &str = "Anchor still in use";
