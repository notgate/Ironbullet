#!/usr/bin/env python3
"""Regenerate Ironbullet brand assets from the committed v0.6.1 interface crop."""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageOps
import json
import shutil

ROOT = Path(__file__).resolve().parents[2]
MEDIA = ROOT / "docs" / "media"
BRAND = ROOT / "assets" / "brand"
STATIC = ROOT / "gui" / "static"
SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 136 136" role="img" aria-labelledby="title desc">
  <title id="title">Ironbullet</title>
  <desc id="desc">An angular interlocking I and B monogram shaped from two forward bullet paths.</desc>
  <rect x="4" y="4" width="128" height="128" rx="26" fill="#0b0b0d"/>
  <path fill="#f4f4f5" fill-rule="evenodd" d="M28 23h46l35 28-21 17 21 18-35 27H28V23Zm20 19v18h20l14-9-14-9H48Zm0 35v19h20l14-10-14-9H48Z"/>
</svg>
"""


def scaled_points(points, scale):
    return [(int(x * scale), int(y * scale)) for x, y in points]


def make_logo():
    scale = 8
    image = Image.new("RGBA", (136 * scale, 136 * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(
        tuple(int(v * scale) for v in (4, 4, 132, 132)),
        radius=26 * scale,
        fill=(11, 11, 13, 255),
    )
    draw.polygon(
        scaled_points([(28, 23), (74, 23), (109, 51), (88, 68), (109, 86), (74, 113), (28, 113)], scale),
        fill=(244, 244, 245, 255),
    )
    for hole in (
        [(48, 42), (48, 60), (68, 60), (82, 51), (68, 42)],
        [(48, 77), (48, 96), (68, 96), (82, 86), (68, 77)],
    ):
        draw.polygon(scaled_points(hole, scale), fill=(11, 11, 13, 255))
    return image.resize((512, 512), Image.Resampling.LANCZOS)


def chamfer_mask(width, height, cut=12):
    mask = Image.new("L", (width, height), 0)
    ImageDraw.Draw(mask).polygon(
        [(cut, 0), (width, 0), (width, height - cut), (width - cut, height), (0, height), (0, cut)],
        fill=255,
    )
    return mask


def add_panel(canvas, image, position, size, shadow_alpha, border):
    x, y = position
    width, height = size
    resized = image.resize(size, Image.Resampling.LANCZOS)
    mask = chamfer_mask(width, height, 14)
    shadow_mask = Image.new("L", canvas.size, 0)
    ImageDraw.Draw(shadow_mask).bitmap((x + 14, y + 18), mask, fill=shadow_alpha)
    shadow_mask = shadow_mask.filter(ImageFilter.GaussianBlur(22))
    shadow = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    shadow.putalpha(shadow_mask)
    canvas.alpha_composite(shadow)
    layer = Image.new("RGBA", size, (0, 0, 0, 0))
    layer.paste(resized, (0, 0), mask)
    canvas.alpha_composite(layer, position)
    ImageDraw.Draw(canvas).line(
        [
            (x + 14, y),
            (x + width - 1, y),
            (x + width - 1, y + height - 14),
            (x + width - 14, y + height - 1),
            (x, y + height - 1),
            (x, y + 14),
            (x + 14, y),
        ],
        fill=border,
        width=2,
        joint="curve",
    )


def add_trace_field(canvas, dark):
    draw = ImageDraw.Draw(canvas)
    line = (238, 238, 240, 45) if dark else (13, 13, 15, 36)
    node = (245, 245, 246, 75) if dark else (10, 10, 12, 68)
    paths = [
        [(0, 115), (105, 115), (158, 168), (335, 168)],
        [(0, 170), (82, 170), (132, 220), (320, 220)],
        [(0, 250), (130, 250), (190, 310), (315, 310)],
        [(0, 330), (92, 330), (156, 394), (315, 394)],
        [(0, 570), (60, 570), (120, 510), (280, 510)],
        [(0, 625), (120, 625), (170, 575), (360, 575)],
    ]
    for path in paths:
        draw.line(path, fill=line, width=2)
        for x, y in path[1:-1]:
            draw.rectangle((x - 3, y - 3, x + 3, y + 3), fill=node)
    for y in (120, 164, 208):
        draw.line([(1440, y), (1520, y), (1560, y + 22), (1600, y + 22)], fill=line, width=2)
        draw.rectangle((1517, y - 3, 1523, y + 3), fill=node)


def make_hero(interface, dark):
    gray = ImageOps.grayscale(interface)
    gray_rgba = Image.merge("RGBA", (gray, gray, gray, interface.getchannel("A")))
    steel = Image.blend(gray_rgba, interface, 0.08)
    canvas = Image.new("RGBA", (1600, 700), (0, 0, 0, 0))
    add_trace_field(canvas, dark)
    add_panel(
        canvas,
        steel,
        (300, 28),
        (1240, 640),
        115 if dark else 78,
        (242, 242, 244, 55) if dark else (18, 18, 20, 52),
    )
    strip = steel.crop((310, 100, 1435, 310))
    add_panel(
        canvas,
        strip,
        (72, 468),
        (860, 161),
        145 if dark else 92,
        (245, 245, 246, 72) if dark else (15, 15, 17, 64),
    )
    draw = ImageDraw.Draw(canvas)
    link = (244, 244, 245, 72) if dark else (12, 12, 14, 62)
    for path in (
        [(932, 496), (972, 496), (992, 516), (1032, 516)],
        [(932, 590), (972, 590), (992, 570), (1032, 570)],
    ):
        draw.line(path, fill=link, width=2)
        for x, y in path[1:-1]:
            draw.rectangle((x - 3, y - 3, x + 3, y + 3), fill=link)
    return canvas


def main():
    MEDIA.mkdir(parents=True, exist_ok=True)
    BRAND.mkdir(parents=True, exist_ok=True)
    STATIC.mkdir(parents=True, exist_ok=True)
    (MEDIA / "ironbullet-logo.svg").write_text(SVG, encoding="utf-8", newline="\n")
    logo = make_logo()
    logo.save(MEDIA / "ironbullet-logo.png", optimize=True)
    logo.save(BRAND / "ironbullet-icon.png", optimize=True)
    logo.save(
        BRAND / "ironbullet.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    shutil.copy2(BRAND / "ironbullet-icon.png", STATIC / "ironbullet-logo.png")
    for destination in (
        ROOT / "release" / "IronBullet.AppDir" / "ironbullet.png",
        ROOT / "release" / "IronBullet.AppDir" / ".DirIcon",
    ):
        if destination.exists():
            shutil.copy2(BRAND / "ironbullet-icon.png", destination)
    interface = Image.open(MEDIA / "ironbullet-interface.png").convert("RGBA")
    interface.convert("RGB").save(ROOT / "docs" / "preview.png", optimize=True)
    make_hero(interface, False).save(MEDIA / "ironbullet-hero-light.png", optimize=True)
    make_hero(interface, True).save(MEDIA / "ironbullet-hero-dark.png", optimize=True)
    outputs = [
        MEDIA / "ironbullet-logo.svg",
        MEDIA / "ironbullet-logo.png",
        MEDIA / "ironbullet-hero-light.png",
        MEDIA / "ironbullet-hero-dark.png",
        BRAND / "ironbullet-icon.png",
        BRAND / "ironbullet.ico",
        STATIC / "ironbullet-logo.png",
        ROOT / "docs" / "preview.png",
    ]
    print(json.dumps({str(path.relative_to(ROOT)): path.stat().st_size for path in outputs}, indent=2))


if __name__ == "__main__":
    main()
