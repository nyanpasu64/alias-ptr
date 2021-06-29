# AliasPtr – (mostly) safe shared pointers in Rust

The `alias-ptr` crate supplies the `AliasPtr` type, which allows safely creating multiple pointers to the same heap-allocated memory, and (unsafely) freeing the memory without reference counting overhead.

The `AliasPtr` type is a pointer whose API is modeled after `Rc` or `Arc` (providing shared `Deref` access to its target). Unlike them, `AliasPtr` expects the user to delete the target manually (an `unsafe` operation). This is designed to work like C++'s raw pointers, which allow aliased access to underlying data, as well as manually freeing/deleting memory.

This is not designed to replace usage of Rust's safe abstractions like Box, but to serve as a fallback where multiple ownership is necessary (and cannot be easily worked around) but the overhead of Rc or Arc is undesired. The intended use is to use `AliasPtr` within your data structures as if it were a `Rc`, carefully audit your unsafe `Drop` logic to ensure you never use-after-free, and expose a safe API to users.

For example, `AliasPtr` can be used to build a sound alternative to `owning_ref::BoxRef`'s usage of a `Box` aliased with a `*const` (which is unsound under Stacked Borrows, fails Miri, and may miscompile once rustc enables noalias for mutable pointers). However, since `BoxRef` is merely a type alias of `owning_ref::OwningRef`, which is generic over the owning type (the reference is always a `*const`), changing `BoxRef<T>` to use an `AliasPtr<T>` rather than `Box<T>` breaks library users which expect `BoxRef<T>` to be the same type as `OwningRef<Box<T>, T>`.

Interestingly, `owning-ref`'s `VecRef` type is not unsound because `Vec` is currently guaranteed to not invalidate references to its elements when moved (`Vec` will *not* use `noalias`). See ["Aliasing rules for `Vec<T>` and other standard containers"](https://github.com/rust-lang/unsafe-code-guidelines/issues/262).

## Install

`alias-ptr` is planned to be published at https://lib.rs/alias-ptr.

## Usage

```rs
use alias_ptr::AliasPtr;
use std::cell::Cell;

struct AliasedPair(AliasPtr<Cell<i32>>, AliasPtr<Cell<i32>>);

impl AliasedPair {
    fn new(x: i32) -> AliasedPair {
        let x = AliasPtr::new(Cell::new(x));
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
fn test_aliased_pair() {
    let pair = AliasedPair::new(1);
    pair.0.set(42);
    assert_eq!(pair.1.get(), 42);
}
```

## Design

AliasPtr holds raw pointers rather than references (since Miri prohibits passing references into `Box::from_raw()`), and lends out shared references whenever dereferenced.

## Testing

AliasPtr is designed to be sound under Stacked Borrows, pass Miri with Stacked Borrows enabled, and not miscompile once rustc enables mutable noalias.

To verify it passes Miri, run:

```
cargo +nightly miri test --target-dir miri
```

Note that this clobbers the "miri" subdirectory.
