use {
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt::Debug,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        rc::Rc,
        sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
    },
    wyz::pipe::*,
};

const ANCHOR_STILL_IN_USE: &str = "Anchor still in use";
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

impl<'a, T: ?Sized> RwAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(RwLock::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn rw_portal(&self) -> RwPortal<T> {
        RwPortal {
            reference: self.reference.deref().clone(),
        }
    }
}

impl<'a, T: ?Sized> WAnchor<'a, T> {
    pub fn new(reference: &'a mut T) -> Self {
        Self {
            reference: ManuallyDrop::new(Arc::new(Mutex::new(reference.into()))),
            _phantom: PhantomData,
        }
    }

    pub fn w_portal(&self) -> WPortal<T> {
        WPortal {
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

#[derive(Debug)]
pub struct UnSendPortal<T: ?Sized> {
    reference: Rc<NonNull<T>>,
}

#[derive(Debug)]
pub struct UnSendRwPortal<T: ?Sized> {
    reference: Rc<RefCell<NonNull<T>>>,
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
    pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
        PortalMutexGuard {
            guard: self.reference.lock().expect(ANCHOR_POISONED),
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

struct PortalReadGuard<'a, T: 'a + ?Sized> {
    guard: RwLockReadGuard<'a, SSNonNull<T>>,
}

struct PortalWriteGuard<'a, T: 'a + ?Sized> {
    guard: RwLockWriteGuard<'a, SSNonNull<T>>,
}

struct PortalMutexGuard<'a, T: 'a + ?Sized> {
    guard: MutexGuard<'a, SSNonNull<T>>,
}

struct UnSendPortalRef<'a, T: 'a + ?Sized> {
    guard: Ref<'a, NonNull<T>>,
}

struct UnSendPortalRefMut<'a, T: 'a + ?Sized> {
    guard: RefMut<'a, NonNull<T>>,
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
            UnwindSafe: PortalReadGuard<'_, dyn Any>,
            PortalWriteGuard<'_, dyn Any>,
            PortalMutexGuard<'_, dyn SS>,
        );
        assert_impl!(
            !UnwindSafe: Anchor<'_, dyn UnwindSafe>,
            Portal<dyn UnwindSafe>,
        );
        assert_impl!(
            UnwindSafe: Anchor<'_, dyn RefUnwindSafe>,
            Portal<dyn RefUnwindSafe>,
        );
        assert_impl!(!UnwindSafe: RwAnchor<'_, ()>, WAnchor<'_, ()>);

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
