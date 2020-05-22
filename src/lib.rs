use std::{
    borrow::{Borrow, BorrowMut},
    fmt::Debug,
    marker::PhantomData,
    ptr::NonNull,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

const ANCHOR_POISONED: &str = "Anchor poisoned";
const ANCHOR_DROPPED: &str = "Anchor dropped";

#[derive(Debug)]
pub struct Anchor<'a, T> {
    reference: Arc<RwLock<Option<NonNull<T>>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
pub struct RwAnchor<'a, T> {
    reference: Arc<RwLock<Option<NonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T> Anchor<'a, T> {
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

impl<'a, T> RwAnchor<'a, T> {
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

impl<'a, T> Drop for Anchor<'a, T> {
    fn drop(&mut self) {
        self.reference.write().unwrap().take().unwrap();
    }
}

impl<'a, T> Drop for RwAnchor<'a, T> {
    fn drop(&mut self) {
        self.reference.write().expect(ANCHOR_POISONED).take();
    }
}

#[derive(Debug)]
pub struct Portal<T> {
    reference: Arc<RwLock<Option<NonNull<T>>>>,
}

#[derive(Debug)]
pub struct RwPortal<T> {
    reference: Arc<RwLock<Option<NonNull<T>>>>,
}

impl<T> Portal<T> {
    pub fn read<'a>(&'a self) -> impl Borrow<T> + 'a {
        PortalReadGuard {
            guard: self.reference.read().unwrap(),
        }
    }
}

impl<T> RwPortal<T> {
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

struct PortalReadGuard<'a, T: 'a> {
    guard: RwLockReadGuard<'a, Option<NonNull<T>>>,
}

struct PortalWriteGuard<'a, T: 'a> {
    guard: RwLockWriteGuard<'a, Option<NonNull<T>>>,
}

impl<'a, T> Borrow<T> for PortalReadGuard<'a, T> {
    fn borrow(&self) -> &T {
        let pointer = self.guard.as_ref().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T> Borrow<T> for PortalWriteGuard<'a, T> {
    fn borrow(&self) -> &T {
        let pointer = self.guard.as_ref().expect(ANCHOR_DROPPED);
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T> BorrowMut<T> for PortalWriteGuard<'a, T> {
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
    //TODO
}
