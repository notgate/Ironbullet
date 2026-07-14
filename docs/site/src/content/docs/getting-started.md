---
title: Install and build
description: Build Ironbullet and its frontend from source.
---

## Requirements

- Rust 1.80 or newer
- Node.js 20 or newer
- Go 1.23 or newer for the request-flow sidecar

## Build

```bash
git clone https://github.com/notgate/Ironbullet.git
cd Ironbullet

cd gui
npm ci
npm run build
cd ..

cd sidecar
go build -o reqflow-sidecar
cd ..

cargo build --release
```

The application binary and `reqflow-sidecar` must be deployed together.

## Core-engine verification

The GUI host has platform-specific native dependencies. The portable core suite can be verified independently:

```bash
cargo test --lib --no-default-features
```
