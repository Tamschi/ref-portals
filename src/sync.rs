use {
    crate::{ANCHOR_DROPPED, ANCHOR_STILL_IN_USE},
    std::{
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
    },
    wyz::pipe::*,
};

const ANCHOR_POISONED: &str = "Anchor poisoned";

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
pub struct RwAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Arc<RwLock<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
}

#[derive(Debug)]
pub struct WAnchor<'a, T: ?Sized> {
    reference: ManuallyDrop<Arc<Mutex<SSNonNull<T>>>>,
    _phantom: PhantomData<&'a mut T>,
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

    pub fn weak_portal(&self) -> WeakPortal<T> {
        Portal::downgrade(&self.portal())
    }
}

impl<'a, T: ?Sized> RwAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(RwLock::new(reference.into()))),
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

impl<'a, T: ?Sized> WAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(Mutex::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn portal(&self) -> WPortal<T> {
        WPortal {
            reference: self.reference.deref().clone(),
        }
    }

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
pub struct Portal<T: ?Sized> {
    reference: Arc<SSNonNull<T>>,
}

#[derive(Debug)]
pub struct RwPortal<T: ?Sized> {
    reference: Arc<RwLock<SSNonNull<T>>>,
}

#[derive(Debug)]
pub struct WPortal<T: ?Sized> {
    reference: Arc<Mutex<SSNonNull<T>>>,
}

impl<T: ?Sized> Portal<T> {
    pub fn downgrade(portal: &Self) -> WeakPortal<T> {
        WeakPortal {
            reference: Arc::downgrade(&portal.reference),
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
            reference: Arc::downgrade(&self.reference),
        }
    }

    pub fn read<'a>(&'a self) -> impl Deref<Target = T> + 'a {
        PortalReadGuard {
            guard: self.reference.read().expect(ANCHOR_POISONED),
        }
    }

    pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        PortalWriteGuard {
            guard: self.reference.write().expect(ANCHOR_POISONED),
        }
    }
}

impl<T: ?Sized> WPortal<T> {
    pub fn downgrade(&self) -> WeakWPortal<T> {
        WeakWPortal {
            reference: Arc::downgrade(&self.reference),
        }
    }

    pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        PortalMutexGuard {
            guard: self.reference.lock().expect(ANCHOR_POISONED),
        }
    }
}

#[derive(Debug)]
pub struct WeakPortal<T: ?Sized> {
    reference: Weak<SSNonNull<T>>,
}

#[derive(Debug)]
pub struct WeakRwPortal<T: ?Sized> {
    reference: Weak<RwLock<SSNonNull<T>>>,
}

#[derive(Debug)]
pub struct WeakWPortal<T: ?Sized> {
    reference: Weak<Mutex<SSNonNull<T>>>,
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

impl<T: ?Sized> WeakWPortal<T> {
    pub fn try_upgrade(&self) -> Option<WPortal<T>> {
        self.reference
            .upgrade()
            .map(|reference| WPortal { reference })
    }

    pub fn upgrade(&self) -> WPortal<T> {
        self.try_upgrade().expect(ANCHOR_DROPPED)
    }
}

struct PortalReadGuard<'a, T: 'a + ?Sized> {
    guard: RwLockReadGuard<'a, SSNonNull<T>>,
}

struct PortalWriteGuard<'a, T: 'a + ?Sized> {
    guard: RwLockWriteGuard<'a, SSNonNull<T>>,
}

struct PortalMutexGuard<'a, T: 'a + ?Sized> {
    guard: MutexGuard<'a, SSNonNull<T>>,
}

impl<'a, T: ?Sized> Deref for PortalReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for PortalWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> Deref for PortalMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        let pointer = self.guard.deref();
        unsafe {
            //SAFETY: Valid as long as self.guard is.
            pointer.as_ref()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        let pointer = self.guard.deref_mut();
        unsafe {
            //SAFETY: Valid as long as self.guard is. Can't be created from a read-only anchor.
            pointer.as_mut()
        }
    }
}

impl<'a, T: ?Sized> DerefMut for PortalMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
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
    //TODO
}
