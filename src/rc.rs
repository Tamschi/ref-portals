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
#[repr(transparent)]
pub struct Anchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Rc<NonNull<T>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
#[repr(transparent)]
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

    #[inline]
    pub fn portal(&self) -> Portal<T> {
        self.reference.deref().clone().pipe(Portal)
    }

    #[inline]
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

    #[inline]
    pub fn portal(&self) -> RwPortal<T> {
        self.reference.deref().clone().pipe(RwPortal)
    }

    #[inline]
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
#[repr(transparent)]
pub struct Portal<T: ?Sized>(Rc<NonNull<T>>);

#[derive(Debug)]
#[repr(transparent)]
pub struct RwPortal<T: ?Sized>(Rc<RefCell<NonNull<T>>>);

impl<T: ?Sized> Portal<T> {
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

impl<T: ?Sized> RwPortal<T> {
    #[inline]
    pub fn downgrade(&self) -> WeakRwPortal<T> {
        Rc::downgrade(&self.0).pipe(WeakRwPortal)
    }

    #[inline]
    pub fn borrow<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        self.0.as_ref().borrow().pipe(PortalRef)
    }

    #[inline]
    pub fn borrow_mut<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        self.0.as_ref().borrow_mut().pipe(PortalRefMut)
    }
}

impl<T: ?Sized> Clone for Portal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.clone().pipe(Self)
    }
}

impl<T: ?Sized> Clone for RwPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.clone().pipe(Self)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct WeakPortal<T: ?Sized>(Weak<NonNull<T>>);

#[derive(Debug)]
#[repr(transparent)]
pub struct WeakRwPortal<T: ?Sized>(Weak<RefCell<NonNull<T>>>);

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
        self.0.clone().pipe(Self)
    }
}

impl<T: ?Sized> Clone for WeakRwPortal<T> {
    #[inline]
    fn clone(&self) -> Self {
        self.0.clone().pipe(Self)
    }
}

#[repr(transparent)]
struct PortalRef<'a, T: 'a + ?Sized>(Ref<'a, NonNull<T>>);

#[repr(transparent)]
struct PortalRefMut<'a, T: 'a + ?Sized>(RefMut<'a, NonNull<T>>);

impl<'a, T: ?Sized> Deref for PortalRef<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        let pointer = self.0.deref();
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
        let pointer = self.0.deref();
        unsafe {
            //SAFETY: Valid as long as self.0 is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalRefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
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
