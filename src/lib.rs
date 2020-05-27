//! Safely use (stack) references outside their original scope.

#![doc(html_root_url = "https://docs.rs/ref-portals/1.0.0-beta.1")]
#![doc(test(no_crate_inject))]
#![warn(
    clippy::as_conversions,
    clippy::cargo,
    clippy::clone_on_ref_ptr,
    clippy::fallible_impl_from,
    clippy::missing_const_for_fn,
    clippy::missing_docs_in_private_items,
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

//! # Example
//!
//! ```rust
//! use ref_portals::rc::Anchor;
//! 
//! let x = "Scoped".to_owned();
//! let anchor = Anchor::new(&x);
//! let self_owned: Box<dyn Fn() + 'static> = Box::new({
//!     let portal = anchor.portal();
//!     move || println!("{}", *portal)
//! });
//! 
//! self_owned(); // Scoped
//! ```
//! 
//! Note that dropping `anchor` before `self_owned` would still cause a panic here.  
//! You can use weak portals to work around this:
//!
//! ```rust
//! use ref_portals::rc::Anchor;
//! 
//! let x = "Scoped".to_owned();
//! let anchor = Anchor::new(&x);
//! let eternal: &'static dyn Fn() = Box::leak(Box::new({
//!     let weak_portal = anchor.weak_portal();
//!     move || println!(
//!         "{}",
//!         *weak_portal.upgrade(), // Panics iff the anchor has been dropped.
//!     )
//! }));
//! 
//! eternal(); // Scoped
//! ```
//!
//! # Notes
//! 
//! Panic assertions in this documentation use [assert_panic](https://crates.io/crates/assert-panic).

pub mod rc;
pub mod sync;

/// Panicked when upgrading weak portals iff the anchor has been destroyed already.
const ANCHOR_DROPPED: &str = "Anchor dropped";

/// Panicked when borrowing through a portal or dropping an anchor if the anchor has been poisoned.
/// Only mutable anchors can be poisoned.
const ANCHOR_POISONED: &str = "Anchor poisoned";

/// Panicked when dropping an anchor if any (strong) portals still exist.
const ANCHOR_STILL_IN_USE: &str = "Anchor still in use (at least one portal exists)";
