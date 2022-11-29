# Scoped Reference

[![Crates.io](https://img.shields.io/crates/v/scoped_reference.svg)](https://crates.io/crates/scoped_reference)
[![Docs.rs](https://docs.rs/scoped_reference/badge.svg)](https://docs.rs/scoped_reference)

This crate provides runtime-checked borrow lifetimes. It allows one to store references of the form `&'a T` as structs with lifetime `'static`.
This is useful in situations where a reference with a shorter lifetime cannot be stored naturally.

The following example demonstrates the use of scoped references. Scoped references come in both mutable and immutable variants.
If the underlying reference is dropped while scoped borrows to it still exist, then the program panics. Note that a panic
will always cause an immediate abort - unwinding is not allowed - because allowing unwinding could lead to
dangling references and undefined behavior.

```rust
struct StaticBorrow(ScopedBorrow<i32>);

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
```