#!/usr/bin/env python3
"""Join completed genuine capture sections without synthesizing interactions.
Copies source frames; preserves their recorded durations. Inputs stay intact.
"""
import argparse
import json
from pathlib import Path
import shlex
import shutil

p = argparse.ArgumentParser()
p.add_argument('captures', nargs='+', type=Path)
p.add_argument('--ends', nargs='+', type=float, help='Optional completed-scene end time for each input')
p.add_argument('--output', required=True, type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
if any(a.output.iterdir()):
    raise ValueError('Use an empty output directory; original takes are preserved')
lines = ['ffconcat version 1.0']
scenes = []
total = 0
count = 0
if a.ends and len(a.ends) != len(a.captures):
    raise ValueError('Supply one end time for each capture')
for input_index, source in enumerate(a.captures):
    base_count = count
    meta = json.loads((source / 'capture.json').read_text())
    limit = a.ends[input_index] if a.ends else meta['duration']
    boundaries = [s['start']+s['duration'] for s in meta['scenes']]
    if not any(abs(limit-b)<.001 for b in boundaries):
        raise ValueError('Trim only at a completed scene boundary')
    entries = (source / 'frames.ffconcat').read_text().splitlines()
    section_duration = 0
    for i, line in enumerate(entries):
        if not line.startswith('file ') or i+1>=len(entries) or not entries[i+1].startswith('duration '):
            continue  # Skip the final duplicated flush frame, not recorded content.
        file = shlex.split(line)[1]
        duration = min(float(entries[i+1].split()[1]), limit-section_duration)
        if limit-section_duration < .000001:
            break
        if duration <= 0:
            raise ValueError('Non-positive frame duration')
        name = f'{count:06}.jpg'
        shutil.copyfile(source / file, a.output / name)
        lines.extend([f"file '{name}'", f'duration {duration:.6f}'])
        section_duration += duration
        count += 1
    if abs(section_duration - limit)>.001:
        raise ValueError('Capture timeline does not match its metadata')
    for scene in meta['scenes']:
        if scene['start']+scene['duration'] > limit+.001:
            continue
        scenes.append({**scene, 'start': scene['start'] + total, 'frames': scene['frames'] + base_count})
    total += limit
lines.append(f"file '{count-1:06}.jpg'")
(a.output / 'frames.ffconcat').write_text('\n'.join(lines)+'\n')
(a.output / 'capture.json').write_text(json.dumps({'duration':total,'frames':count,'cursor':'12x18 pixels; no click effects','scenes':scenes},indent=2))
print('Joined real recorded sections:',total,'seconds')
