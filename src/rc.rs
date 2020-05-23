use {
    crate::ANCHOR_STILL_IN_USE,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        rc::Rc,
    },
    wyz::pipe::*,
};

#[derive(Debug)]
pub struct UnSendAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Rc<NonNull<T>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
pub struct UnSendRwAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Rc<RefCell<NonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> UnSendAnchor<'a, T> {
    pub fn new(reference: &'a T) -> Self {
        Self {
            reference: ManuallyDrop::new(Rc::new(reference.into())),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> UnSendPortal<T> {
        UnSendPortal {
            reference: self.reference.deref().clone(),
        }
    }
}

impl<'a, T: ?Sized> UnSendRwAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Rc::new(RefCell::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn rw_portal(&self) -> UnSendRwPortal<T> {
        UnSendRwPortal {
            reference: self.reference.deref().clone(),
        }
    }
}

impl<'a, T: ?Sized> Drop for UnSendAnchor<'a, T> {
    fn drop(&mut self) {
        unsafe {
            //SAFETY: Dropping.
            ManuallyDrop::take(&mut self.reference)
        }
        .pipe(Rc::try_unwrap)
        .expect(ANCHOR_STILL_IN_USE);
    }
}

impl<'a, T: ?Sized> Drop for UnSendRwAnchor<'a, T> {
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
pub struct UnSendPortal<T: ?Sized> {
    reference: Rc<NonNull<T>>,
}

#[derive(Debug)]
pub struct UnSendRwPortal<T: ?Sized> {
    reference: Rc<RefCell<NonNull<T>>>,
}

impl<T: ?Sized> Deref for UnSendPortal<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let pointer = self.reference.deref();
        unsafe {
            //SAFETY: Valid as long as self.reference is.
            pointer.as_ref()
        }
    }
}

impl<T: ?Sized> UnSendRwPortal<T> {
    pub fn borrow<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        UnSendPortalRef {
            guard: self.reference.as_ref().borrow(),
        }
    }

    pub fn borrow_mut<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        UnSendPortalRefMut {
            guard: self.reference.as_ref().borrow_mut(),
        }
    }
}

struct UnSendPortalRef<'a, T: 'a + ?Sized> {
    guard: Ref<'a, NonNull<T>>,
}

struct UnSendPortalRefMut<'a, T: 'a + ?Sized> {
    guard: RefMut<'a, NonNull<T>>,
}

impl<'a, T: ?Sized> Deref for UnSendPortalRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for UnSendPortalRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for UnSendPortalRefMut<'a, T> {
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
            !Send: UnSendAnchor<'_, ()>,
            UnSendRwAnchor<'_, ()>,
            UnSendPortal<()>,
            UnSendRwPortal<()>,
            UnSendPortalRef<'_, ()>,
            UnSendPortalRefMut<'_, ()>,
        );

        assert_impl!(
            !Sync: UnSendAnchor<'_, ()>,
            UnSendRwAnchor<'_, ()>,
            UnSendPortal<()>,
            UnSendRwPortal<()>,
            UnSendPortalRef<'_, ()>,
            UnSendPortalRefMut<'_, ()>,
        );

        assert_impl!(
            !UnwindSafe: UnSendAnchor<'_, dyn UnwindSafe>,
            UnSendPortal<dyn UnwindSafe>,
        );
        assert_impl!(
            UnwindSafe: UnSendAnchor<'_, dyn RefUnwindSafe>,
            UnSendPortal<dyn RefUnwindSafe>,
        );
        assert_impl!(
            !UnwindSafe: UnSendRwAnchor<'_, ()>,
            UnSendRwPortal<()>,
            UnSendPortalRef<'_, ()>,
            UnSendPortalRefMut<'_, ()>
        );

        assert_impl!(
            //TODO: Should any of these by more RefUnwindSafe?
            !RefUnwindSafe: UnSendAnchor<'_, ()>,
            UnSendRwAnchor<'_, ()>,
            UnSendPortal<()>,
            UnSendRwPortal<()>,
            UnSendPortalRef<'_, ()>,
            UnSendPortalRefMut<'_, ()>,
        );

        assert_impl!(
            Unpin: UnSendAnchor<'_, dyn Any>,
            UnSendRwAnchor<'_, dyn Any>,
            UnSendPortal<dyn Any>,
            UnSendRwPortal<dyn Any>,
            UnSendPortalRef<'_, dyn Any>,
            UnSendPortalRefMut<'_, dyn Any>,
        )
    }
    //TODO
}
