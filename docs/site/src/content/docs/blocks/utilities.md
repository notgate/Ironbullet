---
title: Utilities and integration
description: Reference entries for utilities and integration.
---

# Utilities and integration

Use utility blocks to manage variables, diagnostics, cookies, events, and extensions.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## Log

- **Rust type:** `Log`
- **Availability:** Palette
- **Purpose:** Writes the configured message to the run log.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.

## Set Variable

- **Rust type:** `SetVariable`
- **Availability:** Palette
- **Purpose:** Assigns a literal or interpolated value to a named variable.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.

## Clear Cookies

- **Rust type:** `ClearCookies`
- **Availability:** Palette
- **Purpose:** Clears the active cookie state for the current execution context.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.

## Webhook

- **Rust type:** `Webhook`
- **Availability:** Palette
- **Purpose:** Sends an outbound HTTP callback using the configured webhook settings.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.

## WebSocket

- **Rust type:** `WebSocket`
- **Availability:** Unavailable in native engine
- **Purpose:** Retained for imported configuration compatibility. The native pipeline engine does not execute WebSocket blocks.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.
- **Execution note:** The current native engine returns an explicit unsupported-block error. Do not use this type for a new runnable pipeline.

## Random User Agent

- **Rust type:** `RandomUserAgent`
- **Availability:** Palette
- **Purpose:** Selects a user-agent value and stores it in the configured output variable.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.

## Random Data

- **Rust type:** `RandomData`
- **Availability:** Palette
- **Purpose:** Generates data using the selected type, length, range, or format settings.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.

## Plugin Block

- **Rust type:** `Plugin`
- **Availability:** Palette
- **Purpose:** Invokes a block provided by a loaded plugin and its declared settings schema.
- **Configure:** Set the type-specific input, message, variable name, callback, or plugin configuration and verify it in Debug mode.
