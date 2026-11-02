#!/usr/bin/env python3
"""Bind the Marathon milestone's evidence; never inject input or infer UI success."""
import ast
import hashlib
import json
import struct
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from compare import read_probe, differences

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'outputs'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load(name):
    return json.loads((OUT / name).read_text())


def frame(stem):
    data = load(stem + '.json')
    ppm = (OUT / (stem + '.ppm')).read_bytes()
    assert hashlib.sha256(ppm).hexdigest() == data['ppm_sha256'], stem
    header = ppm.split(b'\n', 3)
    assert header[:3] == [b'P6', b'640 480', b'255'] and len(header[3]) == 640 * 480 * 3
    assert data['read_only_observation'] and data['active'], stem
    assert (OUT / (stem + '.png')).is_file()
    return data


def main():
    summary = load('Validation-summary.json')
    assert (summary['core_tests_passed'], summary['rust_tests_passed'], summary['frontend_tests_passed']) == (47, 77, 4)
    assert summary['component_checks_passed']
    tree = ast.parse((ROOT / 'tools/validate-runtime.py').read_text())
    cases = next(ast.literal_eval(node.value) for node in tree.body
                 if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'CASES' for t in node.targets))
    names = {probe: name for name, probe in cases}
    assert len(names) == len(summary['cases']) == 138
    comparisons = []
    for case in summary['cases']:
        name = names[case['probe']]
        ref, candidate = [OUT / f'{name}-{suffix}.json' for suffix in ('reference', 'runtime')]
        assert digest(ref) == case['reference_sha256'] and digest(candidate) == case['runtime_sha256'], name
        assert case['equal'] and not case['differences']
        assert not differences(read_probe(ref), read_probe(candidate)), name
        comparisons.append({'probe': case['probe'], 'reference': ref.name, 'runtime': candidate.name,
                            'reference_sha256': digest(ref), 'runtime_sha256': digest(candidate)})

    app = load('Marathon-provenance.json')
    cache = Path((ROOT / 'work/validation/dialogitemtext-local-inputs-path.txt').read_text().strip())
    for volume in (ROOT / 'work/volumes/marathon-source-v1', cache / 'source'):
        for asset in app['assets']:
            directory = volume / 'files' / asset['name'].lower().encode().hex()
            for fork in ('data', 'resource'):
                assert digest(directory / fork) == asset[fork + '_sha256']
                assert (directory / fork).stat().st_size == asset[fork + '_bytes']
    application = next(asset for asset in app['assets'] if asset['name'] == 'Marathon v1.2')
    for directory in (ROOT / 'work/apps/Marathon', cache):
        assert digest(directory / 'Marathon.rsrc') == application['resource_sha256']
        assert digest(directory / 'Marathon.data') == application['data_sha256']

    gameplay = load('Marathon-final-gameplay-result.json')
    assert digest(ROOT / gameplay['executable_path']) == gameplay['executable_sha256']
    assert gameplay['executable_identical_across_sessions']
    for episode in gameplay['pauses']:
        rows = [frame(stem) for stem in episode['frames']]
        assert len({r['ppm_sha256'] for r in rows}) == 1
        assert rows[-1]['ticks'] - rows[0]['ticks'] == episode['ticks_advanced'] > 0
        assert rows[-1]['elapsed_us'] - rows[0]['elapsed_us'] == episode['elapsed_us'] >= 60_000_000
        assert rows[-1]['execution_iteration'] > rows[0]['execution_iteration']
    assert frame('Marathon-combined-resumed')['ppm_sha256'] != gameplay['pauses'][0]['framebuffer_sha256']
    firing = load('Marathon-combined-fire-timeline.json')
    held = [r for r in firing if r['modifiers'] & 128 == 0]
    assert len(held) == 13 and held[-1]['elapsed_us'] - held[0]['elapsed_us'] >= 5_000_000
    assert all(r['active'] and r['key_map_hex'] == '0' * 32 for r in held)
    assert firing[-1]['modifiers'] & 128 and firing[-1]['key_map_hex'] == '0' * 32
    for stem in gameplay['sustained_primary_button']['selected_frames']:
        assert frame(stem)['modifiers'] & 128 == 0
    for number, code in ((289, 87), (293, 91)):
        row = frame(f'Marathon-exact-build-walk-{number:03}')
        bits = bytes.fromhex(row['key_map_hex'])
        assert [n for n in range(128) if bits[n // 8] & (1 << (n % 8))] == [code]
    for stem in ('Marathon-exact-build-walk-paused', 'Marathon-exact-build-right-paused', 'Marathon-exact-build-left-paused'):
        row = frame(stem)
        assert row['key_map_hex'] == '0' * 32 and row['modifiers'] & 128
    turns = [frame('Marathon-exact-build-' + direction + '-paused') for direction in ('right', 'left')]
    assert [r['cursor_warps'] for r in turns] == [2, 3]
    assert turns[0]['ppm_sha256'] != turns[1]['ppm_sha256']
    assert all(r['host_mouse'] == [240, 320] for r in turns)

    teach = summary['teachtext_roundtrip']
    for key in ('matches_reference_binary', 'saved_bytes_equal_os9_reference', 'same_process_save_close_open',
                'fresh_process_reopen', 'cancel_preserves_text_and_creates_no_file'):
        assert teach[key], key
    saved = Path(teach['volume']) / 'files' / b'reference.txt'.hex() / 'data'
    assert saved.read_bytes() == (OUT / 'TeachText-reference-document.txt').read_bytes()
    for key in ('move', 'resize', 'scroll', 'thumb'):
        row = summary['teachtext_' + key]
        assert row['unmodified_application'] and row['returned_to_bounded_event_loop']
    clipboard = summary['teachtext_clipboard']
    assert clipboard['unmodified_reference_binary'] and len(clipboard['cases']) == 3
    for case in clipboard['cases']:
        assert hashlib.sha256(bytes(case['text_bytes'])).hexdigest() == case['text_sha256']
        assert digest(ROOT / case['script']) == case['script_sha256']
    replacements = summary['teachtext_replacement']['cases']
    assert {r['case'] for r in replacements} == {'new-after-close', 'cancel', 'replacement-accept', 'save-existing'}
    for row in replacements:
        for key, value in row.items():
            if type(value) is bool:
                assert value, (row['case'], key)
    simple = summary['simpletext_save']
    for key in ('unmodified_application', 'saved_data_matches_native', 'saved_style_resources_match_native_metadata_and_payload',
                'finder_type_creator_match_native', 'document_memory_preserved'):
        assert simple[key], key
    assert digest(OUT / 'SimpleText-saved-document.data') == simple['data_sha256']
    assert digest(OUT / 'SimpleText-saved-document.rsrc') == simple['resource_sha256']
    reopen = summary['simpletext_reopen']
    assert reopen['same_process_save_close_reopen']['text_and_style_restored']
    assert reopen['fresh_process_reopen']['text_and_style_restored'] and reopen['data_matches_native']
    cancel = summary['simpletext_cancel']
    assert cancel['cancel_returned_to_document'] and cancel['no_file_created']

    boot = summary['native_boot']
    assert boot['booted'] and boot['exception_level'] == 1 and not boot['mmu_enabled']
    assert boot['owned_68k_probe_matches_os9'] and boot['shared_core_probe_matches_os9']
    assert digest(OUT / 'Native-core-kernel.elf') == boot['kernel_sha256']
    elf = (OUT / 'Native-core-kernel.elf').read_bytes()
    assert elf[:6] == b'\x7fELF\x02\x01' and struct.unpack_from('<H', elf, 18)[0] == 183
    phoff, = struct.unpack_from('<Q', elf, 32)
    phsize, count = struct.unpack_from('<HH', elf, 54)
    assert all(struct.unpack_from('<I', elf, phoff + i * phsize)[0] != 3 for i in range(count))
    program = ROOT / 'probes/owned68k/program.bin'
    assert len(program.read_bytes()) == 166 and digest(program) == boot['owned_program_sha256']
    serial = (OUT / 'Native-boot-serial.txt').read_text()
    assert 'OWNED68K instructions=195 traps=10' in serial and 'CLASSIC-NATIVE-PASS' in serial
    subprocess.run(['git', 'diff', '--exit-code', gameplay['implementation_commit'], '--',
                    'runtime', 'compat-core', 'vendor/m68k', 'native', 'probes',
                    ':(exclude)runtime/THIRD_PARTY.txt',
                    ':(exclude)runtime/vendor/minifb/LOCAL_CHANGES.txt'],
                   cwd=ROOT, check=True, stdout=subprocess.PIPE)

    evidence = [
        'Marathon-final-gameplay-result.json', 'MouseDriver-validation-summary.json', 'Validation-summary.json',
        'Marathon-provenance.json', 'Marathon-reference-provenance.json', 'Marathon-assets-result.json',
        'Marathon-menu-runtime.png', 'Marathon-exact-build-command.json', 'Marathon-combined-fire-timeline.json',
        'Marathon-exact-build-input-timeline.json', 'Marathon-combined-first-trace.txt', 'Marathon-exact-build-trace.txt',
        'MouseCoupling-v5-provenance.json', 'Native-boot-result.json', 'Native-boot-serial.txt',
        'CPU-port-provenance.json', 'Owned68K-provenance.json', 'OS9-setup.txt', 'Second-pass-completion-audit.txt',
        'Marathon-completion-audit.txt', 'Marathon-playable-handoff.txt',
    ]
    selected_frames = gameplay['sustained_primary_button']['selected_frames']
    selected_frames += gameplay['walking']['selected_frames']
    selected_frames += ['Marathon-combined-resumed', 'Marathon-exact-build-walk-paused',
                        'Marathon-exact-build-right-paused', 'Marathon-exact-build-left-paused',
                        'Marathon-exact-build-final-paused']
    selected_frames += [stem for episode in gameplay['pauses'] for stem in episode['frames']]
    evidence += sorted({stem + '.' + extension for stem in selected_frames for extension in ('json', 'ppm', 'png')})
    source_evidence = ['compat-core/src/catalog.rs', 'runtime/src/files.rs', 'compat-core/src/mouse.rs',
                       'runtime/src/frontend.rs', 'compat-core/src/execution.rs', 'native/src/main.rs',
                       'runtime/THIRD_PARTY.txt', 'runtime/vendor/minifb/LOCAL_CHANGES.txt',
                       'native/THIRD_PARTY.txt', 'runtime/Cargo.lock', 'native/Cargo.lock',
                       'tools/audit-marathon-completion.py']
    result = {
        'schema': 1, 'audited_at_utc': datetime.now(timezone.utc).isoformat(),
        'implementation_commit': gameplay['implementation_commit'], 'all_required_proven': True,
        'native_comparisons_verified': len(comparisons), 'comparisons': comparisons,
        'evidence_sha256': {name: digest(OUT / name) for name in evidence},
        'source_review_sha256': {name: digest(ROOT / name) for name in source_evidence},
        'executable_sha256': gameplay['executable_sha256'],
        'manual_review': 'Reviewed actual original live scene changes and selected stored RGB frames: living forward/backward movement, right/left headings, muzzle flash/hit/reload, healthy paused states. Reviewed generic catalog/volume resolution and cursor request paths; the Marathon Preferences string in Files.catalog_probe is an owned missing-file input, not production routing. Reviewed actual cpu.step/shared-trap execution in the freestanding kernel and document verifier checks of original CODE execution and persisted bytes.',
        'requirements': [
            {'requirement': 'Unchanged original application and sibling assets', 'status': 'proven', 'evidence': ['Marathon-provenance.json', 'Marathon-final-gameplay-result.json']},
            {'requirement': 'Classic directories,relative/absolute paths and forks sufficient for assets', 'status': 'proven', 'evidence': ['catalog-semantics-v1', 'fcb-semantics-v1', 'path-files-pb-semantics-v1', 'Marathon-assets-result.json'], 'scope': 'Generic catalog resolution; no game-specific routing. Complete HFS/OS9 semantics are not claimed.'},
            {'requirement': 'Original title/menu,New Game and first-level color rendering', 'status': 'proven', 'evidence': ['Marathon-menu-runtime.png', 'Marathon-exact-build-walk-286.png', 'Marathon-combined-fire-24.png', 'indexed-color-v1', 'pict-bitmap-v1'], 'scope': 'Actual player-controlled Arrival slice; original campaign completion and whole-frame native pixel equality are not claimed.'},
            {'requirement': 'Sustained keyboard/mouse walking,turning and firing', 'status': 'proven', 'evidence': ['Marathon-final-gameplay-result.json', 'Marathon-mouse-warp-result.json'], 'scope': 'Exact executable forward/backward/turn/fire checks plus prior ten-second walking and repeated recentering on unchanged production source. No native physical acceleration equivalence.'},
            {'requirement': 'Pause,resume and timing stable enough for actual play', 'status': 'proven', 'evidence': ['Marathon-final-gameplay-result.json', 'time-manager-semantics-v1', 'delay-v1'], 'scope': '60s and90s pauses with unchanged images and advancing ticks,successful resumes and subsequent play. Full native game frame-time parity is not claimed.'},
            {'requirement': 'Native observations,shared abstractions,real traces and meaningful comparisons', 'status': 'proven', 'evidence': ['MouseCoupling-v5-provenance.json', 'OS9-setup.txt', 'Validation-summary.json'], 'scope': '138 selected semantic profiles; explicit raw timing/private-state exclusions and unsupported contexts remain documented.'},
            {'requirement': 'Preserve TeachText/SimpleText document,clipboard,save/reopen,replacement/recovery and interaction workflows', 'status': 'proven', 'evidence': ['Validation-summary.json', 'Second-pass-completion-audit.txt'], 'scope': 'Current production execution regression checks plus retained actual native/hosted UI evidence. Bounded event-loop instruction limits are deliberate stops,not application-exit claims.'},
            {'requirement': 'Owned68K program through shared services in freestanding ARM64 QEMU', 'status': 'proven', 'evidence': ['Native-boot-result.json', 'Native-boot-serial.txt', 'Owned68K-provenance.json'], 'scope': '166-byte owned program,195 instructions,10 shared traps,EL1,real interpreter and no dynamic loader. No original app,GUI,storage,interrupt or physical Pi execution in the kernel.'},
            {'requirement': 'Pins,dependency provenance,reproduction,checkpoints and complete audit', 'status': 'proven', 'evidence': evidence, 'scope': 'Current artifact hashes and source review bind this audit. Earlier per-checkpoint goal_complete=false fields are historical stage records.'},
        ],
        'authorized_deferrals': ['Pi500 physical boot,drivers and hardware validation until hardware is owned'],
        'optional_or_outside_scope': ['Audio remains unverified and is not a completion gate', 'Full campaign and complete OS9 equivalence'],
        'known_limits': ['Approximate HUD glyphs,top white strip and selected visual differences',
                         'Preferences are not persisted in the current writable catalog;some control titles disappear on redraw',
                         'GetGWorld and broader offscreen APIs remain unsupported on other reached paths',
                         'Post-death reincarnation/restart controls unverified;excluded dead/attract trials are not evidence',
                         'Host cursor synchronization implemented on macOS;physical Pi drivers and other-host warping deferred'],
    }
    (OUT / 'Marathon-completion-audit.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'all_required_proven': True, 'native_comparisons_verified': len(comparisons),
                      'tests': [47, 77, 4], 'executable_sha256': gameplay['executable_sha256']}))


if __name__ == '__main__':
    main()
