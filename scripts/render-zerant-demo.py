#!/usr/bin/env python3
"""Encode genuine browser captures with explicit streams and validate delivery.
No subtitle filters, caption overlays, or subtitle streams are accepted.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
from PIL import Image


def run(args):
    subprocess.run(args, check=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    parser.add_argument('audio', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--ffmpeg', default=os.environ.get('DEMO_FFMPEG', 'ffmpeg'))
    parser.add_argument('--ffprobe', default='ffprobe')
    args = parser.parse_args()
    capture = args.capture.resolve()
    metadata = json.loads((capture / 'capture.json').read_text())
    duration = metadata['duration']
    if duration <= 0 or not args.audio.is_file():
        raise ValueError('Missing narration or invalid capture duration')
    if not list(capture.glob('*.jpg')):
        raise ValueError('No captured browser frames')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # Chromium can change JPEG dimensions (including screencast downscaling).
    # A changing decoder size reinitializes fps and can silently drop scenes.
    # Normalize *before* concat; the filter graph always receives 1920x1080 RGB.
    normalized = capture / 'normalized'
    normalized.mkdir(exist_ok=True)
    for source in sorted(capture.glob('*.jpg')):
        with Image.open(source) as image:
            image = image.convert('RGB')
            image.thumbnail((1920, 1080), Image.Resampling.LANCZOS)
            if image.width > 600 and image.size != (1920, 1080):
                scale = min(1920 / image.width, 1080 / image.height)
                image = image.resize((round(image.width * scale), round(image.height * scale)), Image.Resampling.LANCZOS)
            canvas = Image.new('RGB', (1920, 1080), '#f7f7f2')
            canvas.paste(image, ((1920 - image.width) // 2, (1080 - image.height) // 2))
            canvas.save(normalized / source.name, quality=95, subsampling=0)
    (normalized / 'frames.ffconcat').write_text((capture / 'frames.ffconcat').read_text())
    temporary = args.output.with_name(args.output.stem + '.partial.mp4')
    # x264 Main level 4.0 / yuv420p / 1080p30 for software and hardware players.
    # The concat timeline records actual compositor timestamps; fps produces CFR.
    run([args.ffmpeg, '-y', '-hide_banner', '-loglevel', 'warning',
         '-f', 'concat', '-safe', '0', '-i', str(normalized / 'frames.ffconcat'),
         '-i', str(args.audio.resolve()), '-map', '0:v:0', '-map', '1:a:0',
         '-vf', 'scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=0xf7f7f2,fps=30,setsar=1',
         '-af', 'apad,alimiter=limit=0.95', '-t', str(duration),
         '-c:v', 'libx264', '-preset', 'medium', '-crf', '20',
         '-profile:v', 'main', '-level:v', '4.0', '-pix_fmt', 'yuv420p',
         '-c:a', 'aac', '-b:a', '192k', '-ar', '48000', '-sn', '-dn',
         '-movflags', '+faststart', str(temporary.resolve())])
    probe = json.loads(subprocess.check_output([args.ffprobe, '-v', 'error',
        '-show_streams', '-show_format', '-of', 'json', str(temporary)]))
    video = next(s for s in probe['streams'] if s['codec_type'] == 'video')
    audio = next(s for s in probe['streams'] if s['codec_type'] == 'audio')
    assert (video['codec_name'], video['pix_fmt'], video['width'], video['height'], video['r_frame_rate']) == ('h264', 'yuv420p', 1920, 1080, '30/1')
    assert audio['codec_name'] == 'aac'
    assert len(probe['streams']) == 2, 'Unexpected streams (captions/data)'
    assert abs(float(probe['format']['duration']) - duration) < 0.1
    # Decode every frame, not only the header. Visual inspection is still required.
    run([args.ffmpeg, '-v', 'error', '-xerror', '-i', str(temporary), '-f', 'null', '-'])
    temporary.replace(args.output)
    (args.output.parent / (args.output.stem + '-probe.json')).write_text(json.dumps(probe, indent=2))
    print(f'Encoded and decoded {duration:.3f}s; inspect extracted frames before acceptance.')


if __name__ == '__main__':
    main()
