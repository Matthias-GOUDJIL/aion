# Aion Developer Skills

## Porting Elo Feature to Aion
1.  **Source Analysis**: Locate the feature in `elo/src` (e.g., `src/stdlib.ts` for functions, `src/types.ts` for data structures). Note the input/output types and edge cases.
2.  **Type Mapping**: Convert TypeScript types to Rust/Aion equivalents:
    -   `number` -> `f64` (default) or `i64`.
    -   `string` -> `String` (heap allocated) or `&str`.
    -   `Date` -> `u64` (timestamp) or struct `DateTime`.
3.  **AST Design**:
    -   Add new `Token` variant in `src/token.rs` if syntax is involved (e.g., `|>`).
    -   Update `src/parser.rs` to handle the new grammar.
4.  **Codegen (LLVM)**:
    -   Implement the IR generation in `src/compiler.rs`.
    -   Ensure memory safety (check for null pointers, bounds).
5.  **StdLib Wrapper**:
    -   Expose the low-level intrinsic in `stdlib/core/intrinsics.ai`.
    -   Wrap it in a user-friendly function in `stdlib/std/`.

## Debugging Compiler Segfaults
1.  **LLVM IR Inspection**: Run `./aion build file.ai` and check `output.ll`. Look for `undef`, `null`, or misaligned pointers.
2.  **LLVM-Auditor Protocol**:
    -   Check `printf` calls: ensure arguments are `ptr` for strings and match the format string.
    -   Verify `alloca` types match `store` types.
    -   Check for missing terminators (`ret`, `br`) in basic blocks.
    -   Validate using `opt-15 -verify output.ll`.
3.  **GDB/LLDB**: Run `gdb --args ./aion build file.ai`.
3.  **AddressSanitizer**: Recompile Aion with `RUSTFLAGS="-Z sanitizer=address" cargo build` (requires nightly) to find use-after-free.

## Writing AI-Native Standard Library
1.  **Intent-First**: Start with `::intent "Description"`.
2.  **Zero-Allocation**: Prefer passing slices `&[]` over `Vec<>` where possible.
3.  **Error Handling**: Return `Result<T, E>` (or equivalent struct) rather than crashing.
