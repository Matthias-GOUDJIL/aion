## Testing

- Test runner: `python3 runner.py` at project root
- Tests are `tests/fixtures/*.ai` files; expected outputs in `tests/expected/*.out`
- Output is parsed between `-------------------------------` delimiter lines
- `005_unsafe_check` is an **expected failure** (tests unsafe block enforcement)
- New expected output files are auto-created on first run if missing
- **Execution**: The AI must NEVER run `runner.py` on its own.
- **Workflow**: After a modification, the AI stops and waits for the user to confirm success or provide the error.
- **Targeted Debugging**: In case of failure, the user will only provide the text block between the `-------------------------------` delimiters of the failing test.
- **Creation**: The AI must continue to suggest/create new tests (`.ai` + `.out` fixtures) for every new feature or bug fix, but without executing them.
- **Count**: Always check the actual number of test files with `ls tests/fixtures/*.ai | wc -l` — do not hardcode test counts in docs.