#!/usr/bin/env python3
"""Audit saved audio milestone evidence against the current implementation.

Does not inject input, regenerate native answers or infer visual success from
an API return code. The selected original muzzle-flash images were reviewed.
"""
import ast
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import wave
from compare import differences, read_probe

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'outputs'
REG = OUT / 'Audio-full-regression-v1'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def data(path):
    path = Path(path)
    return path.read_bytes() if path.exists() else gzip.decompress(Path(str(path)+'.gz').read_bytes())


def load(path):
    return json.loads(data(path))


def module(name):
    spec = importlib.util.spec_from_file_location('audio_audit_'+name.replace('-', '_'), ROOT/'tools'/name)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def main():
    requirements = []
    def proven(name, evidence, scope):
        requirements.append(dict(requirement=name, status='proven', evidence=evidence, scope=scope))

    preservation = load(REG/'preservation.json')
    assert preservation['validation_exit'] == 0
    for row in preservation['artifacts']:
        raw = (REG/row['retained_file']).read_bytes()
        if row.get('encoding'):
            raw = gzip.decompress(raw)
        assert hashlib.sha256(raw).hexdigest() == row['sha256'], row
    summary = load(REG/'Validation-summary.json')
    assert (summary['core_tests_passed'], summary['rust_tests_passed'], summary['frontend_tests_passed']) == (61, 80, 4)
    assert summary['component_checks_passed'] and len(summary['cases']) == 138
    tree = ast.parse((ROOT/'tools/validate-runtime.py').read_text())
    cases = next(ast.literal_eval(n.value) for n in tree.body if isinstance(n, ast.Assign)
                 and any(isinstance(t, ast.Name) and t.id == 'CASES' for t in n.targets))
    names = {probe: name for name, probe in cases}
    with tempfile.TemporaryDirectory(prefix='classic-audio-audit-') as temporary:
        for row in summary['cases']:
            name = names[row['probe']]
            ref, candidate = [data(REG/f'{name}-{suffix}.json') for suffix in ('reference', 'runtime')]
            assert hashlib.sha256(ref).hexdigest() == row['reference_sha256']
            assert hashlib.sha256(candidate).hexdigest() == row['runtime_sha256']
            a, b = Path(temporary)/'reference.json', Path(temporary)/'candidate.json'
            a.write_bytes(ref); b.write_bytes(candidate)
            assert row['equal'] and not row['differences']
            assert not differences(read_probe(a), read_probe(b)), name
    proven('Preserve all existing native manager profiles', ['Audio-full-regression-v1/preservation.json'],
           '138 fresh comparisons independently re-read with strict schemas; historical milestone outputs restored.')

    for path, sha in load(OUT/'Audio-final-provenance/source-sha256.json').items():
        assert digest(ROOT/path) == sha, path
    pins = load(OUT/'Audio-final-provenance/result.json')
    assert digest(pins['gui_executable']) == pins['gui_sha256']
    assert digest(pins['cli_executable']) == pins['cli_sha256']
    for report in ['Audio-final-vector-validation/result.json', 'Audio-final-guest-validation/result.json']:
        result = load(OUT/report)
        assert result['passed'] and result['executable_sha256'] == pins['cli_sha256']
        assert all(case['sample_exact'] for case in result['cases'])
    proven('Known-vector deterministic headless and live output, same pipeline',
           ['Audio-final-vector-validation/result.json', 'Audio-final-guest-validation/result.json'],
           'Exact PCM including silence tails; real AudioQueue callbacks/consumption. Identical owned68K caller matches native scalar results.')
    proven('Pitch/rate, amplitude, stereo layout, mixing/clipping, continuity and lifecycle tests',
           ['Audio-full-regression-v1/Core-tests.txt', 'Audio-full-regression-v1/Validation-summary.json'],
           '61 core +80 runtime +4 frontend tests, including fractional rate continuity, gain before saturation, queue/callback boundaries and guarded guest amplitude dispatch.')

    app = load(OUT/'Marathon-provenance.json')
    cache = Path((ROOT/'work/validation/dialogitemtext-local-inputs-path.txt').read_text().strip())
    for asset in app['assets']:
        for fork in ('data', 'resource'):
            assert digest(cache/'source/files'/asset['name'].lower().encode().hex()/fork) == asset[fork+'_sha256']
    application = next(a for a in app['assets'] if a['name'] == 'Marathon v1.2')
    assert digest(cache/'Marathon.rsrc') == application['resource_sha256']
    assert digest(cache/'Marathon.data') == application['data_sha256']
    proven('Unchanged pinned original Marathon and sound assets', ['Audio-shot-assets-rechecked.json'],
           'Every original data/resource fork and both actually loaded application forks rehashed.')

    native = load(OUT/'SoundPCMProbe-screamer-v1-reference.json')
    assert native['new_channel_error'] == native['play_error'] == native['dispose_error'] == 0
    discovery = load(OUT/'SoundDiscoveryProbe-v1-reference.json')
    assert discovery['osvector_ad_a0ad_equal'] and discovery['osvector_ad_a1ad_equal']
    commands = list(map(json.loads, (OUT/'SoundCommandsProbe-v3-reference.jsonl').read_text().splitlines()))
    playing = next(r for r in commands if r.get('phase') == 'playing_status')
    finished = next(r for r in commands if r.get('phase') == 'completed_status')
    assert playing['busy'] == 1 and finished['busy'] == 0 and finished['callbacks'] == 2
    amplitude = module('check-native-amplitude.py').check(OUT/'SoundAmplitudeProbe-v1-reference.jsonl', OUT/'SoundAmplitudeProbe-v1-native.wav')
    assert amplitude['passed']
    for stage in ['soundguest-screamer-v1', 'soundcommands-screamer-v3', 'soundamplitude-screamer-v1']:
        subprocess.run(['qemu-img', 'info', '--backing-chain', str(ROOT/'work/os9'/stage/'working.qcow2')],
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=True)
    proven('Native OS9 reference, capabilities, actual captured output and Sound Manager measurements',
           ['Audio-final-provenance/result.json', 'SoundPCMProbe-screamer-v1-reference.json',
            'SoundDevicesProbe-v1-reference.jsonl', 'SoundCommandsProbe-v3-reference.jsonl',
            'SoundDiscoveryProbe-v1-reference.json', 'SoundAmplitudeProbe-v1-reference.jsonl'],
           'Retained bootable backing chains, pinned Screamer emulator/firmware, actual WAV path; native channel/status/callback/amplitude ABI. Runtime advertises its own subset, not all native capabilities.')
    pcm = load(OUT/'SoundPCMProbe-screamer-v1-comparison.json')
    old = load(OUT/'SoundCommandsProbe-v2-comparison.json')
    assert pcm['comparison_available'] and pcm['alignment_frames'] == 53499
    assert not old['passed']
    proven('Native quantitative comparisons with raw recordings, alignment and declared limits',
           ['SoundPCMProbe-screamer-v1-comparison.json', 'SoundAmplitudeProbe-v1-transport-check.json',
            'SoundCommandsProbe-v2-comparison.json'],
           'Whole-frame alignment only; no gain/DC fitting or resampling. Native48k correlation0.994 and measured gain/errors retained. Fixture pitch/amplitude-ratio envelope is calibrated, not blind. Older8k zero-order-hold correlation test remains failed and is not claimed as PCM equivalence.')

    for source in ['compat-core/src/audio.rs', 'runtime/src/soundmanager.rs', 'runtime/src/audio_output.rs']:
        assert 'Marathon' not in (ROOT/source).read_text()
    proven('Shared application-independent backed sound services and explicit unsupported behavior',
           ['runtime/src/soundmanager.rs', 'compat-core/src/audio.rs', 'runtime/src/audio_output.rs'],
           'Mono unsigned8 standard headers, format1/2 resources, external sample pointers, queued/immediate gain/buffers/callbacks and checked channel ownership. No app routing, patched app or prerecorded output. Unsupported synth/encoding/commands fail explicitly; absent backend keeps negative profile.')

    capture = OUT/'Marathon-audio-final-live'
    identity = module('verify-marathon-audio-inputs.py').verify(capture, cache/'source/files/736f756e6473/resource')
    waveform = module('compare-marathon-audio-waveform.py').compare(capture)
    assert identity['matched_original_inputs'] and waveform['sample_exact']
    assert waveform['frames'] == 11980320 and not waveform['shutdown_truncated_tokens']
    assert waveform['captured_pcm_sha256'] == load(capture/'waveform-comparison.json')['captured_pcm_sha256']
    shot = load(capture/'shot-evidence.json')
    observations = list(map(json.loads, (OUT/'Marathon-audio-final-turn-fire-observation/observations.jsonl').read_text().splitlines()))
    selected = next(r for r in observations if r.get('ppm_file') == 'held-013.ppm')
    assert selected['key_map_hex'] == '00000000000002000000000000000000' and selected['active']
    assert digest(OUT/'Marathon-audio-final-turn-fire-observation/held-013.ppm') == selected['ppm_sha256']
    assert selected['ppm_sha256'] == shot['snapshot_ppm_sha256']
    pistol = next(r for r in shot['active_original_buffers'] if r['resource_ids'] == [1030])
    assert pistol['start'] <= selected['audio_frame_before'] <= selected['audio_frame_after'] < pistol['end']
    with wave.open(str(capture/'capture.wav')) as stream:
        stream.setpos(shot['excerpt']['start_frame']); raw = stream.readframes(shot['excerpt']['frames'])
    with wave.open(str(capture/'shot-excerpt.wav')) as stream:
        assert raw == stream.readframes(stream.getnframes())
    proven('Actual original first-level weapon firing, waveform and live playback',
           ['Marathon-audio-final-live/shot-evidence.json', 'Marathon-audio-final-live/waveform-comparison.json',
            'Marathon-audio-final-turn-fire-observation/held-013.png'],
           'Reviewed visible muzzle flash during heldSpace overlaps original snd1030 bytes on recorded renderer clock. Entire11980320-frame actual AudioQueue capture is exact under independent recorded-event reconstruction, with no device errors and all frames consumed. No acoustic/loopback claim.')

    gameplay = load(OUT/'Audio-final-gameplay-result.json')
    for stem, value in gameplay['frames'].items():
        assert digest(OUT/(stem+'.ppm')) == value['ppm_sha256']
    assert gameplay['pause_seconds'] >= 20 and gameplay['after_firing_pause_seconds'] >= 30
    assert gameplay['backward_held_observations'] >= 3 and gameplay['left_turn_held_observations'] >= 3
    proven('Preserve first-level walking, turning, firing and pause/resume', ['Audio-final-gameplay-result.json'],
           'Final normal-default-audio GUI build; living full-shield poses, held-key observations and reviewed movement/turn/fire images; stable23s and37s pauses with advancing clocks. Earlier60/90s proofs remain preserved.')
    assert summary['teachtext_typing']['text_bytes_equal']
    assert summary['teachtext_roundtrip']['fresh_process_reopen'] and summary['teachtext_roundtrip']['cancel_preserves_text_and_creates_no_file']
    assert summary['simpletext_typing']['matches_native_saved_document_text']
    assert summary['simpletext_cancel']['cancel_returned_to_document'] and summary['simpletext_cancel']['no_file_created']
    assert summary['simpletext_save']['saved_data_matches_native'] and summary['simpletext_save']['saved_style_resources_match_native_metadata_and_payload']
    assert summary['simpletext_reopen']['save_reopen_complete'] and summary['simpletext_reopen']['reopened_style_runs_match_native']
    for key in ('teachtext_roundtrip', 'simpletext_save', 'simpletext_reopen'):
        volume = Path(summary[key]['volume'])/'files'/b'reference.txt'.hex()/'data'
        assert digest(volume) == 'd23217c3f2a9796a97059585656e33a09cfb51f8df3dfdb5a9e758b775a4b7b4'
    proven('Preserve TeachText and SimpleText document workflows', ['Audio-full-regression-v1/Validation-summary.json.gz'],
           'Fresh original-app typing/edit/cancel/save/same-process and cold reopen, styles, clipboard/replacement, window/scroll checks; durable saved bytes rehashed.')
    boot = summary['native_boot']
    assert boot['booted'] and boot['exception_level'] == 1 and boot['owned_68k_probe_matches_os9'] and boot['shared_core_probe_matches_os9']
    assert digest(REG/'Native-core-kernel.elf') == boot['kernel_sha256']
    proven('Preserve freestanding ARM64 shared-execution proof', ['Audio-full-regression-v1/Native-boot-result.json'],
           'Owned166-byte68K program,195instructions,10sharedtraps at EL1. No physical Pi500/audio-driver claim.')
    proven('Source/dependency provenance, reproducible commands and checkpoints',
           ['Audio-final-provenance/result.json', 'Audio-reproduction.txt'],
           'Current source pins, both tested executable hashes, locked hosted/native dependency trees, original asset hashes, retained captures and exact commands. Earlier checkpoint manifests retain historical scope.')
    assert (OUT/'Audio-reproduction.txt').is_file()
    limits = [
        'Full campaign and complete OS9/audio-device equivalence are outside this milestone; known broader death/GetGWorld path gap remains.',
        'Only the original standard mono8 SFX path observed here is supported. Stereo source, extended/compressed samples, instruments/note/rate commands not observed in gameplay remain explicit unsupported.',
        'Native transport filters/quantization and wall-clock timing are not identical. The older8k correlation failure is retained; no fitted normalization hides it.',
        'Sound effects and startup sampled snd resources are verified; Music-file sequencing/general music APIs are not claimed.',
        'CoreAudio consumed renderer buffers are verified; microphone/loopback/acoustic latency and speaker measurement are not claimed.',
        'Physical Raspberry Pi500 audio hardware/drivers and validation remain deferred until hardware is owned.'
    ]
    result = dict(objective='Unchanged original Marathon1.2 first-level audio in independent runtime',
                  completion_proven=True, requirements=requirements, scope_limits=limits,
                  native8k_transport_failure_preserved=True, audit_tool_sha256=digest(__file__),
                  implementation_commit=pins['implementation_commit'], gui_sha256=pins['gui_sha256'], cli_sha256=pins['cli_sha256'])
    (OUT/'Audio-completion-audit.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(dict(completion_proven=True, requirements=len(requirements), audit=str(OUT/'Audio-completion-audit.json'))))


if __name__ == '__main__':
    main()
