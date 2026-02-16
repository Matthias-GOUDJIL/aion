# Aion Programming Language

Aion is a system programming language designed for AI-native applications, combining the performance of Rust/LLVM with the expressiveness of Python.

## 🚀 Quick Start

### Build the Compiler
```bash
./aion build examples/hello.ai && ./output
```

### Run Tests
```bash
python3 runner.py
```

## 📂 Project Structure

-   **`src/`**: The compiler source code (Rust).
-   **`stdlib/`**: The Aion Standard Library (written in Aion).
-   **`docs/`**: Official specifications and API documentation.
    -   [SPEC.md](docs/SPEC.md): Language Specification.
    -   [API.md](docs/API.md): Standard Library API.
-   **`examples/`**: Sample programs showcasing Aion features.
-   **`tests/`**: Integration test suite and fixtures.

## 🤝 Contributing

Please read [docs/SPEC.md](docs/SPEC.md) before contributing to ensure alignment with the language design.
All code changes must pass the test suite (`python3 runner.py`).

## 📜 License

MIT
