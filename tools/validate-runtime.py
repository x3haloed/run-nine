#!/usr/bin/env python3
"""Re-run limited manager comparisons and record actual original-app progress.

The OS 9 observations are collected separately with collect-reference.py.
The initial milestone also requires the separate reference, provenance and UI
evidence assessed in outputs/Goal-completion-audit.txt.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'outputs'
CASES = [('MouseCouplingProbe','mouse-coupling-v1'),('RoundRectProbe','round-rect-v1'),('TextBoxProbe','text-box-v1'),('DialogItemTextProbe','dialog-item-text-v1'),('TruncationProbe','text-truncation-v1'),('QDLineProbe','qd-line-v1'),('ModalDialogProbe','modal-dialog-v1'),('NewDialogDetailsProbe','new-dialog-v1'),('IndResourceProbe','ind-resource-v1'),('PenPatProbe','pen-pattern-v1'),('HGetVInfoProbe','hgetvinfo-v1'),('MMUModeProbe','mmu-mode-v1'),('OSEventProbe','os-events-v1'),('GetKeysProbe','get-keys-v1'),('QDRandomProbe','qd-random-v1'),('FlushFileProbe','flush-file-v1'),('StillDownProbe','still-down-v1'),('CountResourcesProbe','count-resources-v1'),('InverseFourSemanticsProbe','inverse-four-semantics-v1'),('LegacyColorProbe','legacy-color-v1'),('ColorReadProbe','color-read-v1'),('PICTBitmapProbe','pict-bitmap-v1'),('CursorStateProbe','cursor-state-v1'),('CursorMotionProbe','cursor-motion-v1'),('SANEIntegerPowerSemanticsProbe','sane-small-integer-power-semantics-v1'),('SANEFL2XSemanticsProbe','sane-long-widen-semantics-v1'),('SANEFS2XSemanticsProbe','sane-single-widen-semantics-v1'),('CTabChangedProbe','ctab-changed-v1'),('CTSeedProbe','color-seed-v1'),('WindowDamageProbe','window-damage-v2'),('EmptyDriverProbe','empty-driver-v1'),('PixMapProbe','pixmap-v1'),('CourierProbe','courier-font-v1'),('GetCTableProbe','get-color-table-v1'),('AppReleaseProbe','app-resource-release-v1'),('SANECompareSemanticsProbe','sane-compare-semantics-v1'),('SANEMixedAddSemanticsProbe','sane-mixedadd-semantics-v1'),('SANECosSemanticsProbe','sane-cos-semantics-v1'),('OpenDFSemanticsProbe','open-df-semantics-v2'),('SANETanSemanticsProbe','sane-tan-semantics-v1'),('SANENarrowSemanticsProbe','sane-narrow-semantics-v1'),('SANERoundSemanticsProbe','sane-round-semantics-v1'),('SANEMixedSemanticsProbe','sane-mixed-semantics-v1'),('SANEEnvSemanticsProbe','sane-env-semantics-v1'),('SANEBasicXSemanticsProbe','sane-basicx-semantics-v1'),('SANEClassSemanticsProbe','sane-class-semantics-v1'),('SANEExtendedSemanticsProbe','sane-extended-semantics-v1'),('SANESineSemanticsProbe','sane-sine-semantics-v1'),('SANEDivSemanticsProbe','sane-div-semantics-v1'),('SANEIntegerSemanticsProbe','sane-integer-semantics-v1'),('SANEWidenSemanticsProbe','sane-widen-semantics-v1'),('SANEConvertSemanticsProbe','sane-convert-semantics-v1'),('SANEMulSemanticsProbe','sane-mul-semantics-v1'),('SANESemanticsProbe','sane-semantics-v1'),('InverseSemanticsProbe','inverse-semantics-v1'),('HandCloneProbe','hand-clone-v1'),('OffDeviceSemanticsProbe','off-device-v1'),('MaxDeviceProbe','max-device-v1'),('OriginProbe','origin-v1'),('VBLSemanticsProbe','vbl-semantics-v1'),('WindowBackgroundProbe','window-background-v1'),('WindowColorsProbe','window-colors-v1'),('MoveHHiSemanticsProbe','move-high-semantics-v1'),('IndexedColorProbe','indexed-color-v1'),('TimeManagerSemanticsProbe','time-manager-semantics-v1'),('AlertProbe','alerts-v1'),('SoundNoOutputProbe','sound-no-output-v1'),('AddressingProbe','addressing-v1'),('Owned68KProbe','owned-68k-v1'),('SoundVersionABIProbe','sound-version-abi-v1'),('CatalogSemanticsProbe','catalog-semantics-v1'),('FCBSemanticsProbe','fcb-semantics-v1'),('PathPBSemanticsProbe','path-files-pb-semantics-v1'),('ControlThumbEdgesSemanticsProbe','control-thumb-edges-semantics-v1'),('ControlThumbSemanticsProbe','control-thumb-semantics-v1'),('ControlTrackProbe','control-action-v1'),('TEDoubleProbe','te-double-v1'),('ControlHitProbe','control-hit-v1'),('WindowTrackingProbe','window-tracking-v1'),('WindowGeometryProbe','window-geometry-v1'),('StyleUseProbe','style-use-v1'),('HandleStateProbe','handle-state-v1'),('DocLifecycleProbe','doc-lifecycle-v1'),('ResIOProbe','res-io-v1'),('StyleScrapProbe','style-scrap-v1'),('ResCreateProbe','res-create-v1'),('UserItemsProbe','user-items-v1'),('IconPlotProbe','icon-plot-v1'),('DelayProbe','delay-v1'),('ModalFilterProbe','modal-filter-v1'),('StandardFilterProbe','standard-filter-v1'),('FontGlobalsProbe','font-globals-v1'),('ScriptContextProbe','script-context-v1'),('ResourceEnumProbe','resource-enum-v1'),('StringCompareProbe','string-compare-v1'),('FontAvailabilityProbe','font-availability-v1'),('TEContinuousControlProbe','te-continuous-control-v1'),('TEOutlineRefreshProbe','te-outline-refresh-v1'),('TEArrowsProbe','te-arrows-v1'),('TEContinuousProbe','te-continuous-v1'),('AutoViewProbe','te-auto-view-v1'),('StyleQueriesProbe','style-queries-v1'),('StyledTextProbe','styled-text-v1'),('StyledTextVariantProbe','styled-text-variant-v1'),('WindowResourcesProbe','window-resources-v1'),('WindowResourceFlowProbe','window-resource-flow-v1'),('AEHandlersProbe','apple-event-handlers-v1'),('MenuItemsProbe','menu-items-v1'),('MenuInvalidationProbe','menu-semantics-v1'),('FontProfileProbe', 'font-profile-v1'), ('SystemStyleProbe', 'system-styles-v1'), ('MenuBarProbe', 'menu-bars-v1'), ('GestaltProbe', 'gestalt-v1'), ('EnvironsProbe', 'environs-v1'), ('SegmentProbe', 'segments-v1'), ('AppSourceProbe', 'app-source-v1'), ('DialogParamProbe', 'dialog-params-v1'), ('FileDeleteProbe', 'file-delete-v1'), ('SaveConfirmProbe', 'save-confirm-v1'), ('ProcessProbe', 'processes-v1'), ('ClipboardProbe', 'clipboard-v1'), ('CoreProbe', 'core-v1'), ('QuickDrawProbe', 'quickdraw-v1'),
         ('WindowProbe', 'windows-v1'), ('MemoryFlagsProbe', 'memory-flags-v1'),
         ('RegionProbe', 'regions-v1'), ('ColorPortProbe', 'color-ports-v1'),
         ('TextEditProbe', 'textedit-v1'), ('TextLayoutProbe', 'text-layout-v1'),
         ('ControlProbe', 'controls-v1'), ('WindowFlowProbe', 'window-flow-v1'),
         ('FileProbe', 'files-v1'), ('ResFileProbe', 'res-files-v1'),
         ('TextDirectProbe', 'text-direct-v1'), ('HitTestProbe', 'hit-test-v1'), ('TitleMetricsProbe', 'title-metrics-v1'),
         ('TitleBoundsProbe', 'title-bounds-v1'), ('TitlePairsProbe', 'title-pairs-v1')]

def run(*args):
    return subprocess.run(args, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT)

def checked(*args):
    result = run(*args)
    if result.returncode:
        raise RuntimeError(result.stdout)
    return result.stdout

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    checked(sys.executable,str(ROOT/'tools/check-mouse-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-round-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-box-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-dialogtext-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-truncation-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-line-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-modal-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-newdialog-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-indresource-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-penpat-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-hgetvinfo-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-mmumode-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-osevent-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-getkeys-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-qdrandom-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-flushfile-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-stilldown-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-countresource-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-inverse4-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-legacycolor-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-pictbitmap-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-cursor-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneintpower-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanei32widen-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanef32widen-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-ctabchanged-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-ctseed-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-windowdamage-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-emptydriver-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-pixmap-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-courier-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-getctable-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-apprelease-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanecompare-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanemixedadd-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-cossane-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-opendf-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-tansane-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanenarrow-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneround-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanemixed-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneenv-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanebasicx-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneclass-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneextended-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sinesane-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanediv-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneinteger-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanewiden-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-saneconvert-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sanemul-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-sane-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-inverse-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-hand-clone-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-off-device-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-max-device-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-origin-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-vbl-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-window-background-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-window-colors-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-move-high-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-indexed-color-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-time-manager-reference.py'))
    core_test_output=checked('cargo','test','--manifest-path',str(ROOT/'compat-core/Cargo.toml'))
    (OUT/'Core-tests.txt').write_text(core_test_output.rstrip()+'\n')
    checked(sys.executable,str(ROOT/'tools/check-alert-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-cpu-port.py'))
    checked(sys.executable,str(ROOT/'tools/check-owned-execution.py'))
    checked(sys.executable,str(ROOT/'tools/check-sound-version-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-catalog-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-fcb-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-path-pb-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-control-thumb-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-control-thumb-edges-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-resio-reference.py'))
    checked(sys.executable,str(ROOT/'tools/check-delay-reference.py'))
    comparison_checks=json.loads(checked(sys.executable,str(ROOT/'tools/check-comparison-invariants.py')))
    manifest = str(ROOT / 'runtime/Cargo.toml')
    test_output = checked('cargo', 'test', '--manifest-path', manifest)
    counts = re.findall(r'test result: ok\. (\d+) passed;', test_output)
    if not counts:
        raise RuntimeError('Cannot verify Rust test count')
    gui_test_output = checked('cargo', 'test', '--manifest-path', manifest,
                              '--features', 'gui', 'frontend::tests')
    gui_counts = re.findall(r'test result: ok\. (\d+) passed;', gui_test_output)
    if not gui_counts:
        raise RuntimeError('Cannot verify frontend test count')
    (OUT/'FontCatalogProbe-runtime.json').write_text(checked('cargo','run','--quiet','--manifest-path',manifest,'--','--probe','font-catalog-v1'))
    cases = []
    for name, probe in CASES:
        candidate = OUT / f'{name}-runtime.json'
        if probe not in ['mouse-coupling-v1','round-rect-v1','text-box-v1','dialog-item-text-v1','text-truncation-v1','qd-line-v1','modal-dialog-v1','new-dialog-v1','ind-resource-v1','pen-pattern-v1','hgetvinfo-v1','mmu-mode-v1','os-events-v1','get-keys-v1','qd-random-v1','flush-file-v1','still-down-v1','count-resources-v1','inverse-four-semantics-v1','legacy-color-v1','color-read-v1','pict-bitmap-v1','cursor-state-v1','cursor-motion-v1','sane-small-integer-power-semantics-v1','sane-long-widen-semantics-v1','sane-single-widen-semantics-v1','ctab-changed-v1','color-seed-v1','window-damage-v2','empty-driver-v1','pixmap-v1','courier-font-v1','get-color-table-v1','app-resource-release-v1','sane-compare-semantics-v1','sane-mixedadd-semantics-v1','sane-cos-semantics-v1','open-df-semantics-v2','sane-tan-semantics-v1','sane-narrow-semantics-v1','sane-round-semantics-v1','sane-mixed-semantics-v1','sane-env-semantics-v1','sane-basicx-semantics-v1','sane-class-semantics-v1','sane-extended-semantics-v1','sane-sine-semantics-v1','sane-div-semantics-v1','sane-integer-semantics-v1','sane-widen-semantics-v1','sane-convert-semantics-v1','sane-mul-semantics-v1','sane-semantics-v1','inverse-semantics-v1','off-device-v1','vbl-semantics-v1','time-manager-semantics-v1','move-high-semantics-v1']:
            candidate.write_text(checked('cargo', 'run', '--quiet', '--manifest-path',
                                         manifest, '--', '--probe', probe))
        reference = OUT / f'{name}-reference.json'
        comparison = run(sys.executable, str(ROOT / 'tools/compare.py'),
                             str(reference), str(candidate)).stdout
        (OUT / f'{name}-comparison.json').write_text(comparison)
        cases.append({'probe': probe, 'reference_sha256': sha(reference),
                      'runtime_sha256': sha(candidate), **json.loads(comparison)})
    font_checks=json.loads(checked(sys.executable,str(ROOT/'tools/check-font-profile-invariants.py')))
    application = ROOT / 'work/apps/TeachText/TeachText.rsrc'
    result = run('cargo', 'run', '--quiet', '--manifest-path', manifest, '--',
                 str(application), '--trace')
    (OUT / 'TeachText-runtime-trace.txt').write_text(result.stdout)
    script = ROOT / 'probes/teachtext/new-document.json'
    scripted = run('cargo', 'run', '--quiet', '--manifest-path', manifest, '--',
                   str(application), '--trace', '--event-script', str(script))
    (OUT / 'TeachText-scripted-trace.txt').write_text(scripted.stdout)
    typing_script = ROOT / 'probes/teachtext/type-document.json'
    ppm = ROOT / 'work/validation/TeachText.ppm'
    ppm.parent.mkdir(parents=True, exist_ok=True)
    documents_path = OUT / 'TeachText-own-documents.json'
    documents_path.unlink(missing_ok=True)
    typed = run('cargo', 'run', '--quiet', '--manifest-path', manifest, '--',
                str(application), '--trace', '--event-script', str(typing_script),
                '--dump-documents', str(documents_path), '--capture-framebuffer', str(ppm))
    (OUT / 'TeachText-typing-trace.txt').write_text(typed.stdout)
    documents = json.loads(documents_path.read_text())['documents']
    expected = (OUT / 'TeachText-reference-document.txt').read_bytes()
    delivered = typed.stdout.count('script event:')
    event_count = len(json.loads(typing_script.read_text())['events'])
    text_matches = len(documents)==1 and bytes(documents[0]['text_bytes'])==expected
    typing_result = {'application': 'TeachText 7.0', 'resource_sha256': sha(application),
                     'action': 'original New command and ordinary keyboard events',
                     'event_script_sha256': sha(typing_script),
                     'events_delivered': delivered, 'events_expected': event_count,
                     'reference_text_sha256': hashlib.sha256(expected).hexdigest(),
                     'text_bytes_equal': text_matches,
                     'runtime_status': typed.stdout.strip().splitlines()[-1],
                     'saving_verified': False}
    (OUT / 'TeachText-own-result.json').write_text(json.dumps(typing_result,indent=2)+'\n')
    checked('sips', '-s', 'format', 'png', str(ppm), '--out', str(OUT / 'TeachText-own-runtime.png'))
    saving = run('cargo', 'run', '--quiet', '--manifest-path', manifest, '--',
                 str(application), '--trace', '--event-script',
                 str(ROOT / 'probes/teachtext/save-document.json'))
    (OUT / 'TeachText-save-trace.txt').write_text(saving.stdout)
    checked(sys.executable, str(ROOT / 'tools/verify-app-roundtrip.py'))
    roundtrip=json.loads((OUT / 'TeachText-roundtrip-result.json').read_text())
    checked(sys.executable, str(ROOT / 'tools/verify-app-clipboard.py'))
    clipboard=json.loads((OUT / 'TeachText-clipboard-result.json').read_text())
    checked(sys.executable, str(ROOT / 'tools/verify-app-replacement.py'))
    replacement=json.loads((OUT / 'TeachText-replacement-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-teachtext-move.py'))
    teachtext_move=json.loads((OUT/'TeachText-move-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-teachtext-resize.py'))
    teachtext_resize=json.loads((OUT/'TeachText-resize-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-teachtext-scroll.py'))
    teachtext_scroll=json.loads((OUT/'TeachText-scroll-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-teachtext-thumb.py'))
    teachtext_thumb=json.loads((OUT/'TeachText-thumb-result.json').read_text())
    checked(sys.executable, str(ROOT / 'tools/verify-simpletext-typing.py'))
    simpletext=json.loads((OUT/'SimpleText-typing-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-simpletext-cancel.py'))
    simpletext_cancel=json.loads((OUT/'SimpleText-cancel-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-simpletext-save.py'))
    simpletext_save=json.loads((OUT/'SimpleText-save-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-simpletext-reopen.py'))
    simpletext_reopen=json.loads((OUT/'SimpleText-reopen-result.json').read_text())
    checked(sys.executable,str(ROOT/'tools/verify-resource-persistence.py'))
    resource_persistence=json.loads((OUT/'Resource-persistence-result.json').read_text())
    checked(sys.executable, str(ROOT / 'tools/verify-native-boot.py'))
    native_boot=json.loads((OUT / 'Native-boot-result.json').read_text())
    marathon_alert=json.loads(checked(sys.executable,str(ROOT/'tools/verify-marathon-alert.py')))
    marathon_indexed=json.loads(checked(sys.executable,str(ROOT/'tools/verify-marathon-alert.py'),'--color-screen'))
    report = {'core_tests_passed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',core_test_output))), 'cases': cases, 'rust_tests_passed': sum(map(int, counts)),
              'frontend_tests_passed': sum(map(int, gui_counts)),
              'component_checks_passed': all(case['equal'] for case in cases),
              'teachtext_resource_sha256': sha(application),
              'teachtext_trace_exit_code': result.returncode,
              'teachtext_runtime_status': result.stdout.strip().splitlines()[-1],
              'teachtext_event_script_sha256': sha(script),
              'teachtext_scripted_exit_code': scripted.returncode,
              'teachtext_scripted_status': scripted.stdout.strip().splitlines()[-1],
              'teachtext_typing': typing_result,
              'teachtext_save_status': saving.stdout.strip().splitlines()[-1],
              'teachtext_roundtrip': roundtrip,
              'native_boot': native_boot,
              'marathon_sound_warning': marathon_alert,
              'marathon_indexed_progress': marathon_indexed,
              'teachtext_move': teachtext_move,
              'teachtext_resize': teachtext_resize,
              'teachtext_scroll': teachtext_scroll,
              'teachtext_thumb': teachtext_thumb,
              'simpletext_typing': simpletext,
              'simpletext_cancel': simpletext_cancel,
              'simpletext_save': simpletext_save,
              'simpletext_reopen': simpletext_reopen,
              'resource_persistence':resource_persistence,
              'teachtext_clipboard': clipboard,
              'teachtext_replacement': replacement,
              'comparison_invariant_checks': comparison_checks,
              'font_profile_invariant_checks': font_checks,
              'initial_trial_audit': 'Goal-completion-audit.txt',
              'active_goal_plan': 'Marathon-pass-plan.txt',
              'active_goal_complete': False,
              'full_application_compatibility': False,
              'scope': 'Selected API comparisons and bounded original-application document workflows. Font observations supply compatibility data. A mouse/keyboard frontend and original click-loop hook are separately verified through native UI automation. Full window/control tracking, freestanding application execution, physical Pi support and full application/system coverage remain incomplete; no pixel-equivalence or production security claim.'}
    (OUT / 'Validation-summary.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 0 if report['component_checks_passed'] and text_matches and delivered==event_count else 1

if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        (OUT / 'Validation-summary.json').write_text(json.dumps({'validation_error':str(error),'full_application_compatibility':False},indent=2)+'\n')
        raise
