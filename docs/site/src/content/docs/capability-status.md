---
title: Capability status
description: Supported, unavailable, and in-progress pipeline capabilities.
---

This page is the current contract for the native pipeline engine.

## Supported

- HTTP request execution, response storage, parsing, and key checks
- Redirect control and explicit header precedence
- TCP, UDP, FTP, SSH, IMAP, SMTP, and POP3 protocol blocks where their platform dependencies are available
- JWT and header-spoof block types

## Explicitly unavailable

- **Script** blocks are not executed by the native pipeline engine.
- **WebSocket** blocks are not executed by the native pipeline engine.
- **Multipart** HTTP payloads are rejected pending a structured payload model.

These blocks are not offered for new pipelines in the interface. Existing imported configurations report a direct execution error.

## In progress

- A typed frontend settings editor for the native `NuDataSensor` block.
- Transport-level HTTP version enforcement.
