#!/usr/bin/env python3
"""Audit two pinned native captures; this is not an owned-driver comparison."""
import hashlib
import itertools
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def capture(version, masks):
    pin = json.loads((ROOT / f'outputs/MouseCoupling-v{version}-provenance.json').read_text())
    for path, expected in pin['pins'].items():
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == expected, path
    assert pin['resource_rebuilt_byte_identical']
    data = json.loads((ROOT / f'outputs/MouseCouplingProbe-v{version}-reference.json').read_bytes())
    assert data['schema'] == 1 and data['probe'] == 'mouse-coupling-raw-v1'
    rows = data['records']
    expected = list(itertools.product(range(2), range(2), range(masks), range(4)))
    assert [(r['hide'], r['origin'], r['mask'], r['which']) for r in rows] == expected
    states = ('baseline', 'immediate', 'settled', 'after_system_task')
    for row in rows:
        assert set(row) == {'hide', 'origin', 'mask', 'which', 'requested',
                            'elapsed_ticks', *states}
        assert row['requested'] == [[40, 60], [200, 210], [1, 638], [478, 1]][row['which']]
        assert type(row['elapsed_ticks']) is int and row['elapsed_ticks'] >= 2
        for phase in states:
            state = row[phase]
            assert set(state) == {'mtemp', 'raw_mouse', 'mouse', 'flags_before_get',
                                  'flags_after_get', 'get_mouse', 'prefix', 'suffix'}
            for key in ('mtemp', 'raw_mouse', 'mouse', 'get_mouse'):
                assert len(state[key]) == 2
                assert all(type(v) is int and -32768 <= v <= 32767 for v in state[key])
            offset = (11, 7) if row['origin'] else (0, 0)
            assert state['get_mouse'] == [v + o for v, o in zip(state['mouse'], offset)]
            assert state['prefix'] == 0x11223344 and state['suffix'] == 0x55667788
            assert state['flags_before_get'] == state['flags_after_get']
            assert 0 <= state['flags_before_get'] <= 65535
    return rows


def main():
    old, expanded = capture(3, 10), capture(4, 18)
    assert old == [r for r in expanded if r['mask'] < 10]
    result = {
        'native_only': True,
        'earlier_records': len(old),
        'expanded_records': len(expanded),
        'earlier_records_reproduced_exactly': True,
        'expanded_get_mouse_origin_guard_and_flag_observations': len(expanded) * 4,
        'owned_driver_equivalence_claimed': False,
        'physical_motion_claimed': False,
    }
    (ROOT / 'outputs/MouseCoupling-native-audit.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
