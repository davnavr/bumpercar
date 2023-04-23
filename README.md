# `bumpercar`

A bump allocation arena for fast allocation and deallocation of groups of objects.

Inspired by [`bumpalo`](https://crates.io/crates/bumpalo), but provides additional functionality
for deallocation and usage in multiple threads.

Compatible with `#![no_std]`, depending only on [`alloc`](https://doc.rust-lang.org/alloc/) and
[`core`](https://doc.rust-lang.org/core/index.html).

## Features

- `sync`: Provides the `sync` module, allowing for arena allocation between threads.
- `vec`: Provides the `vec` module, which is a port of [`Vec<T>`] that backed by arena allocation.
