---
title: Security and authentication utilities
description: Reference entries for security and authentication utilities.
---

# Security and authentication utilities

Use these blocks to manage tokens and headers in an authorized workflow. Do not rely on a type being documented here as proof of interface availability.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## JWT Token

- **Rust type:** `JwtToken`
- **Availability:** Palette
- **Purpose:** Creates, parses, or validates a JWT according to the selected token operation.
- **Configure:** Set the relevant input, key/header/token fields, and output variable. Keep secrets out of saved public configurations.

## Header Spoof

- **Rust type:** `HeaderSpoof`
- **Availability:** Palette
- **Purpose:** Builds configured forwarding or client-identification headers for a request context.
- **Configure:** Set the relevant input, key/header/token fields, and output variable. Keep secrets out of saved public configurations.

## NuData Sensor

- **Rust type:** `NuDataSensor`
- **Availability:** Compatibility-only / imported configuration
- **Purpose:** A native settings model retained for imported configurations. The current GUI has no dedicated settings editor, so treat it as compatibility-only.
- **Configure:** Set the relevant input, key/header/token fields, and output variable. Keep secrets out of saved public configurations.
- **Interface note:** The native model can deserialize this type, but the GUI does not currently provide a dedicated editor.
