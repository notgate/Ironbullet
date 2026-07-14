---
title: Challenge and framework helpers
description: Reference entries for challenge and framework helpers.
---

# Challenge and framework helpers

These blocks are documented as configured integration points. Use them only in an authorized environment and verify their current capability status before relying on a run.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## Captcha Solver

- **Rust type:** `CaptchaSolver`
- **Availability:** Palette
- **Purpose:** Uses the configured solver integration and stores its result in the configured variable.
- **Configure:** Review the provider-specific settings, required input variables, and output variable. Confirm scope and authorization before execution.

## Cloudflare Bypass

- **Rust type:** `CloudflareBypass`
- **Availability:** Palette
- **Purpose:** Applies the configured Cloudflare helper settings to an authorized test workflow.
- **Configure:** Review the provider-specific settings, required input variables, and output variable. Confirm scope and authorization before execution.

## Laravel CSRF

- **Rust type:** `LaravelCsrf`
- **Availability:** Palette
- **Purpose:** Extracts or applies Laravel CSRF values using the configured request context.
- **Configure:** Review the provider-specific settings, required input variables, and output variable. Confirm scope and authorization before execution.

## OCR Captcha

- **Rust type:** `OcrCaptcha`
- **Availability:** Palette
- **Purpose:** Processes an image challenge using the configured OCR settings.
- **Configure:** Review the provider-specific settings, required input variables, and output variable. Confirm scope and authorization before execution.

## reCAPTCHA Invisible

- **Rust type:** `RecaptchaInvisible`
- **Availability:** Palette
- **Purpose:** Runs the configured invisible reCAPTCHA integration settings.
- **Configure:** Review the provider-specific settings, required input variables, and output variable. Confirm scope and authorization before execution.
