#!/usr/bin/env python3
"""Rasterise a licensed system font to bounded, antialiased ASCII atlases.
Build-time only. No font parser or font file is required in the guest.
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import hashlib
import shutil

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src" / "generated"
OUT.mkdir(parents=True, exist_ok=True)
font = Path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
if not font.is_file():
    raise SystemExit("Install fonts-dejavu-core before building")
for size, label in [(16, "regular"), (24, "large")]:
    face = ImageFont.truetype(str(font), size)
    asc, desc = face.getmetrics()
    data = bytearray()
    advances = bytearray()
    for cp in range(32, 127):
        canvas = Image.new("L", (32, 32))
        ImageDraw.Draw(canvas).text((1, 1), chr(cp), font=face, fill=255)
        data.extend(canvas.tobytes())
        advances.append(max(3, min(31, round(face.getlength(chr(cp))))))
    (OUT / f"{label}.bin").write_bytes(data)
    (OUT / f"{label}-adv.bin").write_bytes(advances)
    assert len(data) == 95 * 1024 and len(advances) == 95
dist = ROOT / "dist"
dist.mkdir(exist_ok=True)
notice = Path("/usr/share/doc/fonts-dejavu-core/copyright")
if not notice.is_file():
    raise SystemExit("Missing font copyright notice")
shutil.copyfile(notice, dist / "FONT-NOTICE.txt")
with (dist / "FONT-NOTICE.txt").open("a") as out:
    out.write("\nHi5OS embeds raster glyph masks generated at build time from DejaVu Sans.\n")
    out.write("Input font SHA256: " + hashlib.sha256(font.read_bytes()).hexdigest() + "\n")
