# Rigor & Intelligence Track (Phase 1.7)

Stabilize the Aion compiler core and introduce initial AI-Native capabilities.

-   **Owner**: Architect
-   **Status**: 🚧 In Progress
-   **Priority**: High

## 📋 Objectives
- [x] Implement arbitrary method chaining (recursive receiver resolution).
- [x] Implement short-circuiting logical operators (`&&`, `||`).
- [x] Introduce `@intrinsic` support for `std.ai.tensor`.
- [x] Research and integrate Boehm GC for initial memory management.

## 🔗 Related Files
- `src/compiler.rs`: LLVM IR generation.
- `src/checker.rs`: Type system.
- `stdlib/std/ai/tensor.ai`: Tensor implementation.
- `src/runtime.c`: C runtime for GC integration.
- [Implementation Plan](plan.md)
