#!/usr/bin/env python3
"""Compare independently authored TSV vectors against separate decoder programs.

Reports differences; never changes expected results to match an implementation.
Reference processes have explicit per-input deadlines and bounded declared sizes.
"""
import argparse
import concurrent.futures
import csv
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess


def execute(executable, request):
    process = subprocess.Popen([str(executable)], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(json.dumps(request) + '\n', timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        return {'status': 'deadline'}
    if process.returncode:
        return {'status': 'process_error', 'exit': process.returncode, 'stderr': stderr[:1000]}
    result = json.loads(stdout)
    if result['status'] == 'expanded':
        data = bytes.fromhex(result.pop('hex'))
        result.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('vectors', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('old', type=Path)
    parser.add_argument('reference', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    csv.field_size_limit(16 * 1024 * 1024)
    with args.vectors.open() as f:
        vectors = list(csv.DictReader(f, delimiter='\t'))

    def compare(row):
        request = {'attrs': int(row['attrs']), 'hex': row['input_hex'], 'limit': int(row['limit'])}
        expected = ({'status': 'expanded', 'bytes': len(row['output_hex']) // 2,
                     'sha256': hashlib.sha256(bytes.fromhex(row['output_hex'])).hexdigest()}
                    if row['outcome'] == 'decoded' else
                    {'status': 'passthrough'} if row['outcome'] == 'pass' else {'status': 'error'})
        results = {'candidate': execute(args.candidate.resolve(), request)}
        data = bytes.fromhex(row['input_hex'])
        declared = int.from_bytes(data[8:12], 'big') if len(data) >= 18 else 0
        for name, exe in [('old', args.old), ('reference', args.reference)]:
            results[name] = (execute(exe.resolve(), request) if declared <= 8 * 1024 * 1024
                             else {'status': 'skipped', 'reason': 'reference allocation guard'})
        def equivalent(result):
            return all(result.get(k) == value for k, value in expected.items())
        return {'name': row['name'], 'expected_category': row['outcome'],
                'input_sha256': hashlib.sha256(data).hexdigest(), 'expected': expected,
                'results': results, 'matches_expected': {k: equivalent(v) for k, v in results.items()}}

    differences = []
    candidate_failures = []
    with (output / 'comparisons.jsonl').open('w') as log, concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        for i, result in enumerate(pool.map(compare, vectors), 1):
            log.write(json.dumps(result) + '\n')
            log.flush()
            if not result['matches_expected']['candidate']:
                candidate_failures.append(result['name'])
            if not all(result['matches_expected'].values()):
                differences.append(result)
            if i % 500 == 0:
                print('Compared', i, '/', len(vectors), flush=True)
    report = {'cases': len(vectors), 'candidate_failures': candidate_failures,
              'differences_requiring_investigation': differences,
              'note': 'Error-category assertions and native policy decisions are separate from this byte/status comparison.'}
    (output / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    print('Completed', len(vectors), 'cases;', len(differences), 'differences requiring investigation', flush=True)
    return bool(candidate_failures)


if __name__ == '__main__':
    raise SystemExit(main())
