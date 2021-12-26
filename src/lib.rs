//! The `alias-ptr` crate supplies the [`AliasPtr`] type,
//! which allows safely creating multiple pointers to the same heap-allocated memory,
//! and (unsafely) freeing the memory without reference counting overhead.

mod ptr;
mod r#box;

pub use ptr::AliasPtr;
pub use r#box::AliasBox;
