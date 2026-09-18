#!/usr/bin/env python3
"""Draws RF-Organ's branding assets.

These are not what the package currently carries. The branding in
`package/branding/` is now artwork of the instrument itself, and running this
script replaces it - so it refuses to unless `--force` says that is meant.
Nothing in the build runs it; it is kept because a drawing that can be rebuilt
from the repository is worth having, and because it documents the palette the
artwork was matched to.

RackForge's manifest schema 3 wants three PNGs at exact sizes: a 512x512 icon,
a 1600x400 banner and a 1920x1080 splash. They are drawn here rather than
painted so that the package can be rebuilt from the repository, and they carry
no lettering, which keeps the drawings free of any font's licence.

The subject is the instrument's own control: nine drawbars in the colours a
console gives them - two brown, four white and three black, the first black
one being the 2 2/3' as Hammond's own manual describes it - pulled to the
registration of the Gospel Full preset. Colours come from the play surface in
`package/web/style.css` so the package and its interface look like one thing.

    python tools/make-artwork.py

Requires Pillow. The generated files live in `package/branding/`.
"""

from __future__ import annotations

import os
import sys
from PIL import Image, ImageDraw, ImageFilter

GROUND = (0x17, 0x12, 0x0D)
DEEP = (0x0D, 0x0A, 0x07)
GLOW = (0x3B, 0x28, 0x1A)
WOOD = (0x4C, 0x2D, 0x1B)
GOLD = (0xC9, 0xA0, 0x58)
IVORY = (0xEF, 0xE4, 0xCE)
EDGE = (0x12, 0x0F, 0x0C)
LINE = (0x5F, 0x51, 0x42)

# Two brown, four white, three black: 16', 5 1/3', 8', 4', 2 2/3', 2', 1 3/5',
# 1 1/3', 1'.
DRAWBAR_COLOURS = [WOOD, WOOD, IVORY, IVORY, EDGE, IVORY, EDGE, EDGE, IVORY]
# The Gospel Full preset, so the artwork shows a registration a player set.
REGISTRATION = [8, 8, 8, 8, 6, 8, 4, 8, 6]


def blend(first, second, amount):
    return tuple(
        round(one + (other - one) * amount) for one, other in zip(first, second)
    )


def background(size, glow_centre, glow_radius):
    """A dark console body with one warm light on it."""
    width, height = size
    image = Image.new("RGB", size, GROUND)
    draw = ImageDraw.Draw(image)
    for row in range(height):
        amount = row / max(height - 1, 1)
        draw.line(
            [(0, row), (width, row)],
            fill=blend(blend(GROUND, GLOW, 0.35), DEEP, amount**1.4),
        )

    light = Image.new("L", size, 0)
    spot = ImageDraw.Draw(light)
    centre_x, centre_y = glow_centre
    spot.ellipse(
        [
            centre_x - glow_radius,
            centre_y - glow_radius,
            centre_x + glow_radius,
            centre_y + glow_radius,
        ],
        fill=150,
    )
    light = light.filter(ImageFilter.GaussianBlur(glow_radius * 0.55))
    return Image.composite(Image.new("RGB", size, blend(GLOW, GOLD, 0.25)), image, light)


def drawbars(image, box, travel, radius):
    """Nine drawbars riding in a panel, each pulled out by its registration.

    `box` is the panel plate; the bars stand out of its top edge by up to
    `travel`, which is what makes a registration readable at a glance.
    """
    draw = ImageDraw.Draw(image, "RGBA")
    left, top, right, bottom = box
    span = right - left
    pitch = span / len(REGISTRATION)
    bar = pitch * 0.62

    # The panel the drawbars come out of, lit from above so the bars have
    # something to stand against.
    plate = Image.new("RGBA", (round(right - left), round(bottom - top)), (0, 0, 0, 0))
    plate_draw = ImageDraw.Draw(plate)
    height = plate.height
    for row in range(height):
        amount = row / max(height - 1, 1)
        plate_draw.line(
            [(0, row), (plate.width, row)],
            fill=blend((0x3D, 0x34, 0x2B), (0x20, 0x1B, 0x16), amount) + (255,),
        )
    mask = Image.new("L", plate.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, plate.width - 1, plate.height - 1], radius=radius * 1.6, fill=255
    )
    image.paste(plate, (round(left), round(top)), mask)
    draw.rounded_rectangle(
        [left, top, right, bottom],
        radius=radius * 1.6,
        outline=(0x0E, 0x0B, 0x09, 220),
        width=max(2, round(radius * 0.25)),
    )

    for index, (colour, position) in enumerate(zip(DRAWBAR_COLOURS, REGISTRATION)):
        centre = left + pitch * (index + 0.5)
        pulled = travel * position / 8.0
        stem_top = top - pulled
        # The slot the bar disappears into.
        draw.rounded_rectangle(
            [centre - bar * 0.44, top + radius, centre + bar * 0.44, bottom - radius],
            radius=radius * 0.7,
            fill=(0x0B, 0x09, 0x07, 255),
        )
        # Shadow, then the bar. It ends just inside the panel: what a player
        # reads is how much of the coloured stem is out, which is the whole
        # point of a drawbar.
        stem_bottom = top + radius * 1.4
        draw.rounded_rectangle(
            [
                centre - bar / 2 + radius * 0.3,
                stem_top + radius * 0.35,
                centre + bar / 2 + radius * 0.3,
                stem_bottom,
            ],
            radius=radius,
            fill=(0, 0, 0, 90),
        )
        draw.rounded_rectangle(
            [centre - bar / 2, stem_top, centre + bar / 2, stem_bottom],
            radius=radius,
            fill=colour + (255,),
            outline=blend(colour, EDGE, 0.55) + (255,),
            width=max(1, round(radius * 0.16)),
        )
        # The cap, which is the part a player actually holds.
        cap_height = travel * 0.30
        draw.rounded_rectangle(
            [
                centre - bar * 0.62,
                stem_top,
                centre + bar * 0.62,
                stem_top + cap_height,
            ],
            radius=radius * 0.8,
            fill=blend(colour, IVORY, 0.14) + (255,),
            outline=blend(colour, EDGE, 0.5) + (255,),
            width=max(1, round(radius * 0.2)),
        )
        draw.line(
            [
                (centre - bar * 0.34, stem_top + cap_height * 0.32),
                (centre + bar * 0.34, stem_top + cap_height * 0.32),
            ],
            fill=blend(colour, IVORY if colour is EDGE else EDGE, 0.55) + (170,),
            width=max(1, round(radius * 0.22)),
        )


def rotor(image, centre, radius, width):
    """A suggestion of the horn sweeping past, for the wider formats."""
    draw = ImageDraw.Draw(image, "RGBA")
    centre_x, centre_y = centre
    for index in range(5):
        spread = radius * (1.0 + index * 0.11)
        alpha = round(70 * (1.0 - index / 5.0))
        draw.arc(
            [
                centre_x - spread,
                centre_y - spread,
                centre_x + spread,
                centre_y + spread,
            ],
            start=205 + index * 4,
            end=335 - index * 4,
            fill=GOLD + (alpha,),
            width=width,
        )


def icon(path):
    size = (512, 512)
    image = background(size, (256, 150), 260)
    drawbars(image, (62, 288, 450, 452), travel=196, radius=12)
    draw = ImageDraw.Draw(image, "RGBA")
    draw.rounded_rectangle([24, 24, 488, 488], radius=64, outline=GOLD + (70,), width=5)
    image.save(path)
    return size


def banner(path):
    size = (1600, 400)
    image = background(size, (1180, 150), 420)
    rotor(image, (1210, 232), 150, 9)
    drawbars(image, (104, 236, 792, 350), travel=146, radius=10)
    draw = ImageDraw.Draw(image, "RGBA")
    draw.line([(0, 396), (1600, 396)], fill=GOLD + (120,), width=6)
    image.save(path)
    return size


def splash(path):
    size = (1920, 1080)
    image = background(size, (960, 470), 700)
    rotor(image, (960, 470), 330, 16)
    drawbars(image, (600, 664, 1320, 870), travel=272, radius=15)
    draw = ImageDraw.Draw(image, "RGBA")
    draw.line([(300, 900), (1620, 900)], fill=LINE + (110,), width=3)
    image.save(path)
    return size


def main():
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out = os.path.join(here, "package", "branding")
    os.makedirs(out, exist_ok=True)
    existing = [
        name
        for name in ("icon", "banner", "splash")
        if os.path.exists(os.path.join(out, f"{name}.png"))
    ]
    if existing and "--force" not in sys.argv:
        print(
            "refusing to overwrite the artwork in package/branding "
            f"({', '.join(existing)}); pass --force if the drawings are wanted back"
        )
        return
    for name, render in (("icon", icon), ("banner", banner), ("splash", splash)):
        path = os.path.join(out, f"{name}.png")
        width, height = render(path)
        print(f"{name}: {width}x{height} {os.path.getsize(path)} bytes -> {path}")


if __name__ == "__main__":
    main()
