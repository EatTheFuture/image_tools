//! A simplified wrapper for Arc<RwLock<T>>.

use std::sync::{Arc, RwLock};

pub struct Shared<T>(Arc<RwLock<T>>);

impl<T> Shared<T> {
    #[inline(always)]
    pub fn new(inner: T) -> Shared<T> {
        Shared(Arc::new(RwLock::new(inner)))
    }

    /// Get a read-only lock on the data.
    ///
    /// Will block until the lock is acquired.  Will panic on errors.
    #[inline(always)]
    pub fn lock(&self) -> std::sync::RwLockReadGuard<T> {
        self.0.read().unwrap()
    }

    /// Attempt to get a read-only lock on the data.
    ///
    /// Will return `None` if the lock is not immediately available or
    /// if there is an error.  Does not block.
    #[inline(always)]
    pub fn try_lock(&self) -> Option<std::sync::RwLockReadGuard<T>> {
        self.0.try_read().ok()
    }

    /// Get a mutable lock on the data.
    ///
    /// Will block until the lock is acquired.  Will panic on errors.
    #[inline(always)]
    pub fn lock_mut(&self) -> std::sync::RwLockWriteGuard<T> {
        self.0.write().unwrap()
    }

    /// Attempt to get a mutable lock on the data.
    ///
    /// Will return `None` if the lock is not immediately available or
    /// if there is an error.  Does not block.
    #[inline(always)]
    pub fn try_lock_mut(&self) -> Option<std::sync::RwLockWriteGuard<T>> {
        self.0.try_write().ok()
    }

    /// Creates a clone of the shared data reference.
    ///
    /// This is actually identical to calling `.clone()`, but this name
    /// makes it clearer at the call site that the underlying data isn't
    /// being cloned, only the reference.
    #[inline(always)]
    pub fn clone_ref(&self) -> Shared<T> {
        self.clone()
    }
}

impl<T> Clone for Shared<T> {
    #[inline(always)]
    fn clone(&self) -> Shared<T> {
        Shared(self.0.clone())
    }
}

unsafe impl<T> Send for Shared<T> where T: Send {}

unsafe impl<T> Sync for Shared<T> where T: Sync + Send {}

impl<T> std::panic::UnwindSafe for Shared<T> {}
impl<T> std::panic::RefUnwindSafe for Shared<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}
