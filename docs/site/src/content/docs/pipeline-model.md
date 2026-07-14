---
title: Pipeline model
description: How blocks, variables, and execution status fit together.
---

A pipeline is an ordered list of blocks. During execution, blocks read from and write to an isolated variable store.

## Variable scopes

| Scope | Purpose |
| --- | --- |
| `input.*` | Data supplied to a pipeline run |
| `data.*` | Block output and response-scoped values |
| `globals.*` | Shared configuration values |
| `@name` | User variables and captures |

Use explicit variable names at integration boundaries. Avoid relying on legacy response aliases when a request block has a named response variable.

## Status classification

`Key Check` evaluates explicit conditions and classifies a run as `Success`, `Failure`, `Ban`, `Retry`, or leaves it unclassified. Empty condition groups do not match, and numeric comparisons require valid numeric operands.
