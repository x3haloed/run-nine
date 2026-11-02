#!/usr/bin/env python3
"""Compare the shared cursor-device service and real GetMouse dispatch with OS 9."""
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from compare import differences, read_probe
from mouse_observations import project, PHASES

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'outputs'


def leaves(value, path=()):
    if isinstance(value, dict):
        for key, child in value.items():
            yield from leaves(child, path + (key,))
    elif isinstance(value, list):
        for key, child in enumerate(value):
            yield from leaves(child, path + (key,))
    else:
        yield path, value


def main():
    pin = json.loads((OUT / 'MouseCoupling-v5-provenance.json').read_text())
    for path, expected in pin['pins'].items():
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == expected, path
    assert pin['resource_rebuilt_byte_identical']
    native = project(json.loads((OUT / 'MouseCouplingProbe-v5-reference.json').read_bytes()))
    raw = subprocess.check_output(['cargo', 'run', '--quiet', '--manifest-path', str(ROOT / 'runtime/Cargo.toml'),
                                   '--', '--probe', 'mouse-coupling-runtime-raw-v1'], cwd=ROOT, text=True)
    (OUT / 'MouseCouplingProbe-runtime-raw.json').write_text(raw)
    runtime = project(json.loads(raw))
    for label, value in [('reference', native), ('runtime', runtime)]:
        path = OUT / f'MouseCouplingProbe-{label}.json'
        path.write_text(json.dumps(value, indent=2) + '\n')
        assert read_probe(path) == value
    assert not differences(native, runtime), differences(native, runtime)[:10]
    first = native['mouse']['records'][0]
    paths = [('mouse', 'records')]
    paths += [('mouse', 'records', 0, key) for key in first]
    paths += [('mouse', 'records', 0, phase, key) for phase in PHASES for key in first[phase]]
    missing = mutations = 0
    scratch = ROOT / 'work/validation/mouse-incomplete.json'
    for path in paths:
        bad = copy.deepcopy(native)
        target = bad
        for key in path[:-1]:
            target = target[key]
        del target[path[-1]]
        scratch.write_text(json.dumps(bad))
        try:
            read_probe(scratch)
        except ValueError:
            missing += 1
        else:
            raise AssertionError('Accepted missing mouse field ' + str(path))
    for index, row in enumerate(runtime['mouse']['records']):
        for path, old in list(leaves(row)):
            target = row
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = old ^ 1
            assert differences(native['mouse']['records'][index], row), path
            target[path[-1]] = old
            mutations += 1
    print(json.dumps({'records': 288, 'phase_observations': 1728, 'compared_fields_exact': True,
                      'missing_fields_rejected': missing, 'mutations_detected': mutations,
                      'raw_only': ['native elapsed_ticks', 'immediate.flags_after_get'],
                      'physical_motion_native_equivalence_claimed': False}))


if __name__ == '__main__':
    main()
