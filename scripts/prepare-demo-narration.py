#!/usr/bin/env python3
"""Synthesize scene narration; keep 125–140 wpm and explicit reading pauses.
Requires edge-tts CLI on PATH. Saves no subtitles and never alters product data.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

p = argparse.ArgumentParser()
p.add_argument('script', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--tts', default='edge-tts')
a = p.parse_args()
spec = json.loads(a.script.read_text())
a.output.mkdir(parents=True, exist_ok=True)
segments = []
report = []
for i, scene in enumerate(spec['scenes']):
    raw = a.output / f'{i:02d}-{scene["key"]}.mp3'
    if not raw.exists() or raw.stat().st_size == 0:
        subprocess.run([a.tts, '--voice', spec['voice'], '--rate='+spec['rate'], '--text', scene['text'], '--write-media', str(raw)], check=True)
    secs = float(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',str(raw)]))
    words = len(re.findall(r"\b[\w'-]+\b", scene['text']))
    # Aim at 125 wpm without speeding the spoken words to fit an oversized script.
    speaking = words / 125 * 60
    if speaking + 0.7 > scene['duration']:
        raise ValueError(f'{scene["key"]}: shorten script; not enough time for comfortable narration')
    tempo = secs / speaking
    segment = a.output / f'{i:02d}-{scene["key"]}.wav'
    subprocess.run(['ffmpeg','-v','error','-y','-i',str(raw),'-af',f'atempo={tempo:.6f},adelay=350,apad','-t',str(scene['duration']),'-ar','48000','-ac','1','-c:a','pcm_s16le',str(segment)], check=True)
    segments.append(segment.resolve())
    report.append({'scene':scene['key'],'duration':scene['duration'],'words':words,'speaking_seconds':round(speaking,3),'wpm':125,'pause_seconds':round(scene['duration']-speaking,3)})
manifest = a.output / 'audio.ffconcat'
manifest.write_text('ffconcat version 1.0\n'+''.join(f"file '{s}'\n" for s in segments))
subprocess.run(['ffmpeg','-v','error','-y','-f','concat','-safe','0','-i',str(manifest),'-af','loudnorm=I=-16:TP=-1.5:LRA=11','-c:a','pcm_s16le','-ar','48000',str(a.output/'narration.wav')], check=True)
(a.output/'narration-report.json').write_text(json.dumps(report,indent=2))
print('Narration prepared:',sum(s['duration'] for s in spec['scenes']),'seconds')
