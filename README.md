# ref-portals

[![Latest Version](https://img.shields.io/crates/v/ref-portals.svg)](https://crates.io/crates/ref-portals)
[![docs.rs](https://docs.rs/ref-portals/badge.svg?version=1.0.0-beta.2)](https://docs.rs/ref-portals/1.0.0-beta.2/ref_portals/)

Safely use (stack) references outside their original scope.

This library provides convenient runtime-checked out-of-scope handles that are:

- `!Send + !Sync` or (dependently) `Send`/`Sync`,
- immutable or mutable and
- target `Sync` or `!Sync` values.

Please see the documentation for more information about which to choose.

## Example

```rust
use ref_portals::rc::Anchor;

let x = "Scoped".to_owned();
let anchor = Anchor::new(&x);
let self_owned: Box<dyn Fn() + 'static> = Box::new({
    let portal = anchor.portal();
    move || println!("{}", *portal)
});

self_owned(); // Scoped
```

Note that dropping `anchor` before `self_owned` would still cause a freeze here (which is the only way to reliably prevent UB in this situation).  
You can use weak portals to work around this:

```rust
use ref_portals::rc::Anchor;

let x = "Scoped".to_owned();
let anchor = Anchor::new(&x);
let eternal: &'static dyn Fn() = Box::leak(Box::new({
    let weak_portal = anchor.weak_portal();
    move || println!(
        "{}",
        // Panics iff the anchor has been dropped.
        *weak_portal.upgrade(),
    )
}));

eternal(); // Scoped
```

Mutable anchors can panic on drop instead of freezing the thread if the resource is not currently in use. Immutable anchors are more performant but lack the necessary guards to distinguish whether values are actively in use.

Anchors in the `sync` module will block instead of freezing the thread, but that may still cause a (in that case silent) deadlock anyway. Anchors from the `rc` module use `log::error!` to make freezes visible.

## Versioning

`ref-portals` strictly follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) with the following exceptions:

- The minor version will not reset to 0 on major version changes.  
Consider it the global feature level.
- The patch version will not reset to 0 on major or minor version changes.  
Consider it the global patch level.

## Why

This crate is likely extremely niche, but it probably can be useful to experimentally figure out if your logic is sound overall. Any potential UB from lifetime extensions is mapped onto panics or freezes/deadlocks, which at least won't lead to data corruption.
