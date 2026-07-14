# Quick Start Guide - IronBullet v0.5.3-rc1

## What's New in This Release?

**Critical Bug Fixes:**
1. **Issue #56:** SOCKS5 proxies now work in Jobs mode (auto-detection from port numbers)
2. **Memory Leak:** Fixed unbounded memory growth with Shadowsocks proxies

---

## Testing the Fixes

### Test 1: SOCKS5 Auto-Detection (Issue #56)

**Problem:** Previously, plain `127.0.0.1:1080` was treated as HTTP in Jobs mode.

**Test Steps:**

1. Create a test proxy file:
   ```bash
   echo "127.0.0.1:1080" > test_socks5.txt
   ```

2. Configure a SOCKS5 proxy on port 1080 (or use Tor Browser's port 9150)

3. In IronBullet:
   - Create a new Job
   - Set proxy file to `test_socks5.txt`
   - Add an HttpRequest block to `https://httpbin.org/ip`
   - Run the job

4. **Expected Result:**
   - Response shows proxy IP (not your real IP)
   - No connection errors
   - Status: SUCCESS

5. **Before Fix:**
   - Connection failed (tried HTTP instead of SOCKS5)
   - Status: ERROR

### Test 2: Memory Stability with Shadowsocks

**Problem:** Memory usage grew to 100% during extended runs.

**Test Steps:**

1. Configure a Shadowsocks proxy in your proxy file:
   ```
   ss://aes-128-gcm:yourpassword@server.example.com:8388
   ```

2. Create a job with:
   - 100+ threads
   - Large wordlist (10,000+ lines)
   - Shadowsocks proxy enabled

3. Monitor memory usage:
   - Linux: `htop` or `top`
   - Windows: Task Manager

4. **Expected Result:**
   - Memory usage stable (does not grow indefinitely)
   - CPU usage normal
   - WebView2 remains responsive

5. **Before Fix:**
   - Memory grew to 100% over 10-15 minutes
   - UI became unresponsive
   - Application had to be force-closed

---

## Auto-Detected Proxy Ports

The following ports are now automatically detected as SOCKS5 (no need for explicit `socks5://` prefix):

| Port | Description |
|------|-------------|
| 1080 | Standard SOCKS5 port |
| 1081 | Alternative SOCKS5 port |
| 9050 | Tor SOCKS5 port |
| 9150 | Tor Browser SOCKS5 port |
| 1086 | Shadowsocks local (common) |
| 1087 | Shadowsocks local (common) |
| 1088 | Shadowsocks local (common) |

**Example:**
```
# All of these now work without explicit type:
127.0.0.1:1080          # Auto-detected as SOCKS5
127.0.0.1:9050          # Auto-detected as SOCKS5 (Tor)
192.168.1.1:8080        # Still HTTP (not a SOCKS5 port)
socks5://host:8080      # Explicit prefix always works
```

---

## Running the Application

### Linux

```bash
cd /root/ironbullet
./target/release/ironbullet
```

**Note:** Both `ironbullet` and `reqflow-sidecar` must be in the same directory.

### Verify Installation

```bash
# Check version
./target/release/ironbullet --version
# Should show: ironbullet 0.5.3-rc1

# Check help
./target/release/ironbullet --help
```

---

## Proxy Configuration Best Practices

### Format Support

IronBullet supports all these formats:

```
# Explicit schemes (always work)
http://proxy.com:8080
https://proxy.com:443
socks4://proxy.com:1080
socks5://proxy.com:1080
ss://method:password@server.com:8388

# Plain host:port (auto-detected by port)
proxy.com:1080          # → SOCKS5 (port 1080)
proxy.com:8080          # → HTTP (port 8080)
proxy.com:9050          # → SOCKS5 (Tor port)

# With authentication
user:pass@proxy.com:1080     # Auto-detected
proxy.com:1080:user:pass     # OpenBullet format

# Explicit type (5-part format)
socks5:proxy.com:1080:user:pass
http:proxy.com:8080:user:pass
```

### Mixed Proxy Files

You can mix different proxy types in one file:

```txt
# my_proxies.txt
http://httpproxy.com:8080
127.0.0.1:1080                    # Auto → SOCKS5
socks5://another.com:1081
ss://aes-128-gcm:pass@ss.com:8388
```

---

## Troubleshooting

### Issue: Proxies still not working

**Solution:**
1. Check proxy is actually running on that port
2. Test proxy with curl:
   ```bash
   curl --socks5 127.0.0.1:1080 https://httpbin.org/ip
   ```
3. Verify port number in proxy file is correct
4. Check proxy type - if uncertain, use explicit prefix:
   ```
   socks5://127.0.0.1:1080
   ```

### Issue: Memory still growing

**Solution:**
1. Verify you're running v0.5.3-rc1:
   ```bash
   ./ironbullet --version
   ```
2. Check Task Manager / htop for actual memory usage
3. Close and restart application
4. Report issue with:
   - Memory usage pattern
   - Job configuration
   - Proxy types used
   - Duration before problem appears

### Issue: Build failed

**Common causes:**
1. GUI not built: Run `cd gui && npm install && npm run build`
2. Missing dependencies: Run `cargo build` to see errors
3. Disk space: Release build needs ~2GB free space

---

## Reporting Issues

Found a bug in RC1? Please report:

**GitHub:** https://github.com/notgate/Ironbullet/issues

**Include:**
- Version: 0.5.3-rc1
- OS and version
- Proxy type and configuration
- Job settings (threads, wordlist size)
- Steps to reproduce
- Logs (if available)
- Screenshots (if relevant)

---

## What's NOT in RC1

- **Issue #51 (Drag-and-drop):** Deferred to v0.5.4
  - Reason: Critical bugs took priority
  - Status: Tracked in GitHub

---

## Next Release

**v0.5.3 (Final)** will include:
- RC1 bug fixes (if any found during testing)
- Possibly Issue #51 (drag-and-drop)
- Performance improvements
- Additional tests

**Target Date:** After RC1 testing (1-2 weeks)

---

## Thank You!

Your testing and feedback help make IronBullet better. Thank you for trying RC1!

**Questions?** Open a GitHub issue or discussion.

---

**Version:** 0.5.3-rc1
**Build Date:** 2026-04-05
**License:** MIT
