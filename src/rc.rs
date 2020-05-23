use {
    crate::{ANCHOR_DROPPED, ANCHOR_STILL_IN_USE},
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        rc::{Rc, Weak},
    },
    wyz::pipe::*,
};

#[derive(Debug)]
pub struct Anchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Rc<NonNull<T>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
pub struct RwAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Rc<RefCell<NonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> Anchor<'a, T> {
    pub fn new(reference: &'a T) -> Self {
        Self {
            reference: ManuallyDrop::new(Rc::new(reference.into())),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> Portal<T> {
        Portal {
            reference: self.reference.deref().clone(),
        }
    }

    pub fn weak_portal(&self) -> WeakPortal<T> {
        Portal::downgrade(&self.portal())
    }
}

impl<'a, T: ?Sized> RwAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Rc::new(RefCell::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> RwPortal<T> {
        RwPortal {
            reference: self.reference.deref().clone(),
        }
    }

    pub fn weak_portal(&self) -> WeakRwPortal<T> {
        self.portal().downgrade()
    }
}

impl<'a, T: ?Sized> Drop for Anchor<'a, T> {
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Rc::try_unwrap)
        .expect(ANCHOR_STILL_IN_USE);
    }
}

impl<'a, T: ?Sized> Drop for RwAnchor<'a, T> {
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Rc::try_unwrap)
        .expect(ANCHOR_STILL_IN_USE)
        .into_inner(); // Not fallible.
    }
}

#[derive(Debug)]
pub struct Portal<T: ?Sized> {
    reference: Rc<NonNull<T>>,
}

#[derive(Debug)]
pub struct RwPortal<T: ?Sized> {
    reference: Rc<RefCell<NonNull<T>>>,
}

impl<T: ?Sized> Portal<T> {
    pub fn downgrade(portal: &Self) -> WeakPortal<T> {
        WeakPortal {
            reference: Rc::downgrade(&portal.reference),
        }
    }
}

impl<T: ?Sized> Deref for Portal<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let pointer = self.reference.deref();
        unsafe {
            //SAFETY: Valid as long as self.reference is.
            pointer.as_ref()
        }
    }
}

impl<T: ?Sized> RwPortal<T> {
    pub fn downgrade(&self) -> WeakRwPortal<T> {
        WeakRwPortal {
            reference: Rc::downgrade(&self.reference),
        }
    }

    pub fn borrow<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        PortalRef {
            guard: self.reference.as_ref().borrow(),
        }
    }

    pub fn borrow_mut<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        PortalRefMut {
            guard: self.reference.as_ref().borrow_mut(),
        }
    }
}

impl<T: ?Sized> Clone for Portal<T> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
        }
    }
}

impl<T: ?Sized> Clone for RwPortal<T> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
        }
    }
}

#[derive(Debug)]
pub struct WeakPortal<T: ?Sized> {
    reference: Weak<NonNull<T>>,
}

#[derive(Debug)]
pub struct WeakRwPortal<T: ?Sized> {
    reference: Weak<RefCell<NonNull<T>>>,
}

impl<T: ?Sized> WeakPortal<T> {
    pub fn try_upgrade(&self) -> Option<Portal<T>> {
        self.reference
            .upgrade()
            .map(|reference| Portal { reference })
    }

    pub fn upgrade(&self) -> Portal<T> {
        self.try_upgrade().expect(ANCHOR_DROPPED)
    }
}

impl<T: ?Sized> WeakRwPortal<T> {
    pub fn try_upgrade(&self) -> Option<RwPortal<T>> {
        self.reference
            .upgrade()
            .map(|reference| RwPortal { reference })
    }

    pub fn upgrade(&self) -> RwPortal<T> {
        self.try_upgrade().expect(ANCHOR_DROPPED)
    }
}

impl<T: ?Sized> Clone for WeakPortal<T> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
        }
    }
}

impl<T: ?Sized> Clone for WeakRwPortal<T> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
        }
    }
}

struct PortalRef<'a, T: 'a + ?Sized> {
    guard: Ref<'a, NonNull<T>>,
}

struct PortalRefMut<'a, T: 'a + ?Sized> {
    guard: RefMut<'a, NonNull<T>>,
}

impl<'a, T: ?Sized> Deref for PortalRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for PortalRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let pointer = self.guard.deref_mut();
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
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
            Portal<dyn UnwindSafe>,
        );
        assert_impl!(
            UnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
        );
        assert_impl!(
            !UnwindSafe: RwAnchor<'_, ()>,
            RwPortal<()>,
            PortalRef<'_, ()>,
            PortalRefMut<'_, ()>
        );

        assert_impl!(
            //TODO: Should any of these by more RefUnwindSafe?
            !RefUnwindSafe: Anchor<'_, ()>,
            RwAnchor<'_, ()>,
            Portal<()>,
            RwPortal<()>,
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

        assert_impl!(Clone: Portal<dyn Any>,
            RwPortal<dyn Any>,
            WeakPortal<dyn Any>,
            WeakRwPortal<dyn Any>,
        );
    }
    //TODO
}
