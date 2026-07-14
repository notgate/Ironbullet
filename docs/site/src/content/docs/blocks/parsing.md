---
title: Parsing blocks
description: Reference entries for parsing blocks.
---

# Parsing blocks

Turn response text, structured payloads, and cookies into named pipeline variables.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## Parse

- **Rust type:** `Parse`
- **Availability:** Palette
- **Purpose:** Uses the unified parser settings to extract values with the selected parse mode.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.

## Parse LR

- **Rust type:** `ParseLR`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Extracts text between left and right delimiters.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.

## Parse Regex

- **Rust type:** `ParseRegex`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Extracts capture data using a regular expression.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.

## Parse JSON

- **Rust type:** `ParseJSON`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Extracts a value from a JSON document using the configured path.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.

## Parse CSS

- **Rust type:** `ParseCSS`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Extracts text or an attribute from an HTML document using a CSS selector.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.

## Parse XPath

- **Rust type:** `ParseXPath`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Extracts content from XML or HTML using an XPath expression.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.

## Parse Cookie

- **Rust type:** `ParseCookie`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Extracts a named cookie value from an input cookie collection.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.

## Lambda Parser

- **Rust type:** `LambdaParser`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** Applies the legacy lambda-parser settings retained for imported configurations.
- **Configure:** Choose the input variable, extraction rule, output variable, and whether the extracted value should be captured.
- **Compatibility note:** Preserve the block when editing an imported configuration unless you intentionally migrate it to a supported palette block.
