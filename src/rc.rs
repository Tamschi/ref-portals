//! Single-threaded anchors and portals.  
//! These don't implement `Send` or `Sync`, but are more efficient for use cases where that's not needed.

use {
    crate::{ANCHOR_DROPPED, ANCHOR_POISONED, ANCHOR_STILL_IN_USE},
    log::error,
    std::{
        borrow::Borrow,
        cell::{Ref, RefCell, RefMut},
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        panic::{RefUnwindSafe, UnwindSafe},
        ptr::NonNull,
        rc::{Rc, Weak},
        sync::Mutex, // Only to deadlock.
        thread,
    },
    wyz::pipe::*,
};

/// Poison helper for `!Send` mutable anchors.
#[derive(Debug)]
struct Poisonable<T> {
    pointer: T,
    poisoned: bool,
}

/// An `!Send` immutable anchor.  
/// Use this to capture shared references in a single-threaded environment.
///
/// # Deadlocks
///
/// On drop, if any associated `Portal`s exist:
///
/// ```rust
/// # use {assert_deadlock::assert_deadlock, std::time::Duration};
/// use ref_portals::rc::Anchor;
///
/// let mut x = "Scoped".to_owned();
/// let anchor = Anchor::new(&mut x);
/// let portal = anchor.portal();
///
/// assert_deadlock!(drop(anchor), Duration::from_secs(1));
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct Anchor<'a, T: ?Sized> {
    /// Internal pointer to the target of the captured reference.
    reference: ManuallyDrop<Rc<NonNull<T>>>,

    /// Act as sharing borrower.
    _phantom: PhantomData<&'a T>,
}

/// An `!Send` mutable anchor with overlapping immutable borrows.
/// Use this to capture mutable references in a single-threaded environment.
///
/// # Deadlocks
///
/// Iff there is a currently active borrow, then dropping this anchor will cause a deadlock as last resort measure to prevent UB:
///
/// ```rust
/// # use {assert_deadlock::assert_deadlock, std::time::Duration};
/// use ref_portals::rc::RwAnchor;
///
/// let mut x = "Scoped".to_owned();
/// let anchor = RwAnchor::new(&mut x);
/// let portal = anchor.portal();
/// let _guard = portal.borrow();
///
/// assert_deadlock!(drop(anchor), Duration::from_secs(1));
/// ```
///
/// # Panics
///
/// On drop, if any associated `RwPortal`s exist:
///
/// ```rust
/// # use assert_panic::assert_panic;
/// use ref_portals::rc::RwAnchor;
///
/// let mut x = "Scoped".to_owned();
/// let anchor = RwAnchor::new(&mut x);
/// Box::leak(Box::new(anchor.portal()));
///
/// assert_panic!(
///     drop(anchor),
///     &str,
///     "Anchor still in use (at least one portal exists)",
/// );
/// ```
///
/// Otherwise, on drop, iff the anchor has been poisoned:
///
/// ```rust
/// # use assert_panic::assert_panic;
/// use ref_portals::rc::RwAnchor;
///
/// let mut x = "Scoped".to_owned();
/// let anchor = RwAnchor::new(&mut x);
/// {
///     let portal = anchor.portal();
///     assert_panic!({
///         let guard = portal.borrow_mut();
///         panic!()
///     });
/// }
///
/// assert_panic!(
///     drop(anchor),
///     &str,
///     "Anchor poisoned",
/// );
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct RwAnchor<'a, T: ?Sized> {
    /// Internal pointer to the target of the captured reference.
    reference: ManuallyDrop<Rc<RefCell<Poisonable<NonNull<T>>>>>,

    /// Act as exclusive borrower.
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> Anchor<'a, T> {
    /// Creates a new `Anchor` instance, capturing `reference`.
    pub fn new(reference: &'a T) -> Anchor<'a, T> {
        Self {
            reference: ManuallyDrop::new(Rc::new(reference.into())),
            _phantom: PhantomData,
        }
    }

    /// Creates an infallible portal of indefinite lifetime associated with this anchor.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use ref_portals::rc::Anchor;
    /// #
    /// let x = "Scoped".to_owned();
    /// let anchor = Anchor::new(&x);
    /// let self_owned: Box<dyn Fn() + 'static> = Box::new({
    ///     let portal = anchor.portal();
    ///     move || println!("{}", *portal)
    /// });
    ///
    /// self_owned(); // Scoped
    /// ```
    ///
    #[inline]
    pub fn portal(&self) -> Portal<T> {
        self.reference.pipe_deref(Rc::clone).pipe(Portal)
    }

    /// Creates a weak portal of indefinite lifetime associated with this anchor.  
    /// Dropping an anchor doesn't panic if only weak portals exist.
    #[inline]
    pub fn weak_portal(&self) -> WeakPortal<T> {
        Portal::downgrade(&self.portal())
    }
}

impl<'a, T: ?Sized> RwAnchor<'a, T> {
    /// Creates a new `RwAnchor` instance, capturing `reference`.
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Rc::new(RefCell::new(Poisonable {
                pointer: reference.into(),
                poisoned: false,
            }))),
            _phantom: PhantomData,
        }
    }

    /// Creates a fallible portal with unbounded lifetime supporting overlapping reads.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use ref_portals::rc::RwAnchor;
    /// #
    /// let mut x = "Scoped".to_owned();
    /// let anchor = RwAnchor::new(&mut x);
    /// let self_owned: Box<dyn Fn() + 'static> = Box::new({
    ///     let portal = anchor.portal();
    ///     move || {
    ///         println!("{}", *portal.borrow());
    ///         *portal.borrow_mut() = "Replacement".to_owned();
    ///     }
    /// });
    ///
    /// self_owned(); // Scoped
    /// drop(self_owned);
    /// drop(anchor);
    /// println!("{}", x); // Replacement
    /// ```
    ///
    #[inline]
    pub fn portal(&self) -> RwPortal<T> {
        self.reference.pipe_deref(Rc::clone).pipe(RwPortal)
    }

    #[inline]
    pub fn weak_portal(&self) -> WeakRwPortal<T> {
        self.portal().downgrade()
    }
}

impl<'a, T: ?Sized> Drop for Anchor<'a, T> {
    //TODO: Deadlock if active borrows exist.
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Rc::try_unwrap)
        .unwrap_or_else(|_pointer| {
            // Immutable portals are always active borrows, so we need to deadlock immediately here,
            // since a reference could have been sent to another thread.
            error!("!Send `Anchor` dropped while at least one Portal still exists. Deadlocking thread to prevent UB.");
            let deadlock_mutex = Mutex::new(());
            let _deadlock_guard = deadlock_mutex.lock().unwrap();
            let _never = deadlock_mutex.lock();
            // Congratulations.
            unreachable!()
        });
    }
}

//TODO: Test deadlock.
impl<'a, T: ?Sized> Drop for RwAnchor<'a, T> {
    /// Executes the destructor for this type. [Read more](https://doc.rust-lang.org/nightly/core/ops/drop/trait.Drop.html#tymethod.drop)
    ///
    /// # Panics
    ///
    /// If any associated `RwPortal`s exist or, otherwise, iff the anchor has been poisoned:
    ///
    /// ```rust
    /// # use assert_panic::assert_panic;
    /// use ref_portals::rc::RwAnchor;
    ///
    /// let mut x = "Scoped".to_owned();
    /// let anchor = RwAnchor::new(&mut x);
    /// let portal = anchor.portal();
    /// assert_panic!({
    ///     // Poison anchor.
    ///     let _guard = portal.borrow_mut();
    ///     panic!()
    /// });
    ///
    /// assert_panic!(
    ///     drop(anchor),
    ///     &str,
    ///     "Anchor still in use (at least one portal exists)",
    /// );
    /// ```
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Rc::try_unwrap)
        .unwrap_or_else(|reference| {
            reference
                .try_borrow_mut()
                .unwrap_or_else(|_| {
                    // So at this point we know that something else has taken out a borrow of the poisonable value,
                    // and we know that that borrow will never be released because all the types leading there are `!Send`,
                    // and we also don't know whether that's only used on this one thread because a derived reference could have been sent elsewhere.
                    // Meaning this is the only way to prevent UB here:
                    error!("!Send `RwAnchor` dropped while borrowed from. Deadlocking thread to prevent UB.");
                    let deadlock_mutex = Mutex::new(());
                    let _deadlock_guard = deadlock_mutex.lock().unwrap();
                    let _never = deadlock_mutex.lock();
                    // Congratulations.
                    unreachable!()
                })
                .poisoned = true;
            panic!(ANCHOR_STILL_IN_USE)
        })
        .into_inner() // Not fallible.
        .poisoned
        .pipe(|poisoned| {
            if poisoned {
                panic!(ANCHOR_POISONED)
            }
        })
    }
}

/// # Safety:
///
/// ```rust
/// # use {assert_deadlock::assert_deadlock, std::time::Duration};
/// use ref_portals::rc::Anchor;
///
/// let mut x = "Scoped".to_owned();
/// let anchor = Anchor::new(&mut x);
/// let portal = anchor.portal();
///
/// assert_deadlock!(
///     drop(anchor),
///     Duration::from_secs(1),
/// );
/// ```
impl<'a, T: ?Sized> UnwindSafe for Anchor<'a, T> where T: RefUnwindSafe {}

/// # Safety:
///
/// ```rust
/// # use assert_panic::assert_panic;
/// use ref_portals::rc::RwAnchor;
///
/// let mut x = "Scoped".to_owned();
/// let anchor = RwAnchor::new(&mut x);
/// let portal = anchor.portal();
///
/// assert_panic!(
///     drop(anchor),
///     &str,
///     "Anchor still in use (at least one portal exists)",
/// );
/// assert_panic!(
///     { portal.borrow_mut(); },
///     &str,
///     "Anchor poisoned",
/// );
/// ```
impl<'a, T: ?Sized> UnwindSafe for RwAnchor<'a, T> where T: RefUnwindSafe {}

/// An `!Send` immutable portal.  
/// Dereference it directly with `*` or `.deref()`.
#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct Portal<T: ?Sized>(Rc<NonNull<T>>);

/// An `!Send` mutable portal with overlapping immutable borrows.  
/// Acquire a guard by calling `.borrow()` or `.borrow_mut()`.
#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct RwPortal<T: ?Sized>(Rc<RefCell<Poisonable<NonNull<T>>>>);

impl<T: ?Sized> Portal<T> {
    /// Creates a weak portal associated with the same anchor as `portal`.  
    /// Dropping an anchor doesn't panic if only weak portals exist.
    #[inline]
    pub fn downgrade(portal: &Self) -> WeakPortal<T> {
        Rc::downgrade(&portal.0).pipe(WeakPortal)
    }
}

impl<T: ?Sized> Deref for Portal<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        let pointer = self.0.deref();
        unsafe {
            //SAFETY: Valid as long as self.0 is.
            pointer.as_ref()
        }
    }
}

impl<T: ?Sized> Borrow<T> for Portal<T> {
    #[inline]
    fn borrow(&self) -> &T {
        &*self
    }
}

impl<T: ?Sized> RwPortal<T> {
    /// Creates a weak portal associated with the same anchor as this one.  
    /// Dropping an anchor doesn't panic if only weak portals exist.
    #[inline]
    pub fn downgrade(&self) -> WeakRwPortal<T> {
        Rc::downgrade(&self.0).pipe(WeakRwPortal)
    }

    #[inline]
    pub fn borrow<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        let guard = self.0.as_ref().borrow();
        if guard.poisoned {
            panic!(ANCHOR_POISONED)
        }
        PortalRef(guard)
    }

    #[inline]
    pub fn borrow_mut<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        let guard = self.0.as_ref().borrow_mut();
        if guard.poisoned {
            panic!(ANCHOR_POISONED)
        }
        PortalRefMut(guard)
    }
}

impl<T: ?Sized> Clone for Portal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Rc::clone).pipe(Self)
    }
}

impl<T: ?Sized> Clone for RwPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Rc::clone).pipe(Self)
    }
}

//TODO: Docs, test.
impl<T: ?Sized> RefUnwindSafe for RwPortal<T> where T: RefUnwindSafe {}

//TODO: Docs, test.
impl<T: ?Sized> UnwindSafe for RwPortal<T> where T: RefUnwindSafe {}

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct WeakPortal<T: ?Sized>(Weak<NonNull<T>>);

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct WeakRwPortal<T: ?Sized>(Weak<RefCell<Poisonable<NonNull<T>>>>);

impl<T: ?Sized> WeakPortal<T> {
    #[inline]
    pub fn try_upgrade(&self) -> Option<Portal<T>> {
        self.0.upgrade().map(Portal)
    }

    #[inline]
    pub fn upgrade(&self) -> Portal<T> {
        self.try_upgrade().expect(ANCHOR_DROPPED)
    }
}

impl<T: ?Sized> WeakRwPortal<T> {
    #[inline]
    pub fn try_upgrade(&self) -> Option<RwPortal<T>> {
        self.0.upgrade().map(RwPortal)
    }

    #[inline]
    pub fn upgrade(&self) -> RwPortal<T> {
        self.try_upgrade().expect(ANCHOR_DROPPED)
    }
}

impl<T: ?Sized> Clone for WeakPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Weak::clone).pipe(Self)
    }
}

impl<T: ?Sized> Clone for WeakRwPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Weak::clone).pipe(Self)
    }
}

#[repr(transparent)]
struct PortalRef<'a, T: 'a + ?Sized>(Ref<'a, Poisonable<NonNull<T>>>);

#[repr(transparent)]
struct PortalRefMut<'a, T: 'a + ?Sized>(RefMut<'a, Poisonable<NonNull<T>>>);

impl<'a, T: ?Sized> Deref for PortalRef<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        let pointer = &self.0.deref().pointer;
        unsafe {
            //SAFETY: Valid as long as self.0 is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for PortalRefMut<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        let pointer = &self.0.deref().pointer;
        unsafe {
            //SAFETY: Valid as long as self.0 is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalRefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let pointer = &mut self.0.deref_mut().pointer;
        unsafe {
            //SAFETY: Valid as long as self.0 is. Can't be created from a read-only anchor.
            pointer.as_mut()
        }
    }
}

impl<'a, T: ?Sized> Drop for PortalRefMut<'a, T> {
    #[inline]
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.poisoned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _auto_trait_assertions() {
        // Anything that necessitates changes in this method is a breaking change.
        use {assert_impl::assert_impl, core::any::Any};

        assert_impl!(
            !Send: Anchor<'_, ()>,
            RwAnchor<'_, ()>,
            Portal<()>,
            RwPortal<()>,
            PortalRef<'_, ()>,
            PortalRefMut<'_, ()>,
        );

        assert_impl!(
            !Sync: Anchor<'_, ()>,
            RwAnchor<'_, ()>,
            Portal<()>,
            RwPortal<()>,
            PortalRef<'_, ()>,
            PortalRefMut<'_, ()>,
        );

        assert_impl!(
            !UnwindSafe: Anchor<'_, dyn UnwindSafe>,
            RwAnchor<'_, dyn UnwindSafe>,
            Portal<dyn UnwindSafe>,
            RwPortal<dyn UnwindSafe>,
        );
        assert_impl!(
            UnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            RwAnchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
            RwPortal<dyn RefUnwindSafe>,
        );
        assert_impl!(!UnwindSafe: PortalRef<'_, ()>, PortalRefMut<'_, ()>);

        assert_impl!(!RefUnwindSafe: RwPortal<dyn UnwindSafe>);
        assert_impl!(RefUnwindSafe: RwPortal<dyn RefUnwindSafe>);
        assert_impl!(
            //TODO: Should any of these by more RefUnwindSafe?
            !RefUnwindSafe: Anchor<'_, ()>,
            RwAnchor<'_, ()>,
            Portal<()>,
            PortalRef<'_, ()>,
            PortalRefMut<'_, ()>,
        );

        assert_impl!(
            Unpin: Anchor<'_, dyn Any>,
            RwAnchor<'_, dyn Any>,
            Portal<dyn Any>,
            RwPortal<dyn Any>,
            PortalRef<'_, dyn Any>,
            PortalRefMut<'_, dyn Any>,
        )
    }

    fn _impl_trait_assertions() {
        use {assert_impl::assert_impl, core::any::Any};

        assert_impl!(
            Clone: Portal<dyn Any>,
            RwPortal<dyn Any>,
            WeakPortal<dyn Any>,
            WeakRwPortal<dyn Any>,
        );

        assert_impl!(Deref<Target = dyn Any>: Portal<dyn Any>);
        assert_impl!(Borrow<dyn Any>: Portal<dyn Any>);
    }
    //TODO
}
