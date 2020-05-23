//! Theadsafe anchors and portals.  
//! These (but not their guards) are various degrees of `Send` and `Sync` depending on their type parameter.

use {
    crate::{ANCHOR_DROPPED, ANCHOR_STILL_IN_USE},
    std::{
        borrow::Borrow,
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
    },
    wyz::pipe::*,
};

/// Panicked when borrowing through a portal or dropping an anchor if the anchor has been poisoned.
/// Only mutable anchors can be poisoned.
const ANCHOR_POISONED: &str = "Anchor poisoned";

/// An externally synchronised `NonNull<T>`.
/// SS stands for Send Sync.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct SSNonNull<T: ?Sized>(NonNull<T>);
unsafe impl<T: ?Sized + Send> Send for SSNonNull<T> {
    //SAFETY: Externally synchronised in this crate.
}
unsafe impl<T: ?Sized + Sync> Sync for SSNonNull<T> {
    //SAFETY: Externally synchronised in this crate.
}
impl<T: ?Sized> From<&T> for SSNonNull<T> {
    #[inline]
    fn from(value: &T) -> Self {
        Self(value.into())
    }
}
impl<T: ?Sized> From<&mut T> for SSNonNull<T> {
    #[inline]
    fn from(value: &mut T) -> Self {
        Self(value.into())
    }
}
impl<T: ?Sized> Deref for SSNonNull<T> {
    type Target = NonNull<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: ?Sized> DerefMut for SSNonNull<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Anchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Arc<SSNonNull<T>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RwAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Arc<RwLock<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct WAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Arc<Mutex<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> Anchor<'a, T> {
    #[inline]
    pub fn new(reference: &'a T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(reference.into())),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn portal(&self) -> Portal<T> {
        self.reference.pipe_deref(Arc::clone).pipe(Portal)
    }

    #[inline]
    pub fn weak_portal(&self) -> WeakPortal<T> {
        Portal::downgrade(&self.portal())
    }
}

impl<'a, T: ?Sized> RwAnchor<'a, T> {
    #[inline]
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(RwLock::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn portal(&self) -> RwPortal<T> {
        self.reference.pipe_deref(Arc::clone).pipe(RwPortal)
    }

    #[inline]
    pub fn weak_portal(&self) -> WeakRwPortal<T> {
        self.portal().downgrade()
    }
}

impl<'a, T: ?Sized> WAnchor<'a, T> {
    #[inline]
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(Mutex::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub fn portal(&self) -> WPortal<T> {
        self.reference.pipe_deref(Arc::clone).pipe(WPortal)
    }

    #[inline]
    pub fn weak_portal(&self) -> WeakWPortal<T> {
        self.portal().downgrade()
    }
}

impl<'a, T: ?Sized> Drop for Anchor<'a, T> {
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Arc::try_unwrap)
        .unwrap_or_else(|_| panic!(ANCHOR_STILL_IN_USE));
    }
}

impl<'a, T: ?Sized> Drop for RwAnchor<'a, T> {
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Arc::try_unwrap)
        .unwrap_or_else(|_| panic!(ANCHOR_STILL_IN_USE))
        .into_inner()
        .unwrap_or_else(|error| Err(error).expect(ANCHOR_POISONED));
    }
}

impl<'a, T: ?Sized> Drop for WAnchor<'a, T> {
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Arc::try_unwrap)
        .unwrap_or_else(|_| panic!(ANCHOR_STILL_IN_USE))
        .into_inner()
        .unwrap_or_else(|error| Err(error).expect(ANCHOR_POISONED));
    }
}

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct Portal<T: ?Sized>(Arc<SSNonNull<T>>);

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct RwPortal<T: ?Sized>(Arc<RwLock<SSNonNull<T>>>);

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct WPortal<T: ?Sized>(Arc<Mutex<SSNonNull<T>>>);

impl<T: ?Sized> Portal<T> {
    #[inline]
    pub fn downgrade(portal: &Self) -> WeakPortal<T> {
        Arc::downgrade(&portal.0).pipe(WeakPortal)
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
    #[inline]
    pub fn downgrade(&self) -> WeakRwPortal<T> {
        Arc::downgrade(&self.0).pipe(WeakRwPortal)
    }

    #[inline]
    pub fn read<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        self.0.read().expect(ANCHOR_POISONED).pipe(PortalReadGuard)
    }

    #[inline]
    pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        self.0
            .write()
            .expect(ANCHOR_POISONED)
            .pipe(PortalWriteGuard)
    }
}

impl<T: ?Sized> WPortal<T> {
    #[inline]
    pub fn downgrade(&self) -> WeakWPortal<T> {
        Arc::downgrade(&self.0).pipe(WeakWPortal)
    }

    #[inline]
    pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        self.0.lock().expect(ANCHOR_POISONED).pipe(PortalMutexGuard)
    }
}

impl<T: ?Sized> Clone for Portal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Arc::clone).pipe(Self)
    }
}

impl<T: ?Sized> Clone for RwPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Arc::clone).pipe(Self)
    }
}

impl<T: ?Sized> Clone for WPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Arc::clone).pipe(Self)
    }
}

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct WeakPortal<T: ?Sized>(Weak<SSNonNull<T>>);

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct WeakRwPortal<T: ?Sized>(Weak<RwLock<SSNonNull<T>>>);

#[derive(Debug)]
#[must_use]
#[repr(transparent)]
pub struct WeakWPortal<T: ?Sized>(Weak<Mutex<SSNonNull<T>>>);

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

impl<T: ?Sized> WeakWPortal<T> {
    #[inline]
    pub fn try_upgrade(&self) -> Option<WPortal<T>> {
        self.0.upgrade().map(WPortal)
    }

    #[inline]
    pub fn upgrade(&self) -> WPortal<T> {
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

impl<T: ?Sized> Clone for WeakWPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.pipe_ref(Weak::clone).pipe(Self)
    }
}

#[repr(transparent)]
struct PortalReadGuard<'a, T: 'a + ?Sized>(RwLockReadGuard<'a, SSNonNull<T>>);

#[repr(transparent)]
struct PortalWriteGuard<'a, T: 'a + ?Sized>(RwLockWriteGuard<'a, SSNonNull<T>>);

#[repr(transparent)]
struct PortalMutexGuard<'a, T: 'a + ?Sized>(MutexGuard<'a, SSNonNull<T>>);

impl<'a, T: ?Sized> Deref for PortalReadGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        let pointer = self.0.deref();
        unsafe {
            //SAFETY: Valid as long as self.0 is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for PortalWriteGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        let pointer = self.0.deref();
        unsafe {
            //SAFETY: Valid as long as self.0 is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for PortalMutexGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        let pointer = self.0.deref();
        unsafe {
            //SAFETY: Valid as long as self.0 is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalWriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        let pointer = self.0.deref_mut();
        unsafe {
            //SAFETY: Valid as long as self.0 is. Can't be created from a read-only anchor.
            pointer.as_mut()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalMutexGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        let pointer = self.0.deref_mut();
        unsafe {
            //SAFETY: Valid as long as self.0 is. Can't be created from a read-only anchor.
            pointer.as_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn _auto_trait_assertions() {
        // Anything that necessitates changes in this method is a breaking change.
        use {
            assert_impl::assert_impl,
            core::any::Any,
            std::panic::{RefUnwindSafe, UnwindSafe},
        };

        trait S: Send {}
        trait SS: Send + Sync {}

        assert_impl!(!Send: WAnchor<'_, dyn Any>, WPortal<dyn Any>);
        assert_impl!(Send: WAnchor<'_, dyn S>, WPortal<dyn S>);
        assert_impl!(
            !Send: Anchor<'_, dyn S>,
            RwAnchor<'_, dyn S>,
            Portal<dyn S>,
            RwPortal<dyn S>,
        );
        assert_impl!(
            Send: Anchor<'_, dyn SS>,
            RwAnchor<'_, dyn SS>,
            Portal<dyn SS>,
            RwPortal<dyn SS>,
        );
        assert_impl!(
            !Send: PortalReadGuard<'_, ()>,
            PortalWriteGuard<'_, ()>,
            PortalMutexGuard<'_, ()>,
        );

        assert_impl!(!Sync: WPortal<dyn Any>);
        assert_impl!(Sync: WPortal<dyn S>);
        assert_impl!(
            !Sync: Anchor<'_, dyn S>,
            RwAnchor<'_, dyn S>,
            WAnchor<'_, dyn S>,
            Portal<dyn S>,
            RwPortal<dyn S>,
            PortalReadGuard<'_, dyn S>,
            PortalWriteGuard<'_, dyn S>,
            PortalMutexGuard<'_, dyn S>,
        );
        assert_impl!(
            Sync: Anchor<'_, dyn SS>,
            RwAnchor<'_, dyn SS>,
            WAnchor<'_, dyn SS>,
            Portal<dyn SS>,
            RwPortal<dyn SS>,
            PortalReadGuard<'_, dyn SS>,
            PortalWriteGuard<'_, dyn SS>,
            PortalMutexGuard<'_, dyn SS>,
        );

        assert_impl!(
            UnwindSafe: PortalReadGuard<'_, dyn Any>,
            PortalWriteGuard<'_, dyn Any>,
            PortalMutexGuard<'_, dyn Any>,
        );
        assert_impl!(
            !UnwindSafe: Anchor<'_, dyn UnwindSafe>,
            Portal<dyn UnwindSafe>,
        );
        assert_impl!(
            UnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
        );
        assert_impl!(!UnwindSafe: RwAnchor<'_, ()>, WAnchor<'_, ()>,);

        assert_impl!(
            RefUnwindSafe: RwPortal<dyn Any>,
            WPortal<dyn Any>,
            PortalReadGuard<'_, dyn Any>,
            PortalWriteGuard<'_, dyn Any>,
            PortalMutexGuard<'_, dyn Any>,
        );
        assert_impl!(
            !RefUnwindSafe: Anchor<'_, dyn UnwindSafe>,
            RwAnchor<'_, dyn UnwindSafe>,
            WAnchor<'_, dyn UnwindSafe>,
            Portal<dyn UnwindSafe>,
        );
        assert_impl!(
            RefUnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            RwAnchor<'_, dyn RefUnwindSafe>,
            WAnchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
        );

        assert_impl!(
            Unpin: Anchor<'_, dyn Any>,
            RwAnchor<'_, dyn Any>,
            WAnchor<'_, dyn Any>,
            Portal<dyn Any>,
            RwPortal<dyn Any>,
            WPortal<dyn Any>,
            PortalReadGuard<'_, dyn Any>,
            PortalWriteGuard<'_, dyn Any>,
            PortalMutexGuard<'_, dyn Any>,
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
