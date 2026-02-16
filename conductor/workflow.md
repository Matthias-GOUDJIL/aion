# Aion - Workflows

## 🤖 AI Contribution Protocol

All code changes must adhere to the **Roles & Mandates** defined in `.gemini/GEMINI.md`.

## 📦 Build Pipeline

1.  **Format**: `cargo fmt`
2.  **Lint**: `cargo clippy`
3.  **Test**: `python3 runner.py`
4.  **Docs**: Regenerate `docs/API.md` if needed.

## 🐛 Bug Fix Protocol

1.  Create a regression test in `tests/fixtures/`.
2.  Identify the failure (parser loop, type mismatch...).
3.  Implement fix (e.g. `parse_primary` logic).
4.  Verify all tests pass (`17/17`).
5.  Document the fix in `conductor/tracks.md` or relevant `SPEC.md` section.

## 🚀 Feature Development

1.  Define the feature in `docs/SPEC.md`.
2.  Create a failing test case.
3.  Implement parser support -> Checker support -> Codegen support.
4.  Pass all tests.
