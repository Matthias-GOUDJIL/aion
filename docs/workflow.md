## Developer Commands

| Action | Command |
|--------|---------|
| Build + run a file | `./aion build <file.ai> && ./output` |
| Compile + run (all-in-one) | `./aion run <file.ai>` |
| Run test suite | `docker run --rm -v "$(pwd)":/workspace -w /workspace aion-compiler cargo test -- --test-threads=1` |
| Generate documentation | `./aion doc <file.ai>` |
| Transpile to SQL | `./aion transpile <file.ai>` |

**CRITICAL**: The `./aion` wrapper and `cargo test` run inside a Docker container (`aion-compiler` image based on Ubuntu 22.04 with LLVM 15). The wrapper caches `target/` and `cargo/registry` in Docker volumes to avoid full rebuilds.

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

- Check [GitHub Issues](https://github.com/Matthias-GOUDJIL/aion/issues) for the full issue list
- Pick an issue, create a branch named after it: `feat/issue-2-spanned-ast`
- Reference the issue in commits: `feat: add Span (#2)`
- PR description should include `Closes #N` to auto-close

### Issue & PR Naming Conventions

**Issue titles**: `[Area] Brief description in imperative mood`
- Area is one of: `Compiler`, `Codegen`, `Parser`, `Checker`, `Stdlib`, `TypeSystem`, `CI`, `Docs`
- Examples: `[Codegen] Fix signed remainder producing negative bucket indices`, `[Stdlib] Add HashMap::contains_key`

**PR titles**: `type: Brief description` (conventional commit format)
- Type: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`
- Examples: `fix: use unsigned remainder for % operator`, `feat: add contains_key to HashMap`

**Branch names**: `{type}/issue-{N}-{short-slug}` or `{type}/{short-slug}`
- Examples: `fix/issue-61-method-resolution`, `feat/hashmap-utils`

**Labels** (apply to all issues):

| Category | Labels |
|----------|--------|
| Priority | `priority-critical`, `priority-high`, `priority-medium`, `priority-low` |
| Type | `type-bug`, `type-feature`, `type-refactor`, `type-docs`, `type-chore`, `type-test` |
| Area | `area-compiler`, `area-lexer`, `area-parser`, `area-checker`, `area-codegen`, `area-type-system`, `area-stdlib`, `area-runtime`, `area-transpiler`, `area-testing`, `area-ci`, `area-docs` |
| Phase | `phase-1`, `phase-2`, `phase-3` |
| Meta | `good first issue`, `help wanted`, `duplicate`, `invalid`, `wontfix` |

**Label rules**:
- Every issue must have exactly one **priority** and exactly one **type** label.
- Every issue must have at least one **area** label.
- **phase** labels are optional — apply only when the issue maps to a ROADMAP phase.
- **Meta** labels are optional overlays (e.g. `good first issue` + `type-feature`).

## Docker Cache Gotcha

If tests fail with linker errors (`undefined reference`, `file in wrong format`, `cannot find temp_*.o`), the Docker image or volumes are stale. Run:

```bash
docker rmi aion-compiler && docker volume rm aion-target-cache aion-cargo-cache && docker build -t aion-compiler .
```

Note: the Dockerfile pre-compiles `src/runtime.c` to `/opt/aion_runtime.bc` (build-time, `clang-15`). If you edit `src/runtime.c`, rebuild the image so the bitcode is refreshed — `docker rmi aion-compiler && docker build -t aion-compiler .` (#73).
