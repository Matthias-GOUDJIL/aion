## Developer Commands

| Action | Command |
|--------|---------|
| Build + run a file | `./aion build <file.ai> && ./output` |
| Compile + run (all-in-one) | `./aion run <file.ai>` |
| Run test suite | `python3 runner.py` |
| Transpile to SQL | `./aion transpile <file.ai>` |

**CRITICAL**: Never run `cargo build`, `cargo run`, or `cargo test` directly on the host. The `./aion` wrapper runs everything inside a Docker container (`aion-compiler` image based on Ubuntu 22.04 with LLVM 15). The wrapper caches `target/` and `cargo/registry` in Docker volumes to avoid full rebuilds.

## OpenCode Workflow

- **`/task`** — Delegate complex multi-step investigations. Use for broad searches or refactors (e.g., "Find all `unwrap()` in `src/` and replace with proper error handling"). Runs a dedicated agent in parallel.
- **`/grep "pattern"`** — Fast content search across the codebase. Prefer over asking the agent to search manually.
- **`/glob "pattern"`** — Find files by name pattern (e.g., `/glob "*.ai" stdlib/`).
- **`/read <file>`** — Inspect files directly without waiting for the agent.
- **`/todo`** — Track multi-step work. Use when changes span multiple files or require sequential steps.
- **`/edit`** — Make targeted edits to files. Prefer over asking the agent to rewrite entire files.
- **`/bash`** — Run shell commands. The agent uses this for builds, tests, and git operations.
- **Docker cache gotcha**: if tests fail with linker errors (`undefined reference`, `file in wrong format`, `cannot find temp_*.o`), the Docker image or volumes are stale. Run: `docker rmi aion-compiler && docker volume rm aion-target-cache aion-cargo-cache && docker build -t aion-compiler .`