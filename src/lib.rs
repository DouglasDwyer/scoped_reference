#![deny(warnings)]
#![cfg_attr(not(feature = "std"), no_std)]

//! This crate provides runtime-checked borrow lifetimes. It allows one to store references of the form `&'a T` as structs with lifetime `'static`.
//! This is useful in situations where a reference with a shorter lifetime cannot be stored naturally.
//!
//! The following example demonstrates the use of scoped references. Scoped references come in both mutable and immutable variants.
//! If the underlying reference is dropped while scoped borrows to it still exist, then the program panics. Note that a panic
//! will always cause an immediate abort - unwinding is not allowed - because allowing unwinding could lead to
//! dangling references and undefined behavior.
//! 
//! ```no_run
//! # use scoped_reference::*;
//! struct StaticBorrow(ScopedBorrow<i32>);
//! 
//! # fn test_borrow_mut() {
//! let mut x = 10;
//! let borrowed_x = &mut x;
//! let mut scoped_ref = ScopedReference::new_mut(borrowed_x);
//! 
//! let mut mut_ref_to_x = scoped_ref.borrow_mut();
//! *mut_ref_to_x = 9;
//! 
//! // Panic: mut_ref_to_x is still out!
//! // drop(scoped_ref);
//! 
//! drop(mut_ref_to_x);
//! 
//! let static_borrow = StaticBorrow(scoped_ref.borrow());
//! assert_eq!(*static_borrow.0, 9);
//! 
//! // Panic: static_borrow is still out!
//! // drop(scoped_ref);
//! 
//! drop(static_borrow);
//! drop(scoped_ref);
//! # }
//! ```

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(feature = "std")]
use std as alloc;

use core::fmt;
use core::ops::{Deref, DerefMut};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Allows for obtaining references with `'static` lifetime via runtime
/// borrow checking.
pub struct ScopedReference<'a, T: ?Sized> {
    reference: Result<&'a T, &'a mut T>,
    alive: Arc<AtomicUsize>
}

impl<'a, T: ?Sized> ScopedReference<'a, T> {
    /// Creates a new scoped reference for the specified borrow.
    pub fn new(reference: &'a T) -> Self {
        let alive = Arc::new(AtomicUsize::new(0));
        let reference = Ok(reference);
        Self { reference, alive }
    }

    /// Creates a new scoped reference for the specifed mutable borrow.
    pub fn new_mut(reference: &'a mut T) -> Self {
        let alive = Arc::new(AtomicUsize::new(0));
        let reference = Err(reference);
        Self { reference, alive }
    }

    /// Obtains a dynamically-checked borrow to the current reference.
    pub fn borrow(&self) -> ScopedBorrow<T> {
        match &self.reference {
            Ok(r) => {
                self.alive.fetch_add(1, Ordering::Release);
                ScopedBorrow { pointer: *r as *const T, alive: self.alive.clone() }
            },
            Err(r) => {
                if self.alive.load(Ordering::Acquire) == usize::MAX {
                    panic_abort("Cannot borrow a lifetime mutably while it is already borrowed immutably.");
                }
                else {
                    self.alive.fetch_add(1, Ordering::Release);
                    ScopedBorrow { pointer: *r as *const T, alive: self.alive.clone() }
                }
            }
        }
    }

    /// Obtains a mutable dynamically-checked borrow to the current reference.
    pub fn borrow_mut(&mut self) -> ScopedBorrowMut<T> {
        if self.alive.load(Ordering::Acquire) != 0 {
            panic_abort("Scoped lifetime is already borrowed.")
        }
        else {
            self.alive.store(usize::MAX, Ordering::Release);
            ScopedBorrowMut { pointer: unsafe { self.reference.as_mut().map_err(|x| *x as *mut T).unwrap_err_unchecked() }, alive: self.alive.clone() }
        }
    }
}

impl<'a, T: ?Sized> fmt::Debug for ScopedReference<'a, T> {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<'a, T: ?Sized> fmt::Display for ScopedReference<'a, T> {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<'a, T: ?Sized> Drop for ScopedReference<'a, T> {
    fn drop(&mut self) {
        if self.alive.load(Ordering::Acquire) != 0 {
            panic_abort("Scoped lifetime was dropped while a borrow was out.")
        }
    }
}

/// Represents a borrow with a runtime-checked lifetime.
pub struct ScopedBorrow<T: ?Sized> {
    pointer: *const T,
    alive: Arc<AtomicUsize>
}

impl<T: ?Sized> Deref for ScopedBorrow<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.pointer }
    }
}

impl<T: ?Sized> Drop for ScopedBorrow<T> {
    fn drop(&mut self) {
        self.alive.fetch_sub(1, Ordering::Release);
    }
}

impl<T: ?Sized> Clone for ScopedBorrow<T> {
    fn clone(&self) -> Self {
        self.alive.fetch_add(1, Ordering::Release);
        Self { pointer: self.pointer, alive: self.alive.clone() }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for ScopedBorrow<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for ScopedBorrow<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

unsafe impl<T: ?Sized + Send> Send for ScopedBorrow<T> {}
unsafe impl<T: ?Sized + Sync> Sync for ScopedBorrow<T> {}

/// Represents a mutable borrow with a runtime-checked lifetime.
pub struct ScopedBorrowMut<T: ?Sized> {
    pointer: *mut T,
    alive: Arc<AtomicUsize>
}

impl<T: ?Sized> Deref for ScopedBorrowMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.pointer }
    }
}

impl<T: ?Sized> DerefMut for ScopedBorrowMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.pointer }
    }
}

impl<T: ?Sized> Drop for ScopedBorrowMut<T> {
    fn drop(&mut self) {
        self.alive.store(0, Ordering::Release);
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for ScopedBorrowMut<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for ScopedBorrowMut<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

unsafe impl<T: ?Sized + Send> Send for ScopedBorrowMut<T> {}
unsafe impl<T: ?Sized + Sync> Sync for ScopedBorrowMut<T> {}

#[allow(unreachable_code)]
fn panic_abort(error: &str) -> ! {
    #[cfg(panic = "abort")]
    {
        panic!("{}", error);
    }
    #[cfg(all(not(panic = "abort"), feature = "std"))]
    {
        println!("{}", error);
        std::process::abort();
    }
    #[cfg(all(not(panic = "abort"), not(feature = "std")))]
    {
        struct Abort;

        // Panic in a drop while panicking aborts the process
        impl Drop for Abort {
            fn drop(&mut self) {
                panic!();
            }
        }

        #[allow(unused_variables)]
        let abort = Abort;

        panic!("{}", error);

        core::mem::forget(abort);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticBorrow(ScopedBorrow<i32>);

    #[test]
    fn test_borrow_mut() {
        let mut x = 10;
        let borrowed_x = &mut x;
        let mut scoped_ref = ScopedReference::new_mut(borrowed_x);
        
        let mut mut_ref_to_x = scoped_ref.borrow_mut();
        *mut_ref_to_x = 9;

        // Panic: mut_ref_to_x is still out!
        // drop(scoped_ref);

        drop(mut_ref_to_x);

        let static_borrow = StaticBorrow(scoped_ref.borrow());
        assert_eq!(*static_borrow.0, 9);

        // Panic: static_borrow is still out!
        // drop(scoped_ref);

        drop(static_borrow);
        drop(scoped_ref);
    }
}