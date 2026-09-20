//! Cross-cutting concerns shared across bounded contexts (gold layout, B37–B43).

/// Shared kernel domain vocabulary — cross-BC value objects (SYN/CFR record-change model).
pub mod domain;
/// Infrastructure shared across bounded contexts (outbound HTTP, etc.).
pub mod infrastructure;
