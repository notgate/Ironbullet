---
title: Request and protocol blocks
description: Reference entries for request and protocol blocks.
---

# Request and protocol blocks

Build HTTP and protocol-level interactions. Use only with endpoints and systems you are authorized to assess.

All entries use the selected block's settings panel. The focused configuration guidance below describes the fields to review before using the block.

## HTTP Request

- **Rust type:** `HttpRequest`
- **Availability:** Palette
- **Purpose:** Creates an HTTP request, applies configured headers, cookies, body, redirect policy, and stores the named response.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## TCP Request

- **Rust type:** `TcpRequest`
- **Availability:** Palette
- **Purpose:** Sends a configured TCP payload to a target host and port.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## UDP Request

- **Rust type:** `UdpRequest`
- **Availability:** Palette
- **Purpose:** Sends a configured UDP datagram to a target host and port.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## FTP Request

- **Rust type:** `FtpRequest`
- **Availability:** Palette
- **Purpose:** Connects to an FTP service and performs the selected protocol operation.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## SSH Request

- **Rust type:** `SshRequest`
- **Availability:** Palette
- **Purpose:** Connects to an SSH service using the configured credentials and command settings.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## IMAP Request

- **Rust type:** `ImapRequest`
- **Availability:** Palette
- **Purpose:** Connects to an IMAP mailbox and performs the selected mail operation.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## SMTP Request

- **Rust type:** `SmtpRequest`
- **Availability:** Palette
- **Purpose:** Connects to an SMTP service and performs the selected mail operation.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.

## POP Request

- **Rust type:** `PopRequest`
- **Availability:** Palette
- **Purpose:** Connects to a POP service and performs the selected mailbox operation.
- **Configure:** Configure the endpoint, transport-specific connection fields, request payload, timeout, and named response/output fields. For HTTP, review redirect, TLS, cookie, and response-variable settings.
