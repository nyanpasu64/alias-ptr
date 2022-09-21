use std::ops::Deref;
use std::ptr::NonNull;

use crate::AliasPtr;

/// A unique ownership pointer which automatically frees its target but allows aliased
/// references.
///
/// The type `AliasBox<T>` provides unique ownership and shared access to a value of
/// type `T`, allocated in the heap. Invoking [`alias`][AliasBox::alias] on
/// `AliasBox<T>` produces an [`AliasPtr<T>`] instance, which points to the same
/// allocation on the heap as the source `AliasBox`.
/// It is unsound to call [`delete()`][AliasPtr::delete] on the resulting `AliasPtr`.
/// When you drop the `AliasBox`, the target is dropped and
/// deallocated, and all of the aliases can no longer be safely dereferenced.
///
/// `AliasBox` is primarily intended as an unsafe building block for safe abstractions,
/// in order to avoid the runtime overhead of `Rc` or `Arc`
/// in cases where the lifetimes are known statically.
///
/// Shared references in Rust disallow mutation by default, and `AliasBox`
/// is no exception: you cannot generally obtain a mutable reference to
/// something inside an `AliasBox`. If you need mutability, put a `Cell`/`RefCell`
/// (not thread-safe), `Mutex`/`RwLock`/`Atomic` (thread-safe), or `UnsafeCell`
/// (unsafe API) inside the `AliasBox`.
///
/// ## Usage
///
/// For each `AliasBox<T>`, you are responsible for not deleting its `AliasPtr` aliases,
/// or dereferencing them after destroying the `AliasBox`.
///
/// In Rust terms, `AliasBox<T>` frees the target like a `Box<T>`, but dereferences
/// like a `&T` (is not noalias and does not provide exclusive access).
///
/// In C++ terms, `AliasBox<T>` operates like `unique_ptr<T>`, which can be freely
/// aliased by other pointers.
///
/// ## Thread Safety
///
/// `AliasBox<T>: Sync` requires `T: Sync`, because `AliasBox<T>: Sync` allows you to
/// call [`AliasBox::deref()`] or
/// [`AliasBox::alias()`] on another thread. It doesn't require `T: Send` because,
/// although `AliasBox<T>: Sync` lets you call `AliasBox::alias()` on another thread
/// and get an [`AliasPtr<T>`], [`AliasPtr::delete()`] is marked unsafe,
/// and deleting an `AliasBox`-derived `AliasPtr` is unsound to perform on *any* thread.
///
/// `AliasBox<T>: Send` requires `T: Send`, because `AliasBox<T>: Send` allows you to drop `T` on the
/// other thread, requiring `T: Send`. And it requires `T: Sync`, because
/// `AliasBox<T>: Send` allows you to move the `AliasBox<T>` to another thread
/// while keeping `AliasPtr<T>` on the original thread, allowing you to access `&T` on
/// multiple threads. (While [`AliasBox::alias()`] is an `unsafe fn`,
/// the only reason you'd ever create an `AliasBox` is to call it.)
///
/// If these bounds are inappropriate for your data structure, you can
/// `unsafe impl Send/Sync` for your type containing `AliasBox`.
///
/// ## Implementation
///
/// `AliasBox<T>` has the same size as `&T`, and can be created from a `Box<T>`.
///
/// `AliasBox` wraps a raw pointer rather than a `&T`,
/// because it's not legal to pass a `&` into `Box::from_raw()`,
/// and a dangling `&` may be UB.
/// See ["How to Dismantle an Atomic Bomb"](http://blog.pnkfx.org/blog/2021/03/25/how-to-dismantle-an-atomic-bomb/)
/// for details.
#[repr(transparent)]
pub struct AliasBox<T: ?Sized>(NonNull<T>);

impl<T: ?Sized> From<Box<T>> for AliasBox<T> {
    fn from(item: Box<T>) -> Self {
        // Safety: pointer is obtained from Box::into_raw().
        unsafe { Self::from_raw(Box::into_raw(item)) }
    }
}

impl<T: Sized> AliasBox<T> {
    /// Allocates memory on the heap and then places `x` into it.
    ///
    /// This doesn't actually allocate if `T` is zero-sized.
    pub fn new(x: T) -> AliasBox<T> {
        Self::from(Box::new(x))
    }
}

impl<T: ?Sized> Drop for AliasBox<T> {
    fn drop(&mut self) {
        // Safety: This allows creating dangling `AliasPtr`,
        // but it is unsafe to create an AliasPtr from an AliasBox.
        unsafe {
            drop(Box::from_raw(self.0.as_ptr()));
        }
    }
}

impl<T: ?Sized> AliasBox<T> {
    /// Constructs an `AliasBox` from a raw pointer.
    ///
    /// # Safety
    ///
    /// `p` must be non-null and valid (its target is readable and writable),
    /// and must be obtained from `Box::into_raw()`.
    pub unsafe fn from_raw(p: *mut T) -> Self {
        Self(NonNull::new_unchecked(p))
    }

    // TODO should some of these functions be turned into type-level functions
    // to avoid clashing with Deref?

    /// Construct an [`AliasPtr`] pointing to the same data as `self`, allowing for
    /// shared access to `T`.
    ///
    /// # Safety
    ///
    /// The returned `AliasPtr` and all clones are invalid
    /// (safe but unsound to dereference) once `self: AliasBox` is dropped.
    pub unsafe fn alias(&self) -> AliasPtr<T> {
        AliasPtr::from_raw(self.0.as_ptr())
    }

    /// Provides a raw pointer to the data.
    ///
    /// The pointer is valid until `this` is dropped, deallocating the data.
    ///
    /// If you call `unsafe { &mut *p.as_ptr() }`, you must not dereference any other
    /// aliases of `p` while the exclusive reference is active.
    pub fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T: ?Sized> Deref for AliasBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        // Safety: AliasBox is always constructed from a Box,
        // so can be dereferenced safely.
        // It is the responsibility of the user to never alias an AliasBox
        // and delete the alias.
        unsafe { &*self.0.as_ptr() }
    }
}

unsafe impl<T: ?Sized> Send for AliasBox<T> where T: Send + Sync {}
unsafe impl<T: ?Sized> Sync for AliasBox<T> where T: Sync {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::mem::size_of;

    /// In Rust, the last field of a struct is the last one to be deleted.
    /// I maintain this is a mistake, and it makes more sense to delete from back to
    /// front, making it safe for the *first* struct field to own data referenced by
    /// later fields.
    struct AliasedPair(AliasPtr<Cell<i32>>, AliasBox<Cell<i32>>);

    impl AliasedPair {
        fn new(x: i32) -> AliasedPair {
            let x = AliasBox::new(Cell::new(x));

            // Safety: It's unsafe to copy `AliasedPair::0` or alias `AliasedPair::1`,
            // and dereference the dangling pointer after the `AliasedPair` is dropped
            // (which drops the `AliasBox` and frees the backing memory).
            // For `AliasedPair::new()` to be marked as safe, all `AliasPtr` fields
            // (preferably `AliasBox` too) must not be marked `pub`, and the current
            // module must not create dangling `AliasPtr`.
            AliasedPair(unsafe { x.alias() }, x)
        }
    }

    #[test]
    fn test_option_size_of() {
        assert_eq!(size_of::<usize>(), size_of::<AliasBox<i32>>());
        assert_eq!(size_of::<usize>(), size_of::<Option<AliasBox<i32>>>());
    }

    #[test]
    fn test_aliased_pair() {
        let pair = AliasedPair::new(1);
        pair.0.set(42);
        assert_eq!(pair.1.get(), 42);
    }

    // /// Does not compile, as expected.
    // fn f() -> AliasBox<&'static i32> {
    //     let x = 1;
    //     let out = AliasBox::new(&x) as AliasBox<&'static i32>;
    //     out
    // }
}
