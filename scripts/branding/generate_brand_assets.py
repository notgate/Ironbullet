#!/usr/bin/env python3
"""Regenerate Ironbullet brand assets from the committed v0.6.1 interface crop."""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter
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


def rounded_mask(size, radius):
    mask = Image.new("L", size, 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, size[0] - 1, size[1] - 1), radius=radius, fill=255
    )
    return mask


def make_hero(interface, dark):
    """Frame the real interface in a single Fluent-style rounded surface."""
    background = (12, 13, 15, 255) if dark else (242, 244, 247, 255)
    glow_alpha = 38 if dark else 26
    frame_fill = (26, 28, 31, 255) if dark else (255, 255, 255, 255)
    border = (49, 53, 58, 255) if dark else (191, 196, 202, 255)
    canvas = Image.new("RGBA", (1600, 940), background)

    glow = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse((-250, -250, 760, 760), fill=(0, 120, 212, glow_alpha))
    glow_draw.ellipse((1040, 610, 1850, 1320), fill=(0, 180, 180, glow_alpha // 2))
    canvas.alpha_composite(glow.filter(ImageFilter.GaussianBlur(100)))

    # The committed source capture begins mid-way through its native title bar.
    # Trim that damaged strip, then present the unmodified real interface beneath
    # a complete outer application title bar rather than showing a visibly clipped
    # window at the top of the README hero.
    source_top_trim = 18
    titlebar_height = 34
    card_width = 1450
    content = interface.crop((0, source_top_trim, interface.width, interface.height))
    content_height = round(content.height * card_width / content.width)
    screenshot = content.resize((card_width, content_height), Image.Resampling.LANCZOS)
    card_height = titlebar_height + content_height
    x = (canvas.width - card_width) // 2
    y = 88

    shadow = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle(
        (x + 8, y + 16, x + card_width + 8, y + card_height + 16),
        radius=24,
        fill=(0, 0, 0, 105),
    )
    canvas.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(18)))

    titlebar_fill = (31, 33, 36, 255) if dark else (249, 250, 252, 255)
    titlebar_text = (209, 213, 219, 255) if dark else (48, 52, 57, 255)
    window = Image.new("RGBA", (card_width, card_height), frame_fill)
    window_draw = ImageDraw.Draw(window)
    window_draw.rectangle((0, 0, card_width, titlebar_height), fill=titlebar_fill)
    window_draw.line((0, titlebar_height - 1, card_width, titlebar_height - 1), fill=border, width=1)
    window_draw.text((18, 9), "Ironbullet  ·  Signal Path", fill=titlebar_text)
    for control_x in (card_width - 70, card_width - 45, card_width - 20):
        window_draw.ellipse((control_x, 14, control_x + 5, 19), fill=titlebar_text)
    window.alpha_composite(screenshot, (0, titlebar_height))
    window.putalpha(rounded_mask(window.size, 21))

    frame = Image.new("RGBA", (card_width + 12, card_height + 12), frame_fill)
    frame.putalpha(rounded_mask(frame.size, 27))
    canvas.alpha_composite(frame, (x - 6, y - 6))
    canvas.alpha_composite(window, (x, y))
    ImageDraw.Draw(canvas).rounded_rectangle(
        (x - 6, y - 6, x + card_width + 5, y + card_height + 5),
        radius=27,
        outline=border,
        width=2,
    )
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
