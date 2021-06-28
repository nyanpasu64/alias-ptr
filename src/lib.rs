use std::ops::Deref;
use std::ptr::NonNull;

/// The equivalent of C++'s `T*` or `T const*`, with shared ownership over T.
/// You are responsible for deleting exactly one, and not using it or its aliases after.
/// Only necessary until http://blog.pnkfx.org/blog/2021/03/25/how-to-dismantle-an-atomic-bomb/ is fixed,
/// at which point we can use &UnsafeCell<T> instead.
#[repr(transparent)]
pub struct Ptr<T: ?Sized>(NonNull<T>);
// PhantomData is not necessary to prevent leaking Box<&'stack U> variables.
// Also read https://docs.rs/crate/ptr/0.2.2/source/src/lib.rs for reference.

impl<T: ?Sized> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T: ?Sized> From<Box<T>> for Ptr<T> {
    fn from(item: Box<T>) -> Self {
        // Safety: pointer is obtained from Box::into_raw().
        unsafe { Self::from_raw(Box::into_raw(item)) }
    }
}

impl<T: Sized> Ptr<T> {
    pub fn new(x: T) -> Ptr<T> {
        Ptr::from(Box::new(x))
    }
}

impl<T: ?Sized> Ptr<T> {
    /// Requirements: p must be valid (its target is readable and writable).
    /// In order for calling delete() to be sound,
    /// p must be obtained from Box::into_raw().
    ///
    /// Panics: If `p` is null.
    pub unsafe fn from_raw(p: *mut T) -> Self {
        Self(NonNull::new(p).unwrap())
    }

    // TODO should some of these functions be turned into type-level functions
    // to avoid clashing with Deref?

    pub fn copy(&self) -> Self {
        self.clone()
    }

    /// Requirements: The Ptr must be derived from Box::into_raw(),
    /// and neither self nor aliasing pointers can be safely dereferenced
    /// (a safe but unsound operation) after calling delete().
    ///
    /// This method *really* should take `self` by move,
    /// but unfortunately doing so would prevent it from being called in Drop.
    /// See https://internals.rust-lang.org/t/re-use-struct-fields-on-drop-was-drop-mut-self-vs-drop-self/8594
    /// and https://github.com/rust-lang/rust/issues/4330#issuecomment-26852226.
    pub unsafe fn delete(&mut self) {
        Box::from_raw(self.0.as_ptr());
    }

    /// Provides a raw pointer to the data.
    ///
    /// The pointer is valid until delete() is called on the `this` or any of its aliases.
    pub fn as_ptr(this: &Self) -> *const T {
        this.0.as_ptr()
    }
}

impl<T: ?Sized> Deref for Ptr<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // Safety: Ptr is always constructed from a Box,
        // so can be dereferenced safely.
        // It is the responsibility of the user to never delete() a Ptr
        // then dereference it or its aliases afterwards.
        unsafe { &*self.0.as_ptr() }
    }
}

// TODO Send/Sync

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::mem::size_of;

    struct AliasedPair(Ptr<Cell<i32>>, Ptr<Cell<i32>>);

    impl AliasedPair {
        fn new(x: i32) -> AliasedPair {
            let x = Ptr::new(Cell::new(x));
            AliasedPair(x.copy(), x)
        }
    }

    impl Drop for AliasedPair {
        fn drop(&mut self) {
            unsafe {
                self.0.delete();
            }
        }
    }

    #[test]
    fn test_option_size_of() {
        assert_eq!(size_of::<usize>(), size_of::<Ptr<i32>>());
        assert_eq!(size_of::<usize>(), size_of::<Option<Ptr<i32>>>());
    }

    #[test]
    fn test_aliased_pair() {
        let pair = AliasedPair::new(1);
        pair.0.set(42);
        assert_eq!(pair.1.get(), 42);
    }

    // /// Does not compile, as expected.
    // fn f() -> Ptr<&'static i32> {
    //     let x = 1;
    //     let out = Ptr::new(&x) as Ptr<&'static i32>;
    //     out
    // }
}
