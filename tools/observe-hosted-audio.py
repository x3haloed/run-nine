#!/usr/bin/env python3
"""Finite read-only SIGUSR1 observations; never send guest input.

Persist each metadata row immediately, including on normal process exit.
Only save frame images while a guest key is held, up to the requested limit.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import time


def observe(pid, capture, output, seconds, image_limit):
    output.mkdir(parents=True, exist_ok=False)
    meta = Path(str(capture) + '.json')
    previous = meta.read_text() if meta.exists() else None
    deadline = time.monotonic() + seconds
    count = saved = 0
    terminal = False
    try:
        with (output / 'observations.jsonl').open('x') as log:
            while time.monotonic() < deadline:
                os.kill(pid, 0)
                os.kill(pid, signal.SIGUSR1)
                acknowledgement = time.monotonic() + 20
                while True:
                    os.kill(pid, 0)
                    try:
                        text = meta.read_text()
                        value = json.loads(text)
                        if text != previous and value.get('read_only_observation'):
                            break
                    except (FileNotFoundError, json.JSONDecodeError):
                        pass
                    if time.monotonic() >= acknowledgement:
                        raise TimeoutError('Live process has not acknowledged snapshot; not restarted')
                    time.sleep(.01)
                previous = text
                if value['key_map_hex'] != '0' * 32 and saved < image_limit:
                    data = capture.read_bytes()
                    name = f'held-{saved:03d}.ppm'
                    (output / name).write_bytes(data)
                    value['ppm_file'] = name
                    value['ppm_sha256'] = hashlib.sha256(data).hexdigest()
                    saved += 1
                log.write(json.dumps(value) + '\n')
                log.flush()
                count += 1
                time.sleep(.03)
    except ProcessLookupError:
        terminal = True
    finally:
        (output / 'result.json').write_text(json.dumps(dict(
            read_only=True, observations=count, held_images=saved,
            process_exited_during_observation=terminal), indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--seconds', type=float, default=20)
    parser.add_argument('--images', type=int, default=24)
    args = parser.parse_args()
    assert 0 < args.seconds <= 60 and 0 <= args.images <= 60
    observe(args.pid, args.capture, args.output, args.seconds, args.images)
