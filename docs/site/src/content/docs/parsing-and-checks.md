---
title: Parsing and checks
description: Extract values from responses and classify them with explicit checks.
---

Use parsing blocks to turn a response into named variables, then use `Key Check` to classify the values.

## Parsing rules

- Left/right parsing rejects empty delimiters.
- Regex parsing honors multiline mode and native capture expansion.
- CSS parsing with index `-1` returns all matched values.

## Check rules

Use non-empty condition lists. For numeric greater-than or less-than checks, both operands must parse as numbers; invalid input is not silently treated as zero.
