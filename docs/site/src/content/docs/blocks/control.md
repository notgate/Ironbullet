---
title: Control flow and organization
description: Reference entries for control flow and organization.
---

# Control flow and organization

Control execution order, branches, repetition, and visual grouping.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## If / Else

- **Rust type:** `IfElse`
- **Availability:** Palette
- **Purpose:** Evaluates its conditions and runs the configured true or false nested block list.
- **Configure:** Configure the condition, iteration, duration, branch, or nested block list. Keep nested paths small enough to debug independently.

## Loop

- **Rust type:** `Loop`
- **Availability:** Palette
- **Purpose:** Repeats its configured nested block list according to the selected loop settings.
- **Configure:** Configure the condition, iteration, duration, branch, or nested block list. Keep nested paths small enough to debug independently.

## Delay

- **Rust type:** `Delay`
- **Availability:** Palette
- **Purpose:** Waits for the configured duration before continuing execution.
- **Configure:** Configure the condition, iteration, duration, branch, or nested block list. Keep nested paths small enough to debug independently.

## Case / Switch

- **Rust type:** `CaseSwitch`
- **Availability:** Palette
- **Purpose:** Selects a nested branch using configured case matching.
- **Configure:** Configure the condition, iteration, duration, branch, or nested block list. Keep nested paths small enough to debug independently.

## Group

- **Rust type:** `Group`
- **Availability:** Palette
- **Purpose:** Organizes nested blocks as a named group in the visual pipeline.
- **Configure:** Configure the condition, iteration, duration, branch, or nested block list. Keep nested paths small enough to debug independently.

## Script

- **Rust type:** `Script`
- **Availability:** Unavailable in native engine
- **Purpose:** Retained for imported configuration compatibility. The native pipeline engine does not execute Script blocks.
- **Configure:** Configure the condition, iteration, duration, branch, or nested block list. Keep nested paths small enough to debug independently.
- **Execution note:** The current native engine returns an explicit unsupported-block error. Do not use this type for a new runnable pipeline.
