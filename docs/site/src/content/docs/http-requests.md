---
title: HTTP requests
description: Configure an HTTP Request block and consume its response safely.
---

An HTTP Request block interpolates variables, sends a request, and stores the resulting response under its configured response variable.

## Header precedence

Explicit headers take precedence over generated metadata. If you supply `Content-Type` or `Authorization`, Ironbullet does not overwrite it.

## Failure behavior

Before each request, response-scoped state is cleared. A failed safe-mode request cannot leave an earlier status code, body, header, or cookie available to a later block. The current error is available at:

```text
<response-variable>.ERROR
```

## Current limitations

- Multipart requests are rejected until a structured field/file schema is implemented.
- HTTP-version selection is not yet a transport-level guarantee.
