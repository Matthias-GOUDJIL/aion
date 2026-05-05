## Testing

- Test framework: **insta** (snapshot testing) + **assert_cmd** (CLI testing)
- Run tests: `cargo test` (requires Docker and the `aion-compiler` image)
- Test fixtures: `tests/fixtures/*.ai`
- Snapshots: `tests/snapshots/*.snap` (auto-generated, committed to git)
- **Execution**: The AI must NEVER run `cargo test` on its own.
- **Workflow**: After a modification, the AI stops and waits for the user to confirm success or provide the error.
- **Creating new tests**: Add fixture in `tests/fixtures/`, run `cargo test` to generate snapshot, commit the `.snap` file.
- **Updating snapshots**: Run `cargo insta review` to accept/reject changes interactively, or `INSTA_UPDATE=always cargo test` to auto-accept.
- **Expected failures**: Tests like `005_unsafe_check` that test compiler errors should snapshot the stderr output.