# Contributing

## Contribution terms

The project is licensed under the [AGPL-3.0-or-later](LICENSE). Contributions are accepted under the [Contributor License Agreement](CLA.md): you keep the copyright of what you contribute, and you give the maintainer the right to distribute it under the project's licence and under other terms. To agree, add this line to the description of your first pull request:

> I have read the Contributor License Agreement (CLA.md) and I agree to it for this and my future contributions to this project.

A pull request from someone who has not agreed cannot be merged.

## Quick Start

1. **Install just** (command runner):

   ```bash
   # macOS
   brew install just

   # Linux
   curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to ~/bin

   # Or see: https://github.com/casey/just#installation
   ```

2. **Setup and start developing**:

   ```bash
   just dev              # Setup git hooks + start app
   just start            # Start app with hot reload
   just --list           # See all available commands
   ```

3. **Read the policies:**
   - [How work moves](./docs/workflow-c.md) - Branches, pull requests, the checks a change passes
   - [Commit Policy](./COMMIT_POLICY.md) - Commit message format

## Common Commands (using just)

```bash
just start            # Start application
just check            # Run all tests and linters
just test             # Run frontend tests
just test-rust        # Run backend tests
just lint             # Run linter
just format-fix       # Auto-fix formatting
just generate-types   # Generate TS bindings
```

See `justfile` or run `just --list` for all available commands.

## Quality Check

Run all tests and linters before pushing:

```bash
just check              # Full quality check
just check-verbose      # Detailed output

# Or run the script directly:
./scripts/check.sh
```

Output example:

```
| Check              | Status              |
|:-------------------|:--------------------|
| React Tests        | ✅ 110 passing      |
| Rust Lib Tests     | ✅ 50 passing       |
| Build Application  | ✅ Pass             |
| Oxlint (main)      | ✅ 0 warnings       |
| Biome              | ✅ Pass             |
| Clippy (lib)       | ✅ Pass             |
```

## Alternative: Direct Commands

If you prefer not to use `just`, you can run commands directly:

### Testing

```bash
npm test                    # Frontend tests
cd src-tauri && cargo test  # Backend tests
```

### Linting

```bash
npm run lint                              # Frontend
cd src-tauri && cargo clippy -- -D warnings  # Backend
```

### GitHub CLI

```bash
gh pr create                    # Create PR
gh pr view <NUMBER> --comments  # View PR with comments
gh pr list                      # List open PRs
```

## Getting Help

- Check [Architecture Guide](./ARCHITECTURE.md) for system design
- Check [Testing Guide](./TESTING.md) for testing practices
- See recent merged PRs for examples
