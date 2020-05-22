use std::{
    borrow::{Borrow, BorrowMut},
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
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

#[derive(Debug)]
pub struct Portal<T: ?Sized> {
    reference: Arc<RwLock<Option<SSNonNull<T>>>>,
}

#[derive(Debug)]
pub struct RwPortal<T: ?Sized> {
    reference: Arc<RwLock<Option<SSNonNull<T>>>>,
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

struct PortalReadGuard<'a, T: 'a + ?Sized> {
    guard: RwLockReadGuard<'a, Option<SSNonNull<T>>>,
}

struct PortalWriteGuard<'a, T: 'a + ?Sized> {
    guard: RwLockWriteGuard<'a, Option<SSNonNull<T>>>,
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

impl<'a, T: ?Sized> BorrowMut<T> for PortalWriteGuard<'a, T> {
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
    fn _compile_time_assertions() {
        use assert_impl::assert_impl;
        trait SS: Send + Sync {}
        assert_impl!(
            Send: Anchor<'_, dyn SS>,
            RwAnchor<'_, dyn SS>,
            Portal<dyn SS>,
            RwPortal<dyn SS>
        );
        assert_impl!(!Send: PortalReadGuard<'_, ()>, PortalWriteGuard<'_, ()>);
        assert_impl!(
            Sync: Anchor<'_, dyn SS>,
            RwAnchor<'_, dyn SS>,
            Portal<dyn SS>,
            RwPortal<dyn SS>,
            PortalReadGuard<'_, dyn SS>,
            PortalWriteGuard<'_, dyn SS>
        );
    }
    //TODO
}
