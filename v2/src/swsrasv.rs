//! Single Writer Single Reader Atomically Shared Value

use std::{sync::{atomic::{AtomicPtr, Ordering}, Arc}, ptr, fmt::Debug};

pub struct Reader<T> {
    ptr: Arc<AtomicPtr<T>>,
}

impl<T> Reader<T> {
    /// read the latest value, if there is one, returning it.
    /// returns `None` if `write` has not been called since the last `read`
    /// (including if the associated Writer has been dropped)
    #[must_use]
    pub fn read(&mut self) -> Option<Box<T>> {
        // get the stored ptr, replaceing it with `null` to indicate no stored value
        let ptr = self.ptr.swap(ptr::null_mut(), Ordering::Relaxed);
        // check if there is a new value
        if !ptr.is_null() {
            // saftey: the ptr came from Box::into_raw, and has not been used before with Box::from_raw
            Some(unsafe { Box::from_raw(ptr) })
        } else {
            None
        }
    }
}

impl<T> Debug for Reader<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader").finish()
    }
}

// `Reader` handles dropping the cached value, if there is one
impl<T> Drop for Reader<T> {
    fn drop(&mut self) {
        // set ptr to a null
        let ptr = self.ptr.swap(ptr::null_mut(), Ordering::Relaxed);
        // and if there was something, drop the old value (avoids leaks)
        if !ptr.is_null() {
            unsafe { Box::from_raw(ptr) };
        }
    }
}

pub struct Writer<T> {
    // needs no drop handleing, Reader does this for
    ptr: Arc<AtomicPtr<T>>,
}

impl<T> Writer<T> {
    pub fn write_boxed(&mut self, val: Box<T>) {
        let ptr = Box::into_raw(val);
        // can be relaxed, as no other acesses need to be atomic
        let old = self.ptr.swap(ptr, Ordering::Relaxed);
        // check if there was a value that was never read
        if !old.is_null() {
            // if there was, safely get rid of it
            unsafe { drop(Box::from_raw(old)) };
        }
    }

    pub fn write(&mut self, val: T) {
        self.write_boxed(Box::new(val))
    }
}

impl<T> Debug for Writer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer").finish()
    }
}

/// Creates a new reader-writer pair
pub fn new_pair<T>() -> (Writer<T>, Reader<T>) {
    let ptr = Arc::new(AtomicPtr::<T>::new(ptr::null_mut()));
    (Writer { ptr: ptr.clone() }, Reader { ptr })
}
