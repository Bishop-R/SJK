"""Make the Tag toy's icon (`toy_tag`, SJK's own).

The Tag toy sticks a premade picture on a wall, as sprays do. Its pictures are art in
the hub's `sjk_tags` pack; the client bundles only the toy's icon, drawn here from
shapes: `toy_tag.png`, 128x128 like `gfx/mp/f_icon_*`, a spray can tilted to the right,
a puff of paint leaving its nozzle, on transparency. (Until 31/10/2026 it also wrote
`tags.shader`, a decal shader per tag; tags now have a pass of their own,
`crates/sjk-viewer/src/tag_render.rs`, which draws the pack's pictures at full size.)

Usage: python scripts/tag_assets.py [output folder]
(default `crates/sjk-viewer/assets/tags`).
"""

import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SIZE = 128
SCALE = 4
BIG = SIZE * SCALE


def can():
    """The can, upright, on a transparent square of BIG pixels."""
    image = Image.new("RGBA", (BIG, BIG), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    left, right = 150, 330
    top, bottom = 170, 470
    # The body: shaded across, light from the left.
    for x in range(left, right):
        t = (x - left) / (right - left)
        light = 1.0 - abs(t - 0.3) * 1.3
        shade = max(0.35, min(1.0, light))
        colour = (int(232 * shade), int(84 * shade), int(52 * shade), 255)
        draw.line([(x, top), (x, bottom)], fill=colour)
    # Rounded shoulders and foot.
    draw.ellipse([left, top - 26, right, top + 26], fill=(196, 70, 44, 255))
    draw.ellipse([left, bottom - 22, right, bottom + 22], fill=(150, 52, 34, 255))
    # The label band.
    for x in range(left, right):
        t = (x - left) / (right - left)
        shade = max(0.45, min(1.0, 1.0 - abs(t - 0.3) * 1.2))
        draw.line(
            [(x, 290), (x, 360)], fill=(int(246 * shade), int(240 * shade), int(226 * shade), 255)
        )
    draw.rectangle([left + 34, 312, right - 34, 338], fill=(40, 40, 46, 255))
    # The cap and the nozzle.
    draw.rounded_rectangle([190, 96, 290, 160], radius=18, fill=(214, 216, 222, 255))
    draw.rectangle([198, 100, 214, 156], fill=(240, 242, 246, 255))
    draw.rounded_rectangle([222, 62, 262, 100], radius=8, fill=(60, 60, 66, 255))
    draw.rectangle([256, 72, 276, 86], fill=(36, 36, 40, 255))
    # An outline, so it reads on light and dark menus.
    outline = Image.new("RGBA", (BIG, BIG), (0, 0, 0, 0))
    alpha = image.split()[3].filter(ImageFilter.MaxFilter(13))
    outline.putalpha(alpha.point(lambda a: 220 if a else 0))
    outline.paste(image, (0, 0), image)
    return outline


def mist(random_state):
    """A puff of paint drops, spreading right of the nozzle."""
    image = Image.new("RGBA", (BIG, BIG), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    for _ in range(170):
        distance = random_state.uniform(20, 190)
        spread = distance * 0.42
        x = 290 + distance
        y = 79 + random_state.uniform(-spread, spread)
        radius = random_state.uniform(3, 11) * (1.0 - distance / 260)
        alpha = int(255 * (1.0 - distance / 230))
        draw.ellipse(
            [x - radius, y - radius, x + radius, y + radius],
            fill=(255, 120, 70, max(40, alpha)),
        )
    return image


def icon():
    random_state = random.Random(27102026)
    image = can().rotate(-24, resample=Image.BICUBIC, center=(240, 280), translate=(-30, 20))
    puff = mist(random_state).rotate(-24, resample=Image.BICUBIC, center=(240, 280), translate=(-30, 20))
    puff.paste(image, (0, 0), image)
    return puff.resize((SIZE, SIZE), Image.LANCZOS)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "crates/sjk-viewer/assets/tags")
    out.mkdir(parents=True, exist_ok=True)
    icon().save(out / "toy_tag.png", optimize=True)
    print(f"wrote {out / 'toy_tag.png'}")


if __name__ == "__main__":
    main()
