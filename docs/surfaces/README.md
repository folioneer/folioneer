# Surfaces — how each one shows the core

Each surface that calls the core has its own description, which says only what that
surface shows, asks and answers; the core's behaviour is the specs' (`docs/spec/README.md`
says what a spec holds and how surface rules leave it). A rule of the core is never
restated here: a description names the spec rule it shows.

| Surface           | Its description                                                                                     |
| ----------------- | --------------------------------------------------------------------------------------------------- |
| The window        | Its validated mocks, one folder per issue in `folioneer/folioneer-design` (`docs/workflow.md` § 4). |
| The command line  | `command-line.md`: every command, its arguments, its output and its exit codes.                     |
| An AI agent (MCP) | `mcp.md`: the design — the bridge, the owner's consent, each tool's input and answer.               |

A description changes in the task's pull request, as a spec does. `command-line.md` and
`mcp.md` start with the first task that moves a command-line or agent rule out of its spec
(`docs/spec/README.md`).
