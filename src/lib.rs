use std::{
    borrow::{Borrow, BorrowMut},
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

const ANCHOR_STILL_IN_USE: &str = "Anchor still in use";
const ANCHOR_POISONED: &str = "Anchor poisoned";
const ANCHOR_DROPPED: &str = "Anchor dropped";

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
    fn from(value: &T) -> Self {
        Self(value.into())
    }
}
impl<T: ?Sized> From<&mut T> for SSNonNull<T> {
    fn from(value: &mut T) -> Self {
        Self(value.into())
    }
}
impl<T: ?Sized> Deref for SSNonNull<T> {
    type Target = NonNull<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: ?Sized> DerefMut for SSNonNull<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub struct Anchor<'a, T: ?Sized> {
    reference: Arc<RwLock<Option<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
pub struct RwAnchor<'a, T: ?Sized> {
    reference: Arc<RwLock<Option<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

#[derive(Debug)]
pub struct WAnchor<'a, T: ?Sized> {
    reference: Arc<Mutex<Option<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> Anchor<'a, T> {
    pub fn new(reference: &'a T) -> Self {
        Self {
            reference: Arc::new(RwLock::new(Some(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> Portal<T> {
        Portal {
            reference: self.reference.clone(),
        }
    }
}

impl<'a, T: ?Sized> RwAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: Arc::new(RwLock::new(Some(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> Portal<T> {
        Portal {
            reference: self.reference.clone(),
        }
    }

    pub fn rw_portal(&self) -> RwPortal<T> {
        RwPortal {
            reference: self.reference.clone(),
        }
    }
}

impl<'a, T: ?Sized> WAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: Arc::new(Mutex::new(Some(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn w_portal(&self) -> WPortal<T> {
        WPortal {
            reference: self.reference.clone(),
        }
    }
}

impl<'a, T: ?Sized> Drop for Anchor<'a, T> {
    fn drop(&mut self) {
        self.reference
            .try_write()
            .unwrap_or_else(|error| match error {
                std::sync::TryLockError::Poisoned(_) => unreachable!(),
                std::sync::TryLockError::WouldBlock => panic!(ANCHOR_STILL_IN_USE),
            })
            .take()
            .unwrap();
    }
}

impl<'a, T: ?Sized> Drop for RwAnchor<'a, T> {
    fn drop(&mut self) {
        self.reference
            .try_write()
            .unwrap_or_else(|error| match error {
                std::sync::TryLockError::Poisoned(poison) => Err(poison).expect(ANCHOR_POISONED),
                std::sync::TryLockError::WouldBlock => panic!(ANCHOR_STILL_IN_USE),
            })
            .take()
            .unwrap();
    }
}

impl<'a, T: ?Sized> Drop for WAnchor<'a, T> {
    fn drop(&mut self) {
        self.reference
            .try_lock()
            .unwrap_or_else(|error| match error {
                std::sync::TryLockError::Poisoned(poison) => Err(poison).expect(ANCHOR_POISONED),
                std::sync::TryLockError::WouldBlock => panic!(ANCHOR_STILL_IN_USE),
            })
            .take()
            .unwrap();
    }
}

#[derive(Debug)]
pub struct Portal<T: ?Sized> {
    reference: Arc<RwLock<Option<SSNonNull<T>>>>,
}

#[derive(Debug)]
pub struct RwPortal<T: ?Sized> {
    reference: Arc<RwLock<Option<SSNonNull<T>>>>,
}

#[derive(Debug)]
pub struct WPortal<T: ?Sized> {
    reference: Arc<Mutex<Option<SSNonNull<T>>>>,
}

impl<T: ?Sized> Portal<T> {
    pub fn read<'a>(&'a self) -> impl Borrow<T> + 'a {
        PortalReadGuard {
            guard: self.reference.read().unwrap(),
        }
    }
}

impl<T: ?Sized> RwPortal<T> {
    pub fn read<'a>(&'a self) -> impl Borrow<T> + 'a {
        PortalReadGuard {
            guard: self.reference.read().expect(ANCHOR_POISONED),
        }
    }

    pub fn write<'a>(&'a self) -> impl Borrow<T> + BorrowMut<T> + 'a {
        PortalWriteGuard {
            guard: self.reference.write().expect(ANCHOR_POISONED),
        }
    }
}

impl<T: ?Sized> WPortal<T> {
    pub fn write<'a>(&'a self) -> impl Borrow<T> + BorrowMut<T> + 'a {
        PortalMutexGuard {
            guard: self.reference.lock().expect(ANCHOR_POISONED),
        }
    }
}

struct PortalReadGuard<'a, T: 'a + ?Sized> {
    guard: RwLockReadGuard<'a, Option<SSNonNull<T>>>,
}

struct PortalWriteGuard<'a, T: 'a + ?Sized> {
    guard: RwLockWriteGuard<'a, Option<SSNonNull<T>>>,
}

struct PortalMutexGuard<'a, T: 'a + ?Sized> {
    guard: MutexGuard<'a, Option<SSNonNull<T>>>,
}

impl<'a, T: ?Sized> Borrow<T> for PortalReadGuard<'a, T> {
    fn borrow(&self) -> &T {
        let pointer = self.guard.as_ref().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Borrow<T> for PortalWriteGuard<'a, T> {
    fn borrow(&self) -> &T {
        let pointer = self.guard.as_ref().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Borrow<T> for PortalMutexGuard<'a, T> {
    fn borrow(&self) -> &T {
        let pointer = self.guard.as_ref().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> BorrowMut<T> for PortalWriteGuard<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        let pointer = self.guard.as_mut().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_mut()
        }
    }
}

impl<'a, T: ?Sized> BorrowMut<T> for PortalMutexGuard<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        let pointer = self.guard.as_mut().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
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
            UnwindSafe: Portal<dyn Any>,
            PortalReadGuard<'_, dyn Any>,
            PortalWriteGuard<'_, dyn Any>,
            PortalMutexGuard<'_, dyn SS>,
        );
        assert_impl!(!UnwindSafe: Anchor<'_, dyn UnwindSafe>);
        assert_impl!(UnwindSafe: Anchor<'_, dyn RefUnwindSafe>,);
        assert_impl!(!UnwindSafe: RwAnchor<'_, ()>, WAnchor<'_, ()>);

        assert_impl!(
            RefUnwindSafe: Portal<dyn Any>,
            RwPortal<dyn Any>,
            WPortal<dyn Any>,
            PortalReadGuard<'_, dyn Any>,
            PortalWriteGuard<'_, dyn Any>,
            PortalMutexGuard<'_, dyn Any>,
        );
        assert_impl!(
            !RefUnwindSafe: Anchor<'_, dyn UnwindSafe>,
            RwAnchor<'_, dyn UnwindSafe>,
            WAnchor<'_, dyn UnwindSafe>,
        );
        assert_impl!(
            RefUnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            RwAnchor<'_, dyn RefUnwindSafe>,
            WAnchor<'_, dyn RefUnwindSafe>,
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
    //TODO
}
