#!/usr/bin/env python3
"""Regression for the dimension-switch bug; colored technical fixtures only.
No product screenshots, credentials, sessions, or demo data are fabricated.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
from PIL import Image

encoder = os.environ.get('DEMO_FFMPEG', 'ffmpeg')
with tempfile.TemporaryDirectory(prefix='zerant-render-test-') as temporary:
    root = Path(temporary)
    capture = root / 'capture'
    capture.mkdir()
    colors = [(200, 10, 10), (10, 200, 10), (10, 10, 200)]
    sizes = [(1920, 1080), (390, 1080), (1620, 912)]
    for i, (color, size) in enumerate(zip(colors, sizes)):
        Image.new('RGB', size, color).save(capture / f'{i:06}.jpg')
    (capture / 'capture.json').write_text(json.dumps({'duration': 3}))
    (capture / 'frames.ffconcat').write_text("ffconcat version 1.0\nfile '000000.jpg'\nduration 1\nfile '000001.jpg'\nduration 1\nfile '000002.jpg'\nduration 1\nfile '000002.jpg'\n")
    audio = root / 'audio.wav'
    subprocess.run([encoder,'-v','error','-f','lavfi','-i','sine=frequency=440:duration=3','-y',str(audio)],check=True)
    video = root / 'test.mp4'
    subprocess.run(['python3',str(Path(__file__).with_name('render-zerant-demo.py')),str(capture),str(audio),str(video),'--ffmpeg',encoder],check=True)
    assert not (root / 'test.partial.mp4').exists(), 'Partial export leaked'
    for i, expected in enumerate(colors):
        pixel = subprocess.check_output([encoder,'-v','error','-ss',str(i+.5),'-i',str(video),'-vf','crop=2:2:960:540','-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'])[:3]
        assert len(pixel)==3 and all(abs(value-target)<8 for value,target in zip(pixel,expected)), f'Section {i} lost: expected {expected}, got {list(pixel)}'
    print('PASS: all three sections survive changing source dimensions; 1080p30 H.264/AAC decodes; atomic publication works.')
