"""Prepare the supplied Hatchling artwork and a silent four-second preview.
Usage: python3 scripts/prepare-mascot.py /path/to/hatchling.png
Requires Pillow and ffmpeg. Does not generate or substitute a character.
"""
import math
import subprocess
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'public' / 'media'


def main(source):
    mascot = Image.open(source).convert('RGBA')
    # Retain the complete original at full resolution for the library/lightbox.
    mascot.save(DEST / 'hatchling-mascot.png', optimize=True)
    for name, size in [('hatchling-avatar.png', 200), ('avatar.png', 200), ('rail-avatar.png', 108), ('identity-badge.png', 72)]:
        thumb = Image.new('RGBA', (size, size), (255, 255, 255, 255))
        art = mascot.resize((int(size * .94), int(size * .94)), Image.Resampling.LANCZOS)
        thumb.alpha_composite(art, ((size - art.width) // 2, (size - art.height) // 2))
        thumb.save(DEST / name, optimize=True)
    width, height, fps = 360, 640, 24
    background = Image.new('RGBA', (width, height), '#f2efe6')
    draw = ImageDraw.Draw(background)
    draw.rounded_rectangle((20, 30, 340, 610), radius=32, fill='#faf8f1', outline='#e3dece', width=1)
    shadow = Image.new('RGBA', background.size)
    ImageDraw.Draw(shadow).ellipse((80, 470, 280, 500), fill=(82, 60, 25, 35))
    background.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(13)))
    process = subprocess.Popen(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'rawvideo', '-pixel_format', 'rgb24', '-video_size', f'{width}x{height}', '-framerate', str(fps), '-i', '-', '-an', '-c:v', 'libvpx-vp9', '-crf', '34', '-b:v', '0', '-pix_fmt', 'yuv420p', str(DEST / 'sample-video.webm')], stdin=subprocess.PIPE)
    try:
        for frame in range(4 * fps):
            phase = frame / (4 * fps) * 2 * math.pi
            size = round(322 + 3 * math.sin(phase))
            art = mascot.resize((size, size), Image.Resampling.LANCZOS)
            canvas = background.copy()
            canvas.alpha_composite(art, ((width - size) // 2, round(167 + 4 * math.sin(phase))))
            rgb = canvas.convert('RGB')
            if frame == 0:
                rgb.save(DEST / 'video-poster.png', optimize=True)
            process.stdin.write(rgb.tobytes())
    finally:
        process.stdin.close()
    if process.wait() != 0:
        raise RuntimeError('ffmpeg did not finish the mascot video')


if __name__ == '__main__':
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_file():
        raise SystemExit('Provide the path to the supplied Hatchling PNG.')
    main(Path(sys.argv[1]))
