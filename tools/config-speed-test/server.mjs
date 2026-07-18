import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const valueFor = (name, fallback) => {
  const index = args.indexOf(name);
  return index >= 0 ? Number(args[index + 1]) : fallback;
};
const port = valueFor("--port", 18787);
const proxyPort = valueFor("--proxy-port", 18788);
const tokenIndex = args.indexOf("--token");
const token = tokenIndex >= 0 ? args[tokenIndex + 1] : "manual";
const root = path.dirname(fileURLToPath(import.meta.url));

const freshStats = () => ({
  requests: 0,
  duplicateRequests: 0,
  ids: new Set(),
  active: 0,
  maxActive: 0,
  startedAt: 0,
  endedAt: 0,
});
let targetStats = freshStats();
let proxyStats = freshStats();

const publicStats = (stats) => ({
  requests: stats.requests,
  uniqueRequests: stats.ids.size,
  duplicateRequests: stats.duplicateRequests,
  active: stats.active,
  maxActive: stats.maxActive,
  elapsedMs: stats.startedAt && stats.endedAt ? stats.endedAt - stats.startedAt : 0,
});

const sendJson = (res, status, value) => {
  const body = JSON.stringify(value);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body),
    "cache-control": "no-store",
    "access-control-allow-origin": "*",
  });
  res.end(body);
};

const target = http.createServer((req, res) => {
  const url = new URL(req.url ?? "/", `http://127.0.0.1:${port}`);
  if (url.pathname === "/health") return sendJson(res, 200, { ok: true, token });
  if (url.pathname === "/api/reset") {
    targetStats = freshStats();
    proxyStats = freshStats();
    return sendJson(res, 200, { ok: true });
  }
  if (url.pathname === "/api/stats") {
    return sendJson(res, 200, {
      target: publicStats(targetStats),
      proxy: publicStats(proxyStats),
    });
  }
  if (url.pathname === "/api/check") {
    const id = url.searchParams.get("id") ?? "";
    const delayMs = Math.max(0, Math.min(5_000, Number(url.searchParams.get("delay_ms") ?? 0)));
    const now = Date.now();
    if (!targetStats.startedAt) targetStats.startedAt = now;
    targetStats.requests += 1;
    if (targetStats.ids.has(id)) targetStats.duplicateRequests += 1;
    targetStats.ids.add(id);
    targetStats.active += 1;
    targetStats.maxActive = Math.max(targetStats.maxActive, targetStats.active);
    setTimeout(() => {
      targetStats.active -= 1;
      targetStats.endedAt = Date.now();
      sendJson(res, 200, { ok: true, id });
    }, delayMs);
    return;
  }
  if (url.pathname === "/" || url.pathname === "/index.html") {
    const body = fs.readFileSync(path.join(root, "index.html"));
    res.writeHead(200, {
      "content-type": "text/html; charset=utf-8",
      "content-length": body.length,
      "cache-control": "no-store",
    });
    return res.end(body);
  }
  sendJson(res, 404, { error: "not found" });
});

target.listen(port, "127.0.0.1", () => {
  console.log(`speed target: http://127.0.0.1:${port}`);
});

const proxy = http.createServer((req, res) => {
  let destination;
  try {
    destination = new URL(req.url ?? "");
  } catch {
    return sendJson(res, 400, { error: "absolute HTTP proxy URL required" });
  }
  if (!(["127.0.0.1", "localhost"].includes(destination.hostname))) {
    return sendJson(res, 403, { error: "benchmark proxy only permits loopback targets" });
  }

  const now = Date.now();
  if (!proxyStats.startedAt) proxyStats.startedAt = now;
  proxyStats.requests += 1;
  proxyStats.active += 1;
  proxyStats.maxActive = Math.max(proxyStats.maxActive, proxyStats.active);

  const upstream = http.request(
    {
      hostname: destination.hostname,
      port: destination.port || 80,
      method: req.method,
      path: `${destination.pathname}${destination.search}`,
      headers: { ...req.headers, host: destination.host },
    },
    (upstreamResponse) => {
      res.writeHead(upstreamResponse.statusCode ?? 502, upstreamResponse.headers);
      upstreamResponse.pipe(res);
      upstreamResponse.on("end", () => {
        proxyStats.active -= 1;
        proxyStats.endedAt = Date.now();
      });
    },
  );
  upstream.on("error", (error) => {
    proxyStats.active -= 1;
    proxyStats.endedAt = Date.now();
    sendJson(res, 502, { error: error.message });
  });
  req.pipe(upstream);
});

proxy.on("connect", (_req, socket) => {
  socket.end("HTTP/1.1 405 Method Not Allowed\r\nConnection: close\r\n\r\n");
});

proxy.listen(proxyPort, "127.0.0.1", () => {
  console.log(`loopback proxy: http://127.0.0.1:${proxyPort}`);
});

const shutdown = () => {
  proxy.close();
  target.close(() => process.exit(0));
};
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
