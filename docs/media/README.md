# Ironbullet media

These assets document the real Ironbullet interface and the project-owned visual identity.

## Brand assets

- `ironbullet-logo.svg` — canonical sharp-path `I/B` monogram.
- `ironbullet-logo.png` — transparent 512×512 raster derivative.
- `ironbullet-hero-light.png` — transparent README hero for light GitHub themes.
- `ironbullet-hero-dark.png` — transparent README hero for dark GitHub themes.
- `ironbullet-interface.png` — cropped real v0.6.1 Windows interface capture used as the hero source.

Runtime/package copies are generated at:

- `assets/brand/ironbullet-icon.png`
- `assets/brand/ironbullet.ico`
- `gui/static/ironbullet-logo.png`
- `release/IronBullet.AppDir/ironbullet.png`
- `release/IronBullet.AppDir/.DirIcon`

## Interface provenance

The interface source was captured from the public `v0.6.1` Windows release:

- Asset: `ironbullet-v0.6.1-windows-x64.zip`
- Public URL: <https://github.com/notgate/Ironbullet/releases/download/v0.6.1/ironbullet-v0.6.1-windows-x64.zip>
- Archive SHA-256: `e97ca66365bbb56db6608435eba99493ffb375d906844c00a896153f011cb6ef`

The native Wry/WebView2 application was run on Windows under a temporary clean profile. The user's existing Ironbullet profile was moved aside before launch and restored afterward with all four files matching by relative path, size, and SHA-256.

The capture shows a neutral, unexecuted showcase:

- config name: `Signal Path`;
- blocks: `HTTP Request`, `Key Check`, and `Parse`;
- URL: the built-in neutral `https://example.com/api/` default;
- data: `signal-payload`;
- no account, password, private target, proxy, result, or local path.

The authoritative crop removes only non-application borders, the Windows taskbar, and the lower helper panel. It does not alter visible application status or invent controls.

## Hero construction

The heroes are deterministic Pillow compositions generated from `ironbullet-interface.png`. They use:

- an editorial magnification of the real three-block pipeline;
- near-monochrome treatment with 8% of the original UI color retained;
- chamfered panel masks;
- product-specific angular signal paths;
- a real alpha channel and separate light/dark trace contrast.

No interface pixels were AI-generated. No third-party logo or hero artwork is included. The visual hierarchy is an original Ironbullet treatment.

## Regeneration

From the repository root:

```bash
python3 scripts/branding/generate_brand_assets.py
```

Requires Pillow. After generation, verify the SVG, alpha extrema, README references, frontend build, and Rust build before committing.
