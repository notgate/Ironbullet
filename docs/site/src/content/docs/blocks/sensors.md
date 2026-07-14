---
title: Sensor blocks
description: Reference entries for sensor blocks.
---

# Sensor blocks

Sensor blocks have site- and provider-specific settings. Treat their outputs as inputs to a controlled, authorized test pipeline.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## XACF Sensor

- **Rust type:** `XacfSensor`
- **Availability:** Palette
- **Purpose:** Runs the configured XACF sensor workflow and writes its configured output.
- **Configure:** Supply the exact input variables and provider settings expected by the sensor, then record the configured output variable.

## DataDome Sensor

- **Rust type:** `DataDomeSensor`
- **Availability:** Palette
- **Purpose:** Runs the configured DataDome sensor workflow and writes its configured output.
- **Configure:** Supply the exact input variables and provider settings expected by the sensor, then record the configured output variable.

## Akamai V3 Sensor

- **Rust type:** `AkamaiV3Sensor`
- **Availability:** Palette
- **Purpose:** Runs the configured Akamai V3 sensor workflow and writes its configured output.
- **Configure:** Supply the exact input variables and provider settings expected by the sensor, then record the configured output variable.
