---
title: Contributing
description: Repository boundaries, validation, and documentation workflow.
---

## Repository layout

- `src/` — native Rust application, pipeline core, IPC, runner, imports, exports, and sidecar bridge
- `gui/` — Svelte desktop interface
- `sidecar/` — Go request-flow sidecar
- `docs/site/` — Starlight documentation source
- `docs/media/` — project-owned README and docs media

## Required checks

```bash
cargo fmt -- --check
cargo test --lib --no-default-features
cd gui && npm run build
cd ../docs/site && npm run build
```

Keep the capability-status page accurate whenever a block is exposed, hidden, implemented, or explicitly rejected.
