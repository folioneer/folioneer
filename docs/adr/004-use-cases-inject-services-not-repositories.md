# ADR 004 — Use Cases Inject Services, Not Repositories

**Date**: 2026-04-16
**Status**: Accepted

## Context

The `use_cases/` layer orchestrates cross-context application logic. When a use case needs data from a bounded context, it has two options: inject the context's `Service` or inject its `Repository` directly.

Injecting repositories bypasses the service layer, which is where domain validation, event publication, and business invariants live. It also creates a second access path to the same data that can diverge from the service's behaviour over time.

## Decision

Use cases **always inject services**, never repositories.

Any data access or mutation a use case needs from a bounded context must go through that context's service. Repositories remain internal implementation details of their bounded context and are not exposed outside it.

## Consequences

- **Pros**: business invariants and event publication defined in services are never bypassed; single access path per context; consistent with the Component → Hook → Gateway → Command → Service → Repository data-flow rule.
- **Cons**: occasionally requires adding a thin pass-through method to a service for data the repository already exposes; slightly more boilerplate when the use case only needs a simple read.
