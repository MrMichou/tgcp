# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for tgcp.

## What are ADRs?

ADRs document the key architectural decisions made during the development of tgcp, including the context, decision, and consequences of each choice.

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-data-driven-resource-definitions.md) | Data-Driven Resource Definitions with JSON | Accepted |
| [0002](0002-ratatui-tui-framework.md) | Ratatui as TUI Framework | Accepted |
| [0003](0003-async-tokio-runtime.md) | Async Strategy with Tokio Runtime | Accepted |

## Template

When adding new ADRs, use this template:

```markdown
# ADR NNNN: Title

## Status
[Proposed | Accepted | Deprecated | Superseded]

## Context
What is the issue we're addressing?

## Decision
What did we decide and why?

## Consequences
What are the positive and negative results?

## Alternatives Considered
What other options were evaluated?

## References
Links to relevant code, docs, or discussions.
```

## Contributing

1. Copy the template above
2. Number sequentially (0004, 0005, etc.)
3. Submit PR with the new ADR
4. Update this README index
