# IronBullet config speed test

This fixture separates runner performance from public-network noise.

## Browser baseline

```powershell
node .\tools\config-speed-test\server.mjs
```

Open `http://127.0.0.1:18787/`. The page sends unique request IDs at controlled concurrency and reports throughput, p50/p95 latency, missing IDs, duplicates, and peak server concurrency.

## Real IronBullet CLI benchmark

After a Windows release build:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\config-speed-test\run.ps1
powershell -ExecutionPolicy Bypass -File .\tools\config-speed-test\run.ps1 -UseLocalProxy
```

Both runs execute `speed-test.rfx` through the real CLI runner. The local proxy accepts loopback targets only. A run fails if any request is missing, duplicated, serialized unexpectedly, or bypasses the selected proxy.

## Packaged xray cleanup smoke test

```powershell
powershell -ExecutionPolicy Bypass -File ./tools/config-speed-test/test-xray-cleanup.ps1
```

This requires the versioned Windows bundle under `dist/`. It verifies that a bundled `xray.exe` process is observed during an encrypted-proxy CLI run, IronBullet exits within the test bound with code 0, and no new Xray PID or generated config survives shutdown.

Useful controls:

```powershell
.\tools\config-speed-test\run.ps1 -Requests 5000 -Threads 200 -DelayMs 10 -ResultPath .\direct.json
```

## Public proxy sample

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\config-speed-test\sample-public-proxies.ps1
```

This checks a small public HTTP-proxy sample with one anonymous request to `example.com` per candidate. It is deliberately separate from the deterministic benchmark: public proxy availability and latency are volatile and cannot prove or disprove an IronBullet race condition.
