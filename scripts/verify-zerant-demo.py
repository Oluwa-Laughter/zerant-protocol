#!/usr/bin/env python3
"""Decode all frames, scan black/freeze intervals, and extract required QA images.
Freeze intervals are reported for visual review; deliberate reading pauses are
not evidence of a broken recorder. A successful probe never replaces inspection.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

p = argparse.ArgumentParser()
p.add_argument('video', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
probe = json.loads(subprocess.check_output(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(a.video)]))
video = next(s for s in probe['streams'] if s['codec_type'] == 'video')
assert video['codec_name'] == 'h264' and video['pix_fmt'] == 'yuv420p'
assert (video['width'],video['height'],video['r_frame_rate']) == (1920,1080,'30/1')
assert [s['codec_type'] for s in probe['streams']] == ['video','audio'], 'Unexpected subtitle/data streams'
assert next(s for s in probe['streams'] if s['codec_type']=='audio')['codec_name']=='aac'
assert abs(float(probe['format']['duration'])-200) < .1, 'Final video must be 200 seconds'
scan = subprocess.run(['ffmpeg','-hide_banner','-xerror','-i',str(a.video),'-vf','blackdetect=d=0.04:pix_th=0.10,freezedetect=n=-60dB:d=2','-af','volumedetect','-f','null','-'],capture_output=True,text=True,check=True)
(a.output/'full-frame-scan.log').write_text(scan.stderr)
black = re.findall(r'black_start:[^\r\n]+',scan.stderr)
freeze = re.findall(r'lavfi.freezedetect.[^\r\n]+',scan.stderr)
assert not black, f'Black frames detected: {black}'
for seconds in [5,30,60,90,120,150,180,195]:
    subprocess.run(['ffmpeg','-v','error','-y','-ss',str(seconds),'-i',str(a.video),'-frames:v','1','-update','1',str(a.output/f'final-{seconds:03d}.png')],check=True)
report = {'duration':probe['format']['duration'],'resolution':'1920x1080','fps':'30/1','video':'H.264 Main/yuv420p','audio':'AAC','subtitle_streams':0,'black_intervals':black,'decode_errors':0,'freeze_review':freeze,'audio_levels':re.findall(r'(?:mean|max)_volume:[^\r\n]+',scan.stderr)}
(a.output/'verification.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))
print('Inspect all eight PNGs and the freeze intervals before delivery.')
