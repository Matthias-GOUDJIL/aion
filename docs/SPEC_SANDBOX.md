# Aion Specification: Sandboxing & Safety

## 1. The Safety Model
Aion adopts an explicit safety boundary inspired by Rust.
By default, all code is checked in **Safe** context.
Operations that can violate memory safety or interact with raw pointers/FFI must be wrapped in an `unsafe` block or declared inside an `unsafe fn`.

## 2. Unsafe Operations (Enforced Today)
The TypeChecker (`src/analysis/checker.rs`) tracks `in_unsafe_context` and rejects the following operations outside of `unsafe { ... }` blocks or `unsafe fn` bodies:
1. **Calling an `unsafe fn`**: including low-level runtime builtins (`aion_malloc`, `aion_realloc`, `aion_free`, `aion_getenv`, `aion_get_argc`, `aion_get_argv_index`, `exit`) and `core.heap.*` functions.
2. **Raw Pointer Dereference**: reading or writing memory via `*ptr`.
3. **Unsafe Intrinsics**: low-level memory intrinsics (`@intrinsic("malloc", ...)`, `@intrinsic("mem_zero_ptr", ...)`).

## 3. The `unsafe` Keyword
```aion
unsafe fn dangerous_op(p: *i64) -> i64 {
    return *p
}

fn main() {
    unsafe {
        let raw = core.heap.alloc(8) as *i64
        *raw = 42
        let v = dangerous_op(raw)
    }
}
```

## 4. Planned Extensions (Not Yet Implemented)
The following profile-based isolation rules are planned for future sandboxing passes:
* **Kernel Mode vs User Mode build profiles**: restricting `unsafe` block definitions or `@system` module imports via CLI flags.

## 5. Implementation Status
- [x] **Lexer/Parser**: Support for `unsafe` keyword on functions and `unsafe { ... }` blocks (statement and expression position).
- [x] **AST**: `Statement::UnsafeBlock` and `Expression::UnsafeBlock`.
- [x] **Type Checker**: Tracks `in_unsafe_context`; rejects `unsafe fn` calls, pointer dereferences (`*ptr`), and unsafe intrinsics outside `unsafe` scopes (tested by `tests/fixtures/language/unsafe_check_fail.ai` and `unsafe_block.ai`).
- [x] **Codegen**: Transparent execution of `unsafe` blocks in LLVM IR.
