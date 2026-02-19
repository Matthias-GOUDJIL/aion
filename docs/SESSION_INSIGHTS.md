# Aion Development Insights - Session Retrospective

## 🧠 Architectural Lessons

### 1. The "Mut Self" Trap (Pass-by-Value vs. Pass-by-Pointer)
- **Problem**: Initially, the compiler passed all structs by value to functions. Methods like `push` that modified `self` only updated a local copy, leaving the original struct in the caller's scope unchanged.
- **Solution**: Implemented implicit "pass-by-pointer" for any function parameter named `self` that corresponds to a struct. 
- **Insight**: In system languages, `self` must almost always be a pointer if it is mutable. Future monomorphization of structs should keep this in mind.

### 2. Two-Pass Compilation Requirement
- **Problem**: Function calls failed if the target function was declared further down in the file or in an imported file.
- **Solution**: Split the compilation into two passes:
    1.  **Pass 1**: Register all function prototypes in the LLVM module.
    2.  **Pass 2**: Compile the bodies of all registered functions.
- **Insight**: This is standard for languages with forward-referencing or recursive modules.

### 3. Fuzzy Name Resolution for Modules
- **Problem**: Renaming all imported declarations to fully qualified names (e.g., `std.env.args`) broke all internal references in the imported files (which still looked for `args`).
- **Solution**: Implemented a "Fuzzy Lookup" in the Environment and Compiler. If a simple name isn't found, the compiler looks for a qualified name ending with that suffix (e.g., `args` -> `std.env.args`).
- **Insight**: This provides a lightweight namespace system without requiring a complex AST rewriter or symbol aliasing pass.

## 🛠️ Parser Fragility & Fixes

- **`::intent`**: Metadata like `::intent` must be handled as a `NoOp` statement rather than an expression to avoid "undefined variable" errors in the type checker.
- **`if` as Expression**: Added support for `if` expressions in `parse_primary`. This requires a corresponding variant in the `Expression` AST and handling in the compiler (Phi nodes).
- **`*` and `as`**: Pointers and casting are essential for standard library implementation. Added `Deref` and `Cast` variants to the AST.
- **`while` Loops**: Missing from the initial grammar, but used extensively in `stdlib`. Added `While` statement and LLVM IR generation.

## 🚀 Pro-Tips for Future Sessions

1.  **Check `argc`/`argv`**: Always ensure `main` correctly stores OS arguments into the globals `aion_argc` and `aion_argv`.
2.  **Opaque Pointers**: LLVM 15+ uses opaque pointers. When loading from a pointer, you **must** provide the element type explicitly to `build_load`.
3.  **Recursive Imports**: The `process_imports` logic must rename local declarations **before** recursing to avoid exponential prefix concatenation.
4.  **Assignment as Statement**: Assignment `a = b` is currently a statement in Aion, not an expression.
