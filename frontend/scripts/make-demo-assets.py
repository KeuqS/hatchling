"""Rebuild original demo media; requires reportlab, Pillow, numpy, and ffmpeg."""
from pathlib import Path
import math
import subprocess
import wave
import numpy as np
from PIL import Image, ImageDraw
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor

ROOT = Path(__file__).resolve().parents[1]
MEDIA = ROOT / 'public' / 'media'
MEDIA.mkdir(parents=True, exist_ok=True)
c = canvas.Canvas(str(MEDIA / 'sample-roadmap.pdf'), pagesize=(595, 842))
c.setTitle('Hatchling roadmap | Sample project ledger')
c.setAuthor('Hatchling scripted workspace')
c.setFillColor(HexColor('#ffffff')); c.rect(0, 0, 595, 842, fill=1, stroke=0)
c.setFillColor(HexColor('#556171')); c.setFont('Helvetica', 9); c.drawString(58, 785, 'HATCHLING  /  SAMPLE PROJECT LEDGER')
c.setFillColor(HexColor('#182331')); c.setFont('Helvetica-Bold', 34); c.drawString(58, 713, 'A considered roadmap.')
c.setFont('Helvetica', 12); c.setFillColor(HexColor('#556171')); c.drawString(58, 683, 'A reference-led frontend, built around explicit trust.')
c.setStrokeColor(HexColor('#d8dfe5')); c.line(58, 652, 536, 652)
sections = [
    ('01', 'Recorded baseline', ['The scripted adapter owns all product state.', 'The interface does not know how data is stored.', 'Sample data is visibly distinct from real activity.']),
    ('02', 'Definition of done', ['A result is complete only when evidence exists.', 'Every sensitive change asks for explicit approval.', 'Strict types and deterministic checks stay green.']),
    ('03', 'Next milestones', ['Core workspace and accessible navigation.', 'Confirmation rails, local artifacts and activity lanes.', 'A live adapter, supplied separately after review.'])
]
y = 602
for number, title, lines in sections:
    c.setFillColor(HexColor('#467586')); c.setFont('Helvetica', 11); c.drawString(58, y, number)
    c.setFillColor(HexColor('#182331')); c.setFont('Helvetica-Bold', 17); c.drawString(96, y, title)
    c.setFont('Helvetica', 11); c.setFillColor(HexColor('#556171'))
    for i, line in enumerate(lines): c.drawString(96, y - 30 - 22*i, line)
    y -= 143
c.setStrokeColor(HexColor('#d8dfe5')); c.line(58, 110, 536, 110)
c.setFont('Helvetica', 9); c.setFillColor(HexColor('#556171')); c.drawString(58, 83, 'Generic demonstration content. No real work or accounts are represented.'); c.drawRightString(536, 61, '01')
c.showPage(); c.save()
rate = 22050
x = np.arange(rate * 4) / rate
signal = sum(np.sin(2 * math.pi * f * x) * np.exp(-2.0 * x) for f in [261.63, 329.63, 392.0]) / 3
signal *= np.minimum(x * 20, 1) * 0.22
with wave.open(str(MEDIA / 'sample-audio.wav'), 'wb') as audio:
    audio.setnchannels(1); audio.setsampwidth(2); audio.setframerate(rate); audio.writeframes((signal * 32767).astype('<i2').tobytes())
# Original motion study using the reference-provided avatar, not fabricated talking-head footage.
frame = Image.new('RGB', (360, 640), '#e8ede9')
draw = ImageDraw.Draw(frame)
draw.rectangle((35, 50, 325, 470), fill='#f8faf8', outline='#bbc8be', width=2)
draw.rectangle((178, 52, 182, 468), fill='#cdd7cf')
draw.rectangle((37, 215, 323, 219), fill='#cdd7cf')
avatar = Image.open(MEDIA / 'avatar.png').convert('RGB').resize((200, 200))
frame.paste(avatar, (80, 315))
draw.text((34, 551), 'A LITTLE PAUSE', fill='#324639')
draw.text((34, 580), 'Original sample motion study', fill='#52635a')
poster = MEDIA / 'video-poster.png'; frame.save(poster)
subprocess.run(['ffmpeg', '-y', '-loglevel', 'error', '-loop', '1', '-i', str(poster), '-vf', "zoompan=z='min(zoom+0.00025,1.025)':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=100:s=360x640:fps=25", '-t', '4', '-c:v', 'libvpx-vp9', '-b:v', '150k', '-pix_fmt', 'yuv420p', str(MEDIA / 'sample-video.webm')], check=True)
print('Created PDF, original four-second audio, and four-second video assets.')
