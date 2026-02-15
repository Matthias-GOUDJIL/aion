# Aion Language v0.5 - The Universal Nexus (Phase 1.5)

Aion is a universal programming language designed for the AI era. It combines the raw performance of C++/Inkwell with the fluidity of Python, while integrating AI at the core of its syntax.

## 🚀 Current Capabilities (v0.5)

- **Native Compilation**: LLVM IR code generation via Inkwell (Rust).
- **First-Class Temporal Types**: Native support for `Duration` (e.g., `5s`) and `Date` (e.g., `D2024-01-01`) types.
- **Date Arithmetic**: `Date + Duration` handled natively by the Type Checker.
- **AI-Native Syntax**: `::intent` blocks are first-class AST nodes.
- **Docker Isolation**: Isolated compiler with intelligent signal management and unique temporary files.
- **Modular Architecture**: Code generation logic isolated in `src/compiler.rs` to facilitate adding new backends.
- **SQL Transpilation**: Initial support for transforming Aion logic into SQL (Phase 1.5).

## 🛠 Installation

The compiler uses Docker to ensure a stable LLVM 15 environment.

```bash
# To compile a file
./aion build hello.ai

# To run a file directly
./aion run hello.ai

# To generate AI documentation
./aion doc hello.ai
```

## 📝 Code Example

```aion
::intent "Calculate server power"
struct Server {
    cpu_cores: i64,
    ram_gb: i64
}

fn main() {
    let s = Server { cpu_cores: 64, ram_gb: 128 }
    return s.cpu_cores * s.ram_gb
}
```

## 🏗 Roadmap
- [ ] Direct WebAssembly (WASM) backend.
- [ ] Advanced bidirectional type inference.
- [ ] Actor System for massive concurrency.
