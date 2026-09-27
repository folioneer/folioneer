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

2. **Set up and start developing**:

   ```bash
   git config core.hooksPath .githooks   # the commit and push checks
   just install                          # dependencies
   just dev                              # start the app with hot reload
   just --list                           # every recipe
   ```

3. **Read the policies:**
   - [How work moves](./docs/workflow.md) — branches, pull requests, the checks a change passes
   - [Commit Policy](./COMMIT_POLICY.md) — commit message format

Always go through a `just` recipe when one exists; do not run `npm`, `cargo` or `sqlx` directly for something the `justfile` covers.

## Common Commands

```bash
just check            # lint and format check, fast
just format           # auto-fix formatting on both layers
just test             # frontend tests
just test-rust        # backend tests
just harness          # the merge gate, locally: architecture rules, lint, build, tests with coverage for what changed
just generate-types   # regenerate the TypeScript bindings after changing a command
```

## What a pull request passes

The git hooks run the fast checks for what a commit or push touches. On the pull request, CI runs the full gate: lint and types, both test suites with coverage floors, the architecture rules, the licence check, the E2E suite on the real application (skipped when only records change), and the reviewer agents matched to the diff. A pull request merges only when every check is green. Details: [docs/workflow.md](./docs/workflow.md).

## Getting Help

- [Architecture Guide](./ARCHITECTURE.md) for system design
- [Test conventions](./docs/test_convention.md) for testing practices
- Recent merged pull requests for examples
