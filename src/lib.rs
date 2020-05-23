use {
    std::{
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        rc::Rc,
        sync::Arc,
    },
    wyz::pipe::*,
};

const ANCHOR_STILL_IN_USE: &str = "Anchor still in use";

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
    reference: ManuallyDrop<Arc<SSNonNull<T>>>,
    _phantom: PhantomData<&'a T>,
}

#[derive(Debug)]
pub struct UnSendAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Rc<NonNull<T>>>,
    _phantom: PhantomData<&'a T>,
}

impl<'a, T: ?Sized> Anchor<'a, T> {
    pub fn new(reference: &'a T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(reference.into())),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> Portal<T> {
        Portal {
            reference: self.reference.deref().clone(),
        }
    }
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

#[derive(Debug)]
pub struct Portal<T: ?Sized> {
    reference: Arc<SSNonNull<T>>,
}

#[derive(Debug)]
pub struct UnSendPortal<T: ?Sized> {
    reference: Rc<NonNull<T>>,
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
        assert_impl!(!Send: Anchor<'_, dyn S>, Portal<dyn S>);
        assert_impl!(Send: Anchor<'_, dyn SS>, Portal<dyn SS>);

        assert_impl!(!Sync: Anchor<'_, dyn S>, Portal<dyn S>);
        assert_impl!(Sync: Anchor<'_, dyn SS>, Portal<dyn SS>);

        assert_impl!(
            !UnwindSafe: Anchor<'_, dyn UnwindSafe>,
            Portal<dyn UnwindSafe>,
        );
        assert_impl!(
            UnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
        );

        assert_impl!(
            !RefUnwindSafe: Anchor<'_, dyn UnwindSafe>,
            Portal<dyn UnwindSafe>,
        );
        assert_impl!(
            RefUnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
        );

        assert_impl!(Unpin: Anchor<'_, dyn Any>, Portal<dyn Any>)
    }
    //TODO
}
