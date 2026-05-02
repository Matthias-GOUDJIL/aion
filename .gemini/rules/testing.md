## Testing

- Test runner: `python3 runner.py` at project root
- Tests are `tests/fixtures/*.ai` files; expected outputs in `tests/expected/*.out`
- Output is parsed between `-------------------------------` delimiter lines
- `005_unsafe_check` is an **expected failure** (tests unsafe block enforcement)
- New expected output files are auto-created on first run if missing
- **Always run `python3 runner.py` after any compiler change before committing**
- **Create a test for every new feature or bug fix** — if behavior changes, add or update a fixture