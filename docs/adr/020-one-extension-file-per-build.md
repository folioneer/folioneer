# ADR 020 — One extension file decides a build's external data sources and update channel

**Date**: 2026-09-20
**Status**: Accepted

## Context

The application is free and open (AGPL); what may be sold later is served by a server of the owner's. The public build must therefore stop calling services whose terms forbid commercial use, while the owner keeps them for personal use in a build of his own, updated from a channel of its own. The public repository must name nothing private, and a build must not become a fork to maintain.

Every external data source already sits behind a trait (`PriceProvider`, `RateProvider`, `RateHistoryProvider`, `OpenFigiClient`), but the concrete clients were constructed inline in the two entry points, and the updater read its endpoint from `tauri.conf.json` only. `advice-module-design.md` § Part 4 had left the shape of such a hook open.

## Decision

**`src-tauri/src/extensions.rs` is the only file a build differs by.** It returns:

- the external data sources plugged into the composition root: asset prices, exchange rates, rate history, asset lookup. Both entry points (the application and the headless scheduled fetch) take them from there and construct no client themselves.
- the update channel: the endpoints of the update check and download, and the credentials sent with them. None means the endpoint of `tauri.conf.json`, anonymously — the public build.

Another build replaces that one file before compiling. The file adds no command and no type on the wire. A source that does not exist yet (a bank feed, the advice module) gets its slot in the file when it exists, not before.

A channel that sends credentials can be refused; the refusal is recognised and shown (UPD-028, UPD-029), because such a build would otherwise stop updating without a word. A channel without credentials is never classified as refused.

Alternatives considered:

- **Optional Cargo dependency on a private crate** (the recommendation of `advice-module-design.md` § Part 4) — rejected. A dependency the public cannot fetch can break dependency resolution for every contributor, and the public manifest would name the private repository.
- **A Cargo feature guarding a module that only exists elsewhere** — rejected. The public tree would carry a feature that cannot compile there, and `--all-features` tooling breaks on it.
- **A public port with the private implementation loaded at run time (sidecar or dynamic library)** — rejected. A second build pipeline and a process boundary for what is one constructor call.

## Consequences

- **Pros**: the public repository stays self-contained and always compiles; a private build, and later a beta channel, is a one-file difference; removing a source from the public build is an edit of one function; the advice module's hook question is closed.
- **Cons**: the replacement is a file overlay performed outside this repository, so nothing here can check that a replacement still compiles against a new release — the build that overlays it finds out. What the file returns is therefore a contract: changing its shape is announced in the changelog's technical notes.
- This decision does not affect ADR-017: Yahoo Finance remains the price source the public file plugs in.
