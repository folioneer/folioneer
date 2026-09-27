# ADR 022 — Withdraw the code-organisation decisions into the rules

**Date**: 2026-09-27
**Status**: Accepted — supersedes ADR-003, ADR-004, ADR-005, ADR-007

## Context

An ADR records a technical choice that is costly to reverse, with the guard that stops an agent undoing it (#048). Four ADRs record something else: how code is organised or tested. ADR-003 and ADR-005 say how a use case composes bounded contexts, ADR-004 that a use case injects services, never repositories, and ADR-007 where E2E tests stop. The rules docs already carry the first three — and ADR-004 contradicted rule B24, which lets a use case take a repository trait — so an agent following one broke the other.

## Decision

ADR-003, ADR-004, ADR-005 and ADR-007 are withdrawn; the rules docs are their one home: B18 and B22 (a use case composes contexts and has an orchestrator), B24 and B25 (a use case reaches a context through its service or repository traits, never infrastructure; a write that emits an event goes through the service) in `docs/backend-rules.md`, and E11 (E2E tests stop at the `ComboboxField` boundary) in `docs/e2e-rules.md`. From here on, a code-organisation or pattern choice is a rule, not an ADR.

## Consequences

- **Pros**: one statement per rule; the contradiction between ADR-004 and B24 is gone; the ADR index holds only technical choices.
- **Cons**: the withdrawn ADRs stay readable as history only; their guards now live with the rules (architecture rule A9 for B24, `reviewer-e2e` for E11).

## Guard

- **Reversal looks like**: a new ADR that records a code-organisation or testing pattern, or a rule restated in an ADR.
- **Guard**: `adr-reviewer` (the gate in `adr-writer`: technical choices only).
