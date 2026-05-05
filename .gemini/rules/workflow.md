## Developer Commands

| Action | Command |
|--------|---------|
| Build + run a file | `./aion build <file.ai> && ./output` |
| Compile + run (all-in-one) | `./aion run <file.ai>` |
| Run test suite | `python3 runner.py` |
| Generate documentation | `./aion doc <file.ai>` |
| Transpile to SQL | `./aion transpile <file.ai>` |

**CRITICAL**: Never run `cargo build`, `cargo run`, or `cargo test` directly on the host. The `./aion` wrapper runs everything inside a Docker container (`aion-compiler` image based on Ubuntu 22.04 with LLVM 15). The wrapper caches `target/` and `cargo/registry` in Docker volumes to avoid full rebuilds.

## Git Workflow

### Branching Strategy

1. **Never work directly on `main`**. Always create a feature branch:
   ```bash
   git checkout -b feat/my-feature
   # or: fix/my-fix, refactor/my-refactor, docs/my-docs
   ```

2. **Commit with conventional commits**:
   - `feat:` — new feature
   - `fix:` — bug fix
   - `refactor:` — code change that neither fixes a bug nor adds a feature
   - `docs:` — documentation only
   - `test:` — adding or fixing tests
   - `chore:` — maintenance tasks

3. **Push and create PR**:
   ```bash
   git push origin feat/my-feature
   gh pr create --title "feat: my feature" --body "Description"
   ```

4. **Reference issues in commits/PRs**:
   ```
   feat: add Span to AST nodes
   Closes #2
   ```

### Issue Workflow

- Check [.github/ISSUES.md](.github/ISSUES.md) for the full issue list
- Pick an issue, create a branch named after it: `feat/issue-2-spanned-ast`
- Reference the issue in commits: `feat: add Span (#2)`
- PR description should include `Closes #N` to auto-close

## Docker Cache Gotcha

If tests fail with linker errors (`undefined reference`, `file in wrong format`, `cannot find temp_*.o`), the Docker image or volumes are stale. Run:

```bash
docker rmi aion-compiler && docker volume rm aion-target-cache aion-cargo-cache && docker build -t aion-compiler .
```
