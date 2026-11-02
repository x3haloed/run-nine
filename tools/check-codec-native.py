#!/usr/bin/env python3
"""Verify exact native CMPD/EXPT byte comparisons against the packaged corpus."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('observations', type=Path)
    parser.add_argument('report', type=Path)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    expected = {(row['batch'], row['case']): row for row in manifest['cases']}
    if not expected or len(expected) != len(manifest['cases']):
        raise ValueError('empty or duplicate manifest cases')
    rows = [json.loads(line) for line in args.observations.read_text().splitlines()]
    if not rows or rows[0].get('schema') != 1 or rows[0].get('event') != 'start':
        raise ValueError('missing native start record')
    if rows[0]['batches'] != manifest['batches']:
        raise ValueError('native/manifest batch counts differ')
    complete = rows[-1].get('event') == 'complete'
    seen, begun, batches, failures = set(), set(), set(), []
    for row in rows[1:]:
        event = row['event']
        if event == 'open':
            if row['error'] or not row['valid_ref']:
                failures.append({'open_error': row})
        elif event == 'batch':
            b = row['batch']
            if b in batches or row['cases'] != sum(key[0] == b for key in expected):
                raise ValueError('duplicate/incomplete native batch')
            batches.add(b)
        elif event in ('begin', 'result'):
            key = row['batch'], row['case']
            if key not in expected or row['batch'] not in batches:
                raise ValueError('unknown native case')
            if event == 'begin':
                if key in begun:
                    raise ValueError('duplicate begin')
                begun.add(key)
                continue
            if key not in begun or key in seen:
                raise ValueError('duplicate/unbegun result')
            seen.add(key)
            case = expected[key]
            if (row['native_error'] or row['expected_error'] or not row['equal']
                    or row['different_bytes'] != 0 or row['first_difference'] != -1
                    or row['actual_size'] != case['output_bytes']
                    or row['expected_size'] != case['output_bytes']):
                failures.append({'name': case['name'], 'native': row})
        elif event != 'complete':
            raise ValueError('unrecognized native event')
    missing = [expected[key]['name'] for key in expected.keys() - seen]
    report = {'complete': complete, 'cases': len(seen), 'expected_cases': len(expected),
              'missing': missing, 'failures': failures,
              'passed': complete and not missing and not failures and len(batches) == manifest['batches'],
              'manifest_sha256': hashlib.sha256(args.manifest.read_bytes()).hexdigest(),
              'observations_sha256': hashlib.sha256(args.observations.read_bytes()).hexdigest(),
              'scope': 'OS9 Resource Manager loaded owned compressed resources; native probe compared every byte against packaged owned expectations. No whole-OS or arbitrary-malformed-input claim.'}
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report[k] for k in ('complete', 'cases', 'expected_cases', 'passed')}))
    return not report['passed']


if __name__ == '__main__':
    raise SystemExit(main())
