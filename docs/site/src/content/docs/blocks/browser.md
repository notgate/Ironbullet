---
title: Browser automation
description: Reference entries for browser automation.
---

# Browser automation

These blocks operate on the browser context created by Browser Open. Validate selectors and navigation in Debug mode before using a full job.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## Browser Open

- **Rust type:** `BrowserOpen`
- **Availability:** Palette
- **Purpose:** Starts or attaches the browser context used by subsequent browser blocks.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Navigate To

- **Rust type:** `NavigateTo`
- **Availability:** Palette
- **Purpose:** Navigates the active browser page to the configured URL.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Click Element

- **Rust type:** `ClickElement`
- **Availability:** Palette
- **Purpose:** Finds an element using the configured selector and performs a click.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Type Text

- **Rust type:** `TypeText`
- **Availability:** Palette
- **Purpose:** Finds an element and enters configured text.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Wait For Element

- **Rust type:** `WaitForElement`
- **Availability:** Palette
- **Purpose:** Waits until the configured selector is available or the timeout expires.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Get Element Text

- **Rust type:** `GetElementText`
- **Availability:** Palette
- **Purpose:** Reads text from the configured element and stores it in an output variable.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Get DOM

- **Rust type:** `GetDom`
- **Availability:** Palette
- **Purpose:** Stores the complete active page DOM, or the inner/outer HTML of the first matching CSS selector.
- **Configure:** Leave Selector empty for the full page; set **Include selected element** for outer HTML; use a named output variable.

## Screenshot

- **Rust type:** `Screenshot`
- **Availability:** Palette
- **Purpose:** Captures the active browser page using the configured output settings.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.

## Execute JS

- **Rust type:** `ExecuteJs`
- **Availability:** Palette
- **Purpose:** Runs configured JavaScript in the active browser page context.
- **Configure:** Choose stable selectors, set timeouts deliberately, and configure the browser/output variable fields before running a full job.
