---
name: rust-unsafe-checker
description: >
  Reviews and writes unsafe Rust and FFI for soundness. Use when code contains
  unsafe blocks or functions, raw pointers (*const, *mut, NonNull), transmute,
  unions, MaybeUninit, #[repr(C)] layouts, extern "C" or FFI bindings (bindgen,
  cbindgen, libc, CString, CStr), manual Send/Sync impls, or when checking
  SAFETY comments, undefined behavior, or safe wrappers around unsafe code.
allowed-tools: ["Read", "Grep", "Glob"]
---

# Unsafe Rust Checker

## When Unsafe is Valid

| Use Case | Example |
|----------|---------|
| FFI | Calling C functions |
| Low-level abstractions | Implementing `Vec`, `Arc` |
| Performance | Measured bottleneck with safe alternative too slow |

**NOT valid:** Escaping borrow checker without understanding why.

## Required Documentation

```rust
// SAFETY: <why this is safe>
unsafe { ... }

/// # Safety
/// <caller requirements>
pub unsafe fn dangerous() { ... }
```

## Quick Reference

| Operation | Safety Requirements |
|-----------|---------------------|
| `*ptr` deref | Valid, aligned, initialized |
| `&*ptr` | + No aliasing violations |
| `transmute` | Same size, valid bit pattern |
| `extern "C"` | Correct signature, ABI |
| `static mut` | Synchronization guaranteed |
| `impl Send/Sync` | Actually thread-safe |

## Common Errors

| Error | Fix |
|-------|-----|
| Null pointer deref | Check for null before deref |
| Use after free | Ensure lifetime validity |
| Data race | Add proper synchronization |
| Alignment violation | Use `#[repr(C)]`, check alignment |
| Invalid bit pattern | Use `MaybeUninit` |
| Missing SAFETY comment | Add `// SAFETY:` |

## Deprecated → Better

| Deprecated | Use Instead |
|------------|-------------|
| `mem::uninitialized()` | `MaybeUninit<T>` |
| `mem::zeroed()` for refs | `MaybeUninit<T>` |
| Raw pointer arithmetic | `NonNull<T>`, `ptr::add` |
| `CString::new().unwrap().as_ptr()` | Store `CString` first |
| `static mut` | `AtomicT` or `Mutex` |
| Manual extern | `bindgen` |

## FFI Crates

| Direction | Crate |
|-----------|-------|
| C → Rust | bindgen |
| Rust → C | cbindgen |
| Python | PyO3 |
| Node.js | napi-rs |

Claude knows unsafe Rust. Focus on SAFETY comments and soundness.

## Rules and Checklists

Load only the files that match the code under review:

- `checklists/before-unsafe.md`: decide whether `unsafe` is justified before writing it
- `checklists/review-unsafe.md`: step-by-step review of existing unsafe code
- `checklists/common-pitfalls.md`: frequent soundness bugs
- `examples/safe-abstraction.md`: wrapping unsafe code in a sound safe API
- `examples/ffi-patterns.md`: idiomatic Rust/C interop
- `AGENTS.md`: index of all 47 rules by section and level
- `rules/<section>-NN-*.md`: individual rules, grouped by prefix:
  `general-`, `safety-`, `ptr-`, `union-`, `mem-`, `ffi-`, `io-`

These rules predate the Rust 2024 edition. For `unsafe extern` blocks,
`#[unsafe(no_mangle)]`, `unsafe_op_in_unsafe_fn`, and `static_mut_refs`, follow
the `unsafe-` rules in `rust-best-practices`.
