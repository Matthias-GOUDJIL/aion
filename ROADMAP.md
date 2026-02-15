# Aion Language Roadmap: Towards Self-Hosting (v1.0)

This document outlines the technical path to transform Aion from its current Rust-based prototype into a universal system language capable of compiling itself.

---

## 🧱 Phase 1: Power Up (v0.4 - v0.6)
*Goal: Make Aion capable of writing complex programs and data manipulators.*

### **v0.4: Advanced Type System**
- [ ] **Enums & Pattern Matching**: Essential for compiler logic (AST ready, Codegen partial).
- [ ] **Generics**: Introduction of `Type<T>` for reusable data structures.
- [x] **First-Class Strings**: Initial string support with correct LLVM typing.

### **v0.5: Extended Standard Library**
- [ ] **`std.fs`**: File read/write capabilities.
- [ ] **`std.collections`**: Direct Aion implementation of `Vec<T>` and `HashMap<K,V>`.
- [ ] **`std.env`**: Command-line argument management.

### **v0.6: Native Runtime (End of C Runtime)**
- [ ] **Self-Runtime**: Rewrite `aion_spawn` and the thread scheduler in Aion via `syscalls`.
- [ ] **GCC/Clang Removal**: Aion will only depend on LLVM for the final binary.

---

## 💎 Phase 1.5: Elo Legacy (v0.5 - v0.6)
*Goal: Integrate data primitives and security proven by Elo.*

1.  **Native Temporal Management (Time & Duration)** [COMPLETED]
    *   First-class primitives: `Duration` (e.g., `5s`, `10ms`), `Date` (e.g., `D2024-01-01`).
    *   Essential for scheduling and logging in Shyrka OS.
2.  **Pipeline Operator (`|>`)** [COMPLETED]
    *   Fluid syntax for data processing (e.g., `data |> filter |> map`).
    *   Improves readability of AI and transformation algorithms.
3.  **SQL Backend & Interoperability** [IN PROGRESS]
    *   Ability to compile Aion subsets to pure SQL.
    *   Support for IF blocks and complex expressions (Phase 1.5).
4.  **Data Sandboxing (Safe-Mode)** [COMPLETED]
    *   `unsafe` keyword recognized in Lexer/Parser.
    *   Strict security verification in TypeChecker: `unsafe` function calls are blocked outside of `unsafe { ... }` blocks.

---

## 🔄 Phase 2: The Great Port (v0.7 - v0.9)
*Goal: Rewrite the compiler core (currently in Rust) in the Aion language.*

### **v0.7: Aion Lexer**
- [ ] `lexer.ai` implementation. Validation by the Rust (Bootstrap) compiler.

### **v0.8: Aion Parser**
- [ ] `parser.ai` implementation. AST manipulation via Aion `struct` and `match`.

### **v0.9: Aion LLVM Backend**
- [ ] Direct generation of `.ll` (LLVM IR) text files from the AST.

---

## ♾️ Phase 3: The Ouroboros (v1.0)
*Goal: Full Self-Hosting.*

1. **Final Compilation**: Use the Rust compiler to compile the Aion source code of `aionc.ai`.
2. **The Loop**: Use the resulting binary to re-compile its own source code.
3. **Consecration**: Definitive removal of Rust code. Aion is 100% autonomous.

---

## 📊 Current State: v0.6 (Phase 1.6 - Rigor & Completeness)
- [x] Lexer / Parser EBNF v0.5.
- [x] Type Checker (Safety & Temporal & Security Enforcement).
- [x] CodeGen LLVM (Native & Dynamic Strings & Function Returns).
- [x] Modular Compiler Architecture (`src/compiler.rs`).
- [ ] **EXHAUSTIVITY GOAL**: Real Enum Tagged Unions and deep Pattern Matching.
