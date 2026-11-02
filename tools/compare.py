#!/usr/bin/env python3
"""Compare observed JSON probe results. Exits nonzero on any mismatch."""
import argparse
import json
from pathlib import Path

def differences(reference, candidate, path='$'):
    if path=='$' and isinstance(reference,dict) and isinstance(candidate,dict) and reference.get('probe')==candidate.get('probe')=='sane-semantics-v1':
        from sane_observations import bounded_differences
        return bounded_differences(reference,candidate)
    if path=='$' and isinstance(reference,dict) and isinstance(candidate,dict) and reference.get('probe')==candidate.get('probe')=='sane-sine-semantics-v1':
        from sinesane_observations import bounded_differences
        return bounded_differences(reference,candidate)
    if path=='$' and isinstance(reference,dict) and isinstance(candidate,dict) and reference.get('probe')==candidate.get('probe')=='sane-tan-semantics-v1':
        from tansane_observations import bounded_differences
        return bounded_differences(reference,candidate)
    if path=='$' and isinstance(reference,dict) and isinstance(candidate,dict) and reference.get('probe')==candidate.get('probe')=='sane-cos-semantics-v1':
        from cossane_observations import bounded_differences
        return bounded_differences(reference,candidate)
    return _exact_differences(reference,candidate,path)

def _exact_differences(reference, candidate, path='$'):
    if type(reference) is not type(candidate):
        return [{'path': path, 'reference': reference, 'candidate': candidate}]
    if isinstance(reference, dict):
        result = []
        for key in sorted(reference.keys() | candidate.keys()):
            if key not in reference or key not in candidate:
                result.append({'path': f'{path}.{key}', 'missing': 'reference' if key not in reference else 'candidate'})
            else:
                result.extend(differences(reference[key], candidate[key], f'{path}.{key}'))
        return result
    if isinstance(reference, list):
        if len(reference) != len(candidate):
            return [{'path': path, 'reference_length': len(reference), 'candidate_length': len(candidate)}]
        return [difference for i, (a, b) in enumerate(zip(reference, candidate)) for difference in differences(a, b, f'{path}[{i}]')]
    return [] if reference == candidate else [{'path': path, 'reference': reference, 'candidate': candidate}]

def read_probe(path):
    value = json.loads(Path(path).read_text())
    if not isinstance(value, dict) or value.get('schema') != 1:
        raise ValueError(f'{path}: expected probe observations with schema 1')
    sections={'mouse-coupling-v1':('mouse',),'round-rect-v1':('drawing',),'text-box-v1':('text',),'dialog-item-text-v1':('dialogs',),'text-truncation-v1':('text',),'qd-line-v1':('drawing',),'modal-dialog-v1':('dialogs',),'new-dialog-v1':('dialogs',),'ind-resource-v1':('resources',),'pen-pattern-v1':('pens',),'hgetvinfo-v1':('volumes',),'mmu-mode-v1':('addressing',),'os-events-v1':('events',),'get-keys-v1':('events',),'qd-random-v1':('random',),'flush-file-v1':('files',),'still-down-v1':('events',),'count-resources-v1':('resources',),'inverse-four-semantics-v1':('color',),'legacy-color-v1':('colors',),'color-read-v1':('colors',),'pict-bitmap-v1':('pictures',),'cursor-state-v1':('cursors',),'cursor-motion-v1':('cursors',),'sane-small-integer-power-semantics-v1':('math',),'sane-long-widen-semantics-v1':('math',),'sane-single-widen-semantics-v1':('math',),'ctab-changed-v1':('color',),'color-seed-v1':('color',),'window-damage-v2':('windows',),'empty-driver-v1':('drivers',),'pixmap-v1':('pixmaps',),'courier-font-v1':('fonts',),'get-color-table-v1':('color',),'app-resource-release-v1':('resources',),'sane-compare-semantics-v1':('math',),'sane-mixedadd-semantics-v1':('math',),'sane-cos-semantics-v1':('math',),'open-df-semantics-v2':('files',),'sane-tan-semantics-v1':('math',),'sane-narrow-semantics-v1':('math',),'sane-round-semantics-v1':('math',),'sane-mixed-semantics-v1':('math',),'sane-env-semantics-v1':('math',),'sane-basicx-semantics-v1':('math',),'sane-class-semantics-v1':('math',),'sane-extended-semantics-v1':('math',),'sane-sine-semantics-v1':('math',),'sane-div-semantics-v1':('math',),'sane-integer-semantics-v1':('math',),'sane-widen-semantics-v1':('math',),'sane-convert-semantics-v1':('math',),'sane-mul-semantics-v1':('math',),'sane-semantics-v1':('math',),'inverse-semantics-v1':('color',),'hand-clone-v1':('memory',),'off-device-v1':('devices',),'max-device-v1':('devices',),'origin-v1':('ports',),'vbl-semantics-v1':('retrace',),'window-background-v1':('windows',),'window-colors-v1':('windows',),'move-high-semantics-v1':('memory',),'indexed-color-v1':('color',),'time-manager-semantics-v1':('timers',),'alerts-v1':('alerts',),'sound-no-output-v1':('sound',),'addressing-v1':('addressing',),'owned-68k-v1':('execution',),'sound-version-abi-v1':('sound',),'catalog-semantics-v1':('files',),'fcb-semantics-v1':('files','startup'),'path-files-pb-semantics-v1':('files',),'control-thumb-edges-semantics-v1':('controls',),'control-thumb-semantics-v1':('controls',),'control-action-v1':('controls',),'te-double-v1':('textedit',),'control-hit-v1':('controls',),'window-tracking-v1':('windows',),'window-geometry-v1':('windows',),'style-use-v1':('textedit',),'handle-state-v1':('memory',),'doc-lifecycle-v1':('windows','dialogs'),'style-scrap-v1':('textedit',),'res-io-v1':('resources',),'res-create-v1':('resources',),'icon-plot-v1':('icons',),'user-items-v1':('dialogs',),'delay-v1':('timing',),'modal-filter-v1':('dialogs',),'standard-filter-v1':('dialogs',),'font-globals-v1':('fonts',),'script-context-v1':('scripts',),'font-availability-v1':('fonts',),'string-compare-v1':('strings',),'resource-enum-v1':('resources',),'te-outline-refresh-v1':('textedit',),'te-arrows-v1':('textedit',),'te-continuous-control-v1':('textedit',),'te-continuous-v1':('textedit',),'te-auto-view-v1':('textedit',),'style-queries-v1':('textedit',),'styled-text-v1':('textedit',),'window-resources-v1':('windows',),'window-resource-flow-v1':('windows',),'apple-event-handlers-v1':('handlers',),'menu-items-v1':('menus',),'menu-semantics-v1':('menus',),'font-catalog-v1':('fonts',),'font-profile-v1':('fonts',),'system-styles-v1':('styles',),'menu-bars-v1':('menus',),'gestalt-v1':(),'environs-v1':('environment',),'segments-v1':('segments',),'app-source-v1':('source',),'dialog-params-v1':('parameters',),'file-delete-v1':('files',),'save-confirm-v1':('save',),'processes-v1':('processes',),'clipboard-v1':('clipboard',),'core-v1':('memory','rectangles'),'quickdraw-v1':('globals','port','changed'),'windows-v1':('initial','window'),'memory-flags-v1':('flags',),'regions-v1':('regions',),'color-ports-v1':('port',),'textedit-v1':('textedit',),'text-layout-v1':('textedit',),'controls-v1':('controls','services'),'window-flow-v1':('windows',),'files-v1':('files',),'res-files-v1':('resources',),'text-direct-v1':('textedit',),'hit-test-v1':('windows','textedit'),'title-metrics-v1':('metrics',),'title-bounds-v1':(),'title-pairs-v1':()}
    if value.get('probe') not in sections:
        raise ValueError(f'{path}: unrecognized probe identifier')
    for key in sections[value['probe']]:
        if not isinstance(value.get(key), dict) or not value[key]:
            raise ValueError(f'{path}: missing {key} observations')
    if value['probe']=='mouse-coupling-v1':
        from mouse_observations import validate
        validate(value)
    if value['probe']=='ctab-changed-v1':
        from ctabchanged_observations import validate
        validate(value)
    if value['probe']=='color-seed-v1':
        from ctseed_observations import validate
        validate(value)
    if value['probe']=='window-damage-v2':
        from damage_observations import validate
        validate(value)
    if value['probe']=='empty-driver-v1':
        from driver_observations import validate
        validate(value)
    if value['probe']=='pixmap-v1':
        from pixmap_observations import validate
        validate(value)
    if value['probe']=='courier-font-v1':
        from courier_observations import validate
        validate(value)
    if value['probe']=='get-color-table-v1':
        from getctable_observations import validate
        validate(value)
    if value['probe']=='app-resource-release-v1':
        from apprelease_observations import validate
        validate(value)
    if value['probe']=='sane-compare-semantics-v1':
        from sanecompare_observations import validate
        validate(value)
    if value['probe']=='sane-mixedadd-semantics-v1':
        from sanemixedadd_observations import validate
        validate(value)
    if value['probe']=='sane-cos-semantics-v1':
        from cossane_observations import validate
        validate(value)
    if value['probe']=='open-df-semantics-v2':
        from opendf_observations import validate
        validate(value)
    if value['probe']=='sane-tan-semantics-v1':
        from tansane_observations import validate
        validate(value)
    if value['probe']=='sane-narrow-semantics-v1':
        from sanenarrow_observations import validate
        validate(value)
    if value['probe']=='sane-round-semantics-v1':
        from saneround_observations import validate
        validate(value)
    if value['probe']=='sane-mixed-semantics-v1':
        from sanemixed_observations import validate
        validate(value)
    if value['probe']=='sane-env-semantics-v1':
        from saneenv_observations import validate
        validate(value)
    if value['probe']=='sane-basicx-semantics-v1':
        from sanebasicx_observations import validate
        validate(value)
    if value['probe']=='sane-class-semantics-v1':
        from saneclass_observations import validate
        validate(value)
    if value['probe']=='sane-extended-semantics-v1':
        from saneextended_observations import validate
        validate(value)
    if value['probe']=='sane-sine-semantics-v1':
        from sinesane_observations import validate
        validate(value)
    if value['probe']=='sane-div-semantics-v1':
        from sanediv_observations import validate
        validate(value)
    if value['probe']=='sane-integer-semantics-v1':
        from saneinteger_observations import validate
        validate(value)
    if value['probe']=='still-down-v1':
        from stilldown_observations import validate
        validate(value)
    if value['probe']=='round-rect-v1':
        from round_observations import validate
        validate(value)
    if value['probe']=='text-box-v1':
        from box_observations import validate
        validate(value)
    if value['probe']=='dialog-item-text-v1':
        from dialogtext_observations import validate
        validate(value)
    if value['probe']=='text-truncation-v1':
        from truncation_observations import validate
        validate(value)
    if value['probe']=='qd-line-v1':
        from line_observations import validate
        validate(value)
    if value['probe']=='modal-dialog-v1':
        from modal_observations import validate
        validate(value)
    if value['probe']=='new-dialog-v1':
        from newdialog_observations import validate
        validate(value)
    if value['probe']=='ind-resource-v1':
        from indresource_observations import validate
        validate(value)
    if value['probe']=='pen-pattern-v1':
        from penpat_observations import validate
        validate(value)
    if value['probe']=='hgetvinfo-v1':
        from hgetvinfo_observations import validate
        validate(value)
    if value['probe']=='mmu-mode-v1':
        from mmumode_observations import validate
        validate(value)
    if value['probe']=='os-events-v1':
        from osevent_observations import validate
        validate(value)
    if value['probe']=='get-keys-v1':
        from getkeys_observations import validate
        validate(value)
    if value['probe']=='qd-random-v1':
        from random_observations import validate
        validate(value)
    if value['probe']=='flush-file-v1':
        from flushfile_observations import validate
        validate(value)
    if value['probe']=='count-resources-v1':
        from countresource_observations import validate
        validate(value)
    if value['probe']=='inverse-four-semantics-v1':
        from inverse4_observations import validate
        validate(value)
    if value['probe']in['legacy-color-v1','color-read-v1']:
        from legacycolor_observations import validate
        validate(value)
    if value['probe']=='pict-bitmap-v1':
        from pictbitmap_observations import validate
        validate(value)
    if value['probe']in['cursor-state-v1','cursor-motion-v1']:
        from cursor_observations import validate
        validate(value)
    if value['probe']=='sane-small-integer-power-semantics-v1':
        from saneintpower_observations import validate
        validate(value)
    if value['probe']=='sane-long-widen-semantics-v1':
        from sanei32widen_observations import validate
        validate(value)
    if value['probe']=='sane-single-widen-semantics-v1':
        from sanef32widen_observations import validate
        validate(value)
    if value['probe']=='sane-widen-semantics-v1':
        from sanewiden_observations import validate
        validate(value)
    if value['probe']=='sane-convert-semantics-v1':
        from saneconvert_observations import validate
        validate(value)
    if value['probe']=='sane-mul-semantics-v1':
        from sanemul_observations import validate
        validate(value)
    if value['probe']=='sane-semantics-v1':
        from sane_observations import validate
        validate(value)
    if value['probe']=='inverse-semantics-v1':
        from inverse_observations import validate
        validate(value)
    if value['probe']=='hand-clone-v1':
        from clone_observations import validate
        validate(value)
    if value['probe']=='off-device-v1':
        from off_device_observations import validate
        validate(value)
    if value['probe']=='max-device-v1':
        from device_observations import validate
        validate(value)
    if value['probe']=='origin-v1':
        from origin_observations import validate
        validate(value)
    if value['probe']=='vbl-semantics-v1':
        from vbl_observations import validate
        validate(value)
    if value['probe']=='window-background-v1':
        rows=value['windows'].get('cases')
        fields={'kind','id','seed_rgb','shown_rgb','port_bg','erased_rgb','after_set_rgb','reshown_rgb'}
        if set(value)!= {'schema','probe','windows'} or set(value['windows'])!={'cases'} or not isinstance(rows,list) or len(rows)!=4:raise ValueError(f'{path}: incomplete window background cases')
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=fields or row['kind']!=i or row['id']!=[22000,22001,22001,22002][i]:raise ValueError(f'{path}: missing background context')
            for key in fields-{'kind','id'}:
                if not isinstance(row[key],list) or len(row[key])!=3 or any(type(v) is not int for v in row[key]):raise ValueError(f'{path}: missing background RGB')
    if value['probe']=='window-colors-v1':
        from window_color_observations import validate
        validate(value)
    if value['probe']=='move-high-semantics-v1':
        from move_observations import validate
        validate(value)
    if value['probe']=='indexed-color-v1':
        from color_observations import validate
        validate(value)
    if value['probe']=='time-manager-semantics-v1':
        from timer_observations import validate
        validate(value)
    if value['probe']=='sound-no-output-v1':
        rows=value['sound'].get('cases')
        if not isinstance(rows,list) or len(rows)!=6:raise ValueError(f'{path}: incomplete absent-output cases')
        expected={'synth','init','input_ccr','error','ccr','stack_delta','stack_guard','channel_null','pointer_guards'}
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or any(type(row[k]) is not int for k in expected-{'pointer_guards'}):raise ValueError(f'{path}: incomplete sound failure ABI')
            if row['synth']!=-1 or row['init']!=[0x7a5a3921,0x74737421][i//3] or row['input_ccr']!=[16,31,0][i%3]:raise ValueError(f'{path}: missing absent-output context')
            if not isinstance(row['pointer_guards'],list) or len(row['pointer_guards'])!=2 or any(type(n) is not int for n in row['pointer_guards']):raise ValueError(f'{path}: incomplete channel pointer guards')
    if value['probe']=='alerts-v1':
        rows=value['alerts'].get('cases')
        if not isinstance(rows,list) or len(rows)!=16:raise ValueError(f'{path}: incomplete alert variants/positions')
        expected={'resource','mode','callback','result','calls','old_port_restored','owner_front'}
        inner={'current_dialog','front_dialog','kind','visible','default_item','edit_field','port_rect','content','text_kind','text_hex','structure','event_nonnull','hit_nonnull'}
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or any(type(row[k]) is not int for k in expected-{'callback'}):raise ValueError(f'{path}: incomplete alert return')
            if row['resource']!=128+i//4 or row['mode']!=i%4:raise ValueError(f'{path}: missing alert context')
            callback=row['callback']
            if not isinstance(callback,dict) or set(callback)!=inner or any(type(callback[k]) is not int for k in inner-{'port_rect','content','text_hex','structure'}):raise ValueError(f'{path}: incomplete guest filter state')
            for field in ['port_rect','content','structure']:
                if not isinstance(callback[field],list) or len(callback[field])!=4 or any(type(n) is not int for n in callback[field]):raise ValueError(f'{path}: incomplete alert geometry')
            if not isinstance(callback['text_hex'],str) or len(bytes.fromhex(callback['text_hex']))*2!=len(callback['text_hex']):raise ValueError(f'{path}: invalid dialog item bytes')
    if value['probe']=='owned-68k-v1':
        execution=value['execution']
        if set(execution)!={'result_hex'} or not isinstance(execution['result_hex'],str) or len(execution['result_hex'])!=88 or len(bytes.fromhex(execution['result_hex']))!=44:raise ValueError(f'{path}: incomplete owned instruction results/guards')
    if value['probe']=='addressing-v1':
        addressing=value['addressing'];rows=addressing.get('cases')
        if set(addressing)!={'mmu32bit','cases'} or type(addressing['mmu32bit']) is not int or not isinstance(rows,list) or len(rows)!=12:raise ValueError(f'{path}: incomplete addressing profile')
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!={'input','input_ccr','output','ccr','a0','stack_delta'} or any(type(n) is not int for n in row.values()):raise ValueError(f'{path}: incomplete address ABI')
            if row['input']!=[0,0x123456,0xff123456,0xffffffff][i//3] or row['input_ccr']!=[16,31,0][i%3]:raise ValueError(f'{path}: missing address/flag context')
    if value['probe']=='sound-version-abi-v1':
        sound=value['sound']
        if set(sound)!={'reference_profile_version','cases'} or type(sound['reference_profile_version']) is not int:raise ValueError(f'{path}: incomplete sound profile')
        rows=sound['cases']
        if not isinstance(rows,list) or len(rows)!=3:raise ValueError(f'{path}: incomplete version replay')
        for row in rows:
            if not isinstance(row,dict) or set(row)!={'input_ccr','sp_delta','stack_value','stack_tail','ccr','record_hex'}:raise ValueError(f'{path}: incomplete version return')
            if any(type(row[k]) is not int for k in ['input_ccr','sp_delta','stack_value','ccr']):raise ValueError(f'{path}: invalid version scalar')
            if not isinstance(row['stack_tail'],list) or len(row['stack_tail'])!=2 or any(type(n) is not int for n in row['stack_tail']):raise ValueError(f'{path}: incomplete stack guards')
            if not isinstance(row['record_hex'],str) or len(row['record_hex'])!=32 or len(bytes.fromhex(row['record_hex']))!=16:raise ValueError(f'{path}: incomplete record guards')
    if value['probe']=='catalog-semantics-v1':
        rows=value['files'].get('cases')
        if not isinstance(rows,list) or len(rows)!=28:raise ValueError(f'{path}: incomplete catalog contexts')
        expected={'case','error','io_result','volume_same','ref','version_filler_hex','index_out','attributes','access','object_tag','object_positive','parent_tag','identity_roles','guards','name33_hex','body_hex'}
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or row['case']!=i:raise ValueError(f'{path}: incomplete catalog state')
            if any(type(row[k]) is not int for k in expected-{'ref','identity_roles','version_filler_hex','name33_hex','body_hex'}):raise ValueError(f'{path}: missing catalog scalar')
            for k,n in [('ref',3),('identity_roles',2)]:
                if not isinstance(row[k],list) or len(row[k])!=n or any(type(v) is not int for v in row[k]):raise ValueError(f'{path}: missing catalog identity')
            for k,n in [('version_filler_hex',2),('name33_hex',33),('body_hex',76)]:
                if not isinstance(row[k],str) or len(row[k])!=2*n or len(bytes.fromhex(row[k]))!=n:raise ValueError(f'{path}: missing catalog bytes')
    if value['probe']=='fcb-semantics-v1':
        startup=value['startup']
        if set(startup)!={'global_ref_same','global_name_same'} or any(type(n) is not int for n in startup.values()):raise ValueError(f'{path}: missing application startup state')
        rows=value['files'].get('cases')
        if not isinstance(rows,list) or len(rows)!=18:raise ValueError(f'{path}: incomplete FCB contexts')
        arrays={'volume_out':3,'file_id':2,'fillers':2,'raw_physical':3}
        expected={'case','error','io_result','ref_same','ref_zero','index_out','flags','eof','position','file_volume_same','parent_same','guards','name33_hex'}|set(arrays)
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or row['case']!=i:raise ValueError(f'{path}: incomplete FCB state')
            if any(type(row[k]) is not int for k in expected-set(arrays)-{'name33_hex'}):raise ValueError(f'{path}: missing FCB scalar')
            if not isinstance(row['name33_hex'],str) or len(row['name33_hex'])!=66 or len(bytes.fromhex(row['name33_hex']))!=33:raise ValueError(f'{path}: missing FCB name/guard bytes')
            for k,n in arrays.items():
                if not isinstance(row[k],list) or len(row[k])!=n or any(type(v) is not int for v in row[k]):raise ValueError(f'{path}: missing FCB array')
    if value['probe']=='path-files-pb-semantics-v1':
        rows=value['files'].get('cases')
        if not isinstance(rows,list) or len(rows)!=28:raise ValueError(f'{path}: incomplete native path cases')
        expected={'case','spec_error','spec_volume_same','spec_volume_zero','spec_parent','spec_name_hex','open_error','failed_ref_preserved','read','pb_error','pb_io_result','pb_ref'}
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or row['case']!=i:raise ValueError(f'{path}: incomplete path context')
            if any(type(row[k]) is not int for k in expected-{'spec_name_hex','read'}):raise ValueError(f'{path}: missing path scalar')
            if not isinstance(row['spec_name_hex'],str) or len(row['spec_name_hex'])>126 or len(bytes.fromhex(row['spec_name_hex']))*2!=len(row['spec_name_hex']):raise ValueError(f'{path}: invalid path spelling')
            if not isinstance(row['read'],list) or len(row['read'])!=6 or any(type(n) is not int for n in row['read']):raise ValueError(f'{path}: missing path read bytes')
    if value['probe']=='control-thumb-edges-semantics-v1':
        rows=value['controls'].get('cases')
        if not isinstance(rows,list) or len(rows)!=16:raise ValueError(f'{path}: incomplete indicator edge cases')
        expected={'run','case','rect','initial_value','start','entry_mouse','entry_button','end','result','value','hilite'}
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or any(type(row[k]) is not int for k in expected-{'rect','start','entry_mouse','end'}):raise ValueError(f'{path}: incomplete indicator edge state')
            if row['run']!=i//8 or row['case']!=i%8:raise ValueError(f'{path}: missing indicator edge context')
            for k,width in [('rect',4),('start',2),('entry_mouse',2),('end',2)]:
                if not isinstance(row[k],list) or len(row[k])!=width or any(type(n) is not int for n in row[k]):raise ValueError(f'{path}: missing indicator edge geometry')
    if value['probe']=='control-thumb-semantics-v1':
        rows=value['controls'].get('cases')
        if not isinstance(rows,list) or len(rows)!=6:raise ValueError(f'{path}: incomplete indicator cases')
        expected={'action','start','entry_mouse','entry_button','end','result','value','hilite','first_callback'}
        for i,row in enumerate(rows):
            if not isinstance(row,dict) or set(row)!=expected or any(type(row[k]) is not int for k in ['action','entry_button','result','value','hilite']):raise ValueError(f'{path}: incomplete indicator state')
            if row['action']!=i//3:raise ValueError(f'{path}: missing nil/action indicator contexts')
            for k,width in [('start',2),('entry_mouse',2),('end',2),('first_callback',4 if row['action'] else 0)]:
                if not isinstance(row[k],list) or len(row[k])!=width or any(type(n) is not int for n in row[k]):raise ValueError(f'{path}: missing indicator point/callback')
            if row['start']!=[102,560] or row['end']!=[[122,560],[82,560],[102,560]][i%3]:raise ValueError(f'{path}: unexpected indicator gesture inputs')
    if value['probe']=='control-action-v1':
        rows=value['controls'].get('cases')
        if not isinstance(rows,list) or len(rows)!=8:raise ValueError(f'{path}: incomplete control action cases')
        for row in rows:
            if not isinstance(row,dict) or any(type(row.get(k)) is not int for k in ['control','initial_part','result','value','hilite','port_preserved']):raise ValueError(f'{path}: missing control action results')
            for key,width in [('start',2),('end',2),('callback',5)]:
                data=row.get(key)
                if not isinstance(data,list) or len(data)!=width or any(type(n) is not int for n in data):raise ValueError(f'{path}: incomplete control action observations')
        if not {20,21,22,23}.issubset({r['initial_part'] for r in rows}) or {r['control'] for r in rows}!={0,1}:raise ValueError(f'{path}: missing action parts or nil-action context')
    if value['probe']=='te-double-v1':
        data=value['textedit']; rows=data.get('cases')
        if type(data.get('double_time')) is not int or not isinstance(rows,list) or len(rows)!=96:raise ValueError(f'{path}: incomplete double-click cases')
        for row in rows:
            if not isinstance(row,dict) or any(type(row.get(k)) is not int for k in ['text','mode','styled']):raise ValueError(f'{path}: missing double-click context')
            for key,count,width in [('offsets',1,2),('points',2,2),('states',3,5)]:
                data=[row.get(key)] if key=='offsets' else row.get(key)
                if not isinstance(data,list) or len(data)!=count or any(not isinstance(r,list) or len(r)!=width or any(type(n) is not int for n in r) for r in data):raise ValueError(f'{path}: incomplete double-click observations')
    if value['probe']=='control-hit-v1':
        data=value['controls'];scrollbars=data.get('scrollbars');buttons=data.get('buttons')
        if not isinstance(scrollbars,list) or len(scrollbars)!=11 or not isinstance(buttons,list) or len(buttons)!=3:raise ValueError(f'{path}: incomplete control hit cases')
        for row in scrollbars+buttons:
            points=row.get('points') if isinstance(row,dict) else None
            if not isinstance(points,list) or len(points)<80 or any(not isinstance(p,list) or len(p)!=3 or any(type(n) is not int for n in p) for p in points):raise ValueError(f'{path}: incomplete hit coordinates')
        for mode,row in enumerate(scrollbars):
            if row.get('mode')!=mode or type(row.get('port_preserved')) is not int or len(row['points'])!={9:105,10:221}.get(mode,237):raise ValueError(f'{path}: missing control mode/context observations')
            for key,width in [('rect',4),('range',3)]:
                field=row.get(key)
                if not isinstance(field,list) or len(field)!=width or any(type(n) is not int for n in field):raise ValueError(f'{path}: incomplete control geometry/range')
        for kind,row in enumerate(buttons):
            if row.get('kind')!=kind or len(row['points'])!=88:raise ValueError(f'{path}: missing button inputs')
        if data.get('chain') is None or not isinstance(data['chain'],list) or len(data['chain'])!=3 or any(type(n) is not int for n in data['chain']):raise ValueError(f'{path}: missing control chain observations')
        rows=data.get('find')
        if not isinstance(rows,list) or len(rows)!=5 or any(not isinstance(row,list) or len(row)!=2 or any(type(n) is not int for n in row) for row in rows):raise ValueError(f'{path}: incomplete FindControl observations')
    if value['probe']=='window-tracking-v1':
        rows=value['windows'].get('cases')
        if not isinstance(rows,list) or len(rows)!=6:raise ValueError(f'{path}: incomplete window tracking observations')
        for row in rows:
            if not isinstance(row,dict) or any(type(row.get(k)) is not int for k in ['kind','modifiers','result','port_preserved']):raise ValueError(f'{path}: incomplete window tracking results')
            for key,width in [('start',2),('end',2),('before',4),('after_tracking',4),('content',4)]:
                v=row.get(key)
                if not isinstance(v,list) or len(v)!=width or any(type(n) is not int for n in v):raise ValueError(f'{path}: incomplete tracking geometry')
    if value['probe']=='window-geometry-v1':
        states=value['windows'];expected={'initial','moved','moved_front','shrunk','expanded_no_update','expanded_update','zero_size','moved_dirty'}
        if set(states)!=expected:raise ValueError(f'{path}: incomplete window geometry transitions')
        for state in states.values():
            if not isinstance(state,dict) or any(type(state.get(k)) is not int for k in ['front','current']):raise ValueError(f'{path}: missing window geometry identity')
            for key,width in [('port',4),('bitmap',4),('structure',4),('content',4),('update',4),('visible',4),('local_pixels',2),('update_hits',3)]:
                row=state.get(key)
                if not isinstance(row,list) or len(row)!=width or any(type(n) is not int for n in row):raise ValueError(f'{path}: missing window geometry observations')
    if value['probe']=='style-use-v1':
        rows=value['textedit'].get('cases')
        if not isinstance(rows,list) or len(rows)!=4:raise ValueError(f'{path}: incomplete style import cases')
        for row in rows:
            if not isinstance(row,dict) or not isinstance(row.get('document'),dict) or any(not isinstance(row.get(k),list) or len(row[k])!=2 or any(type(n) is not int for n in row[k]) for k in ['range','handle_state']):raise ValueError(f'{path}: incomplete style import observations')
            if any(k not in row['document'] for k in ['selection','runs','styles','text','heights','null_scrap','line_starts','n_styles','selection_scrap']):raise ValueError(f'{path}: incomplete imported document')
    if value['probe']=='handle-state-v1':
        data=value['memory'];rows=data.get('states')
        if not isinstance(rows,list) or len(rows)!=20 or any(not isinstance(r,list) or len(r)!=8 or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete handle states')
        purge=data.get('purge')
        if not isinstance(purge,list) or len(purge)!=18 or any(not isinstance(r,list) or len(r)!=8 or any(type(n) is not int for n in r) for r in purge):raise ValueError(f'{path}: incomplete purge flag observations')
        for k,width in [('nil_set',4),('nil_get',2)]:
            r=data.get(k)
            if not isinstance(r,list) or len(r)!=width or any(type(n) is not int for n in r):raise ValueError(f'{path}: missing nil-handle observations')
    if value['probe']=='doc-lifecycle-v1':
        rows=value['windows'].get('close')
        if not isinstance(rows,list) or len(rows)!=2 or any(not isinstance(r,list) or len(r)!=7 or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete window ownership observations')
        data=value['dialogs'];opening=data.get('default_open',{})
        if type(data.get('cancelled')) is not int or type(opening.get('count')) is not int or not isinstance(opening.get('port'),list) or len(opening['port'])!=4:raise ValueError(f'{path}: missing default Open context')
        rows=opening.get('items')
        if not isinstance(rows,list) or len(rows)!=9 or len(rows)!=opening['count']:raise ValueError(f'{path}: incomplete default Open items')
        for i,r in enumerate(rows,1):
            if not isinstance(r,dict) or type(r.get('number')) is not int or r['number']!=i or type(r.get('kind')) is not int or not isinstance(r.get('rect'),list) or len(r['rect'])!=4 or not isinstance(r.get('text'),list) or any(type(n) is not int for n in r['rect']+r['text']):raise ValueError(f'{path}: incomplete default Open item')
    if value['probe']=='style-scrap-v1':
        rows=value['textedit'].get('cases')
        if not isinstance(rows,list) or len(rows)!=3:raise ValueError(f'{path}: incomplete style-scrap cases')
        for row in rows:
            if not isinstance(row,dict):raise ValueError(f'{path}: invalid style-scrap case')
            for k in ['selection','after']:
                if not isinstance(row.get(k),list) or len(row[k])!=2 or any(type(n) is not int for n in row[k]):raise ValueError(f'{path}: missing style-scrap selection')
            h=row.get('hex')
            if not isinstance(h,str) or len(h)<4 or len(h)%2 or any(c not in '0123456789abcdef' for c in h):raise ValueError(f'{path}: incomplete style-scrap bytes')
    if value['probe']=='res-io-v1':
        data=value['resources']
        widths={'first_open':3,'empty_before':2,'add':4,'write':4,'second_get1':2,'second_search':2,'repeated_open':3,'app_search':2,'close_other':2,'close_current':2,'read_open':3,'read_info':4,'second_search_current':2,'missing_primed':3,'search_missing_primed':3,'second_open':3,'missing_id':2,'release_reload':3,'detach':6}
        for k,width in widths.items():
            row=data.get(k)
            if not isinstance(row,list) or len(row)!=width or any(type(n) is not int for n in row):raise ValueError(f'{path}: incomplete stored-resource observations {k}')
        for k in ['second_create_current_same','repeated_before_is_second','repeated_current_is_original']:
            if type(data.get(k)) is not int:raise ValueError(f'{path}: missing resource identity observation')
        for k in ['name_hex','data_hex']:
            h=data.get(k)
            if not isinstance(h,str) or len(h)%2 or any(c not in '0123456789abcdef' for c in h):raise ValueError(f'{path}: missing resource bytes')
        rows=data.get('failed_open')
        if not isinstance(rows,list) or len(rows)!=3 or any(not isinstance(row,list) or len(row)!=2 or any(type(n) is not int for n in row) for row in rows):raise ValueError(f'{path}: missing failed resource opens')
    if value['probe']=='res-create-v1':
        data=value['resources'];rows=data.get('cases')
        if not isinstance(rows,list) or len(rows)!=4:raise ValueError(f'{path}: incomplete resource-file creation cases')
        for row in rows:
            if not isinstance(row,dict) or any(type(row.get(k)) is not int for k in ['error','current_same']):raise ValueError(f'{path}: invalid resource creation result')
            for k in ['fork_errors','finder']:
                if not isinstance(row.get(k),list) or len(row[k])!=2 or any(type(n) is not int for n in row[k]):raise ValueError(f'{path}: incomplete resource creation metadata')
            for k in ['data_hex','resource_hex']:
                h=row.get(k)
                if not isinstance(h,str) or len(h)%2 or any(c not in '0123456789abcdef' for c in h):raise ValueError(f'{path}: incomplete resource creation fork bytes')
        if not isinstance(data.get('invalid'),list) or len(data['invalid'])!=2 or any(type(n) is not int for n in data['invalid']):raise ValueError(f'{path}: missing resource creation errors')
    if value['probe']=='icon-plot-v1':
        data=value['icons'];errors=data.get('errors');pixels=data.get('bitmap_hex')
        if not isinstance(errors,list) or len(errors)!=3 or any(type(n) is not int for n in errors) or not isinstance(pixels,str) or len(pixels)!=1024 or any(c not in '0123456789abcdef' for c in pixels):raise ValueError(f'{path}: incomplete monochrome icon observations')
    if value['probe']=='user-items-v1':
        data=value['dialogs']
        for key,length,width in [('calls',3,9),('caller',1,5),('dialog',1,6)]:
            rows=data.get(key);rows=rows if key=='calls' else [rows]
            if not isinstance(rows,list) or len(rows)!=length or any(not isinstance(r,list) or len(r)!=width or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete user-item drawing observations')
    if value['probe']=='delay-v1':
        rows=value['timing'].get('cases')
        if not isinstance(rows,list) or len(rows)!=3 or any(not isinstance(r,list) or len(r)!=5 or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete Delay observations')
    if value['probe']=='modal-filter-v1':
        data=value['dialogs'];rows=data.get('cases')
        if not isinstance(rows,list) or len(rows)!=3:raise ValueError(f'{path}: incomplete modal-filter cases')
        for mode,row in enumerate(rows):
            if not isinstance(row,dict) or row.get('mode')!=mode or any(type(row.get(k)) is not int for k in ['good','null_calls','idle_hooks']) or not isinstance(row.get('name'),list) or any(type(n) is not int or not 0<=n<=255 for n in row['name']):raise ValueError(f'{path}: invalid modal-filter observations')
            if not isinstance(row.get('calls'),list) or not row['calls'] or any(not isinstance(r,list) or len(r)!=4 or any(type(n) is not int for n in r) for r in row['calls']):raise ValueError(f'{path}: missing callback-order observations')
        for phase in ['initial','changed','restored']:
            row=data.get('item_changes',{}).get(phase,{})
            if any(type(row.get(k)) is not int for k in ['kind','handle_matches','memory_matches']) or not isinstance(row.get('rect'),list) or len(row['rect'])!=4 or any(type(n) is not int for n in row['rect']):raise ValueError(f'{path}: incomplete dialog-item mutation state')
    if value['probe']=='standard-filter-v1':
        data=value['dialogs'];rows=data.get('cases')
        if any(type(data.get(k)) is not int for k in ['error','nonnull']) or not isinstance(rows,list) or len(rows)!=14 or any(not isinstance(r,list) or len(r)!=5 or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete callable standard-filter observations')
        errors=data.get('setter_errors')
        if not isinstance(errors,list) or len(errors)!=3 or any(type(n) is not int for n in errors):raise ValueError(f'{path}: missing dialog setter result codes')
    if value['probe']=='font-globals-v1':
        row=value['fonts'].get('globals')
        if not isinstance(row,list) or len(row)!=3 or any(type(n) is not int for n in row):raise ValueError(f'{path}: incomplete font globals')
    if value['probe']=='script-context-v1':
        data=value['scripts']
        for key,length,width in [('context',3,2),('properties',6,3)]:
            rows=data.get(key)
            if not isinstance(rows,list) or len(rows)!=length or any(not isinstance(r,list) or len(r)!=width or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete Roman script context')
    if value['probe']=='resource-enum-v1':
        data=value['resources']
        for key in ['count','absent','system_count']:
            r=data.get(key)
            if not isinstance(r,list) or len(r)!=2 or any(type(n) is not int for n in r):raise ValueError(f'{path}: missing resource count/error')
        items=data.get('items')
        if not isinstance(items,list) or len(items)!=6:raise ValueError(f'{path}: incomplete resource enumeration')
        for row,index in zip(items,[0,1,2,3,4,-1]):
            if not isinstance(row,dict) or row.get('index')!=index or any(type(row.get(k)) is not int for k in ['nonnull','error']):raise ValueError(f'{path}: invalid indexed resource result')
            if row['nonnull']:
                if any(type(row.get(k)) is not int for k in ['id','type','info_error','same_handle','home_is_current']) or any(not isinstance(row.get(k),list) or any(type(n) is not int or not 0<=n<=255 for n in row[k]) for k in ['name','bytes']):raise ValueError(f'{path}: incomplete resource identity/data')
    if value['probe']=='string-compare-v1':
        rows=value['strings'].get('cases')
        if not isinstance(rows,list) or len(rows)!=6 or any(not isinstance(r,list) or len(r)!=4 or any(not isinstance(s,list) or len(s)!=3 or any(type(n) is not int for n in s) for s in r) for r in rows):raise ValueError(f'{path}: incomplete string comparison matrix')
    if value['probe']=='font-availability-v1':
        rows=value['fonts'].get('queries')
        if not isinstance(rows,list) or len(rows)!=8 or any(not isinstance(r,list) or len(r)!=3 or any(type(n) is not int for n in r) for r in rows):raise ValueError(f'{path}: incomplete font availability queries')
    if value['probe']=='te-outline-refresh-v1':
        for name in ['plain','styled']:
            row=value['textedit'].get(name,{})
            if type(row.get('feature')) is not int or row.get('selection')!=[1,3] or not isinstance(row.get('sel_rect'),list) or len(row['sel_rect'])!=4 or any(type(n) is not int for n in row['sel_rect']):raise ValueError(f'{path}: incomplete selection-outline record')
            for phase in ['active','inactive','background']:
                mask=row.get(phase,{})
                rows=mask.get('rows')
                if not isinstance(rows,list) or len(rows)!=16 or any(not isinstance(r,str) or len(r)!=30 or set(r)-{'0','1'} for r in rows) or type(mask.get('count')) is not int or mask['count']!=sum(r.count('1') for r in rows):raise ValueError(f'{path}: invalid selection-outline mask')
                ys=[(y+28,x+20) for y,r in enumerate(rows) for x,bit in enumerate(r) if bit=='1']
                expected=[min(y for y,x in ys),min(x for y,x in ys),max(y for y,x in ys)+1,max(x for y,x in ys)+1] if ys else [32767,32767,-32768,-32768]
                if mask.get('bounds')!=expected:raise ValueError(f'{path}: inconsistent selection-outline bounds')
    if value['probe']=='te-arrows-v1':
        def arrow_state(row):return isinstance(row,list) and len(row)==7 and all(type(n) is int for n in row)
        for name in ['plain','styled']:
            row=value['textedit'].get(name,{})
            if any(not arrow_state(row.get(key)) for key in ['empty_down','empty_up','selection_up','selection_down']) or not isinstance(row.get('moves'),list) or len(row['moves'])!=6 or any(not arrow_state(r) for r in row['moves']):raise ValueError(f'{path}: incomplete vertical-key observations')
    if value['probe']=='te-continuous-control-v1':
        for name in ['initial','pending','mixed','shared_face','boundary_caret','no_common_face','plain']:
            row=value['textedit'].get(name+'_raw',{})
            if any(not isinstance(row.get(k),list) or not row[k] for k in ['runs','styles','null']):raise ValueError(f'{path}: missing raw style diagnostic')
            for key,width in [('runs',2),('styles',3)]:
                if any(not isinstance(r,list) or len(r)!=width or any(type(n) is not int for n in r) for r in row[key]):raise ValueError(f'{path}: malformed raw style diagnostic')
            if row['null'][0] not in [0,1] or len(row['null'])!=(4 if row['null'][0] else 1) or any(type(n) is not int for n in row['null']):raise ValueError(f'{path}: malformed null-style diagnostic')
    if value['probe'] in ['te-continuous-v1','te-continuous-control-v1']:
        for name in ['initial','pending','mixed','shared_face','boundary_caret','no_common_face','plain']:
            rows=value['textedit'].get(name)
            if not isinstance(rows,list) or len(rows)!=6:raise ValueError(f'{path}: incomplete continuous-style queries')
            for row,requested in zip(rows,[0,1,2,4,8,15]):
                if not isinstance(row,dict) or row.get('requested')!=requested or type(row.get('mode')) is not int or row['mode']&~requested or type(row.get('result_byte')) is not int or not 0<=row['result_byte']<=255:raise ValueError(f'{path}: invalid continuous-style result')
                attrs=row.get('attributes')
                if not isinstance(attrs,list) or len(attrs)!=6 or any((type(n) is not int) if row['mode']&bit else n is not None for n,bit in zip(attrs,[1,2,4,8,8,8])):raise ValueError(f'{path}: invalid continuous-style attributes')
        for name in ['height_empty','height_mixed','height_cr','height_two']:
            rows=value['textedit'].get(name)
            if not isinstance(rows,list) or len(rows)!=10 or any(not isinstance(row,list) or len(row)!=3 or any(type(n) is not int for n in row) for row in rows):raise ValueError(f'{path}: incomplete height bounds queries')
    if value['probe']=='te-auto-view-v1':
        def auto_state(row):
            return isinstance(row,dict) and all(type(row.get(key)) is int for key in ['auto','click_flags','cr_only','n_lines']) and all(isinstance(row.get(key),list) and len(row[key])==count and all(type(n) is int for n in row[key]) for key,count in [('selection',2),('dest',4),('view',4)])
        for name in ['plain','styled']:
            rows=value['textedit'].get(name,{})
            for key in ['initial','enabled','disabled','set','selection_view','scrolled','pinned_high','pinned_low']:
                if not auto_state(rows.get(key)):raise ValueError(f'{path}: incomplete automatic-scroll state {name}/{key}')
            if any(type(rows.get(key)) is not int for key in ['set_result','clear_result']) or not isinstance(rows.get('typing'),list) or len(rows['typing'])!=5 or any(not auto_state(row) for row in rows['typing']):raise ValueError(f'{path}: incomplete auto-scroll typing/feature observations')
    if value['probe']=='style-queries-v1':
        for name in ['initial','pending','typed','mixed','recalculated']:
            row=value['textedit'].get(name)
            if not isinstance(row,dict) or not isinstance(row.get('heights'),list) or len(row['heights']) not in [4,6] or any(not isinstance(r,list) or len(r)!=3 or any(type(n) is not int for n in r) for r in row['heights']):raise ValueError(f'{path}: missing styled height queries {name}')
            if not isinstance(row.get('styles'),list) or len(row['styles'])!=3:raise ValueError(f'{path}: missing style/point queries {name}')
            for entry in row['styles']:
                if not isinstance(entry,dict) or type(entry.get('offset')) is not int or any(not isinstance(entry.get(key),list) or len(entry[key])!=count or any(type(n) is not int for n in entry[key]) for key,count in [('style',6),('metrics',2),('point',2)]):raise ValueError(f'{path}: invalid style/point query {name}')
    if value['probe']=='styled-text-v1':
        for name in ['initial','insertion_style','typed','mixed','deleted','inserted','set_text']:
            row=value['textedit'].get(name)
            if not isinstance(row,dict) or any(type(row.get(key)) is not int for key in ['length','line_height','font_ascent','n_lines','n_runs','n_styles','refcon']):raise ValueError(f'{path}: incomplete styled TextEdit state {name}')
            for key,count,width in [('selection',2,None),('line_starts',row['n_lines']+1,None),('runs',row['n_runs']+1,2),('styles',row['n_styles'],9),('heights',max(1,row['n_lines']),2)]:
                entries=row.get(key)
                if not isinstance(entries,list) or len(entries)!=count:raise ValueError(f'{path}: incomplete style table {name}/{key}')
                if width is None:
                    if any(type(n) is not int for n in entries):raise ValueError(f'{path}: invalid style field {name}/{key}')
                elif any(not isinstance(entry,list) or len(entry)!=width or any(type(n) is not int for n in entry) for entry in entries):raise ValueError(f'{path}: invalid style table {name}/{key}')
            for key in ['null_scrap','selection_scrap']:
                if not isinstance(row.get(key),list) or any(not isinstance(entry,list) or len(entry)!=9 or any(type(n) is not int for n in entry) for entry in row[key]):raise ValueError(f'{path}: invalid styled scrap {name}/{key}')
            if not isinstance(row.get('text'),list) or len(row['text'])!=row['length'] or any(type(b) is not int or not 0<=b<=255 for b in row['text']) or not isinstance(row.get('style_pointer_offsets'),list) or any(type(v) is not int or not 0<=v<=94 for v in row['style_pointer_offsets']) or not isinstance(row.get('scrap_identity'),list) or len(row['scrap_identity'])!=3 or any(type(v) is not int or v not in [0,1] for v in row['scrap_identity']):raise ValueError(f'{path}: invalid styled record identity/text {name}')
    if value['probe'] in ['window-resources-v1','window-resource-flow-v1']:
        windows=value['windows']
        def rectangle(row):return isinstance(row,list) and len(row)==4 and all(type(n) is int for n in row)
        if value['probe']=='window-resources-v1':
            for name in ['mono','color','missing_mono','missing_color']:
                row=windows.get(name)
                if not isinstance(row,dict) or type(row.get('nonnull')) is not int or type(row.get('resource_error')) is not int:raise ValueError(f'{path}: missing resource-window outcome {name}')
                if row['nonnull']:
                    if any(type(row.get(key)) is not int for key in ['color','visible','kind','go_away','refcon','current']) or any(not rectangle(row.get(key)) for key in ['port_rect','content','structure']) or not isinstance(row.get('title'),list) or any(type(b) is not int or not 0<=b<=255 for b in row['title']):raise ValueError(f'{path}: invalid resource-window state {name}')
        else:
            for name in ['mono','color']:
                rows=windows.get(name,{})
                if not isinstance(rows.get('title'),list) or any(type(b) is not int or not 0<=b<=255 for b in rows['title']):raise ValueError(f'{path}: missing window-title observation')
                for key in ['initial','changed','moved','resized','shown','hidden','reshown']:
                    row=rows.get(key)
                    if not isinstance(row,dict) or type(row.get('visible')) is not int or type(row.get('refcon')) is not int or any(not rectangle(row.get(field)) for field in ['port_rect','content','structure','update','vis']):raise ValueError(f'{path}: incomplete window lifecycle {name}/{key}')
    if value['probe']=='apple-event-handlers-v1':
        handlers=value['handlers']
        for key in ['missing','installed','replaced','after_wrong','removed','nil_lookup','system_lookup','application_lookup','system_removed']:
            row=handlers.get(key)
            if not isinstance(row,dict) or type(row.get('error')) is not int or type(row.get('refcon')) is not int or not isinstance(row.get('handler'),list) or len(row['handler'])!=4 or any(type(v) is not int or v not in [0,1] for v in row['handler']):raise ValueError(f'{path}: incomplete handler lookup {key}')
        for key in ['install','replace','remove_wrong','remove','remove_again','nil_install','nil_remove','system_install','system_remove']:
            if type(handlers.get(key)) is not int:raise ValueError(f'{path}: missing handler operation {key}')
    if value['probe']=='menu-items-v1':
        menus=value['menus']
        def item_state(row):
            return isinstance(row,dict) and type(row.get('count')) is int and isinstance(row.get('values'),list) and len(row['values'])==4 and all(type(n) is int for n in row['values']) and isinstance(row.get('text'),list) and len(row['text'])<=255 and all(type(n) is int and 0<=n<=255 for n in row['text'])
        for key in ['initial','checked','unchecked','renamed','invalid_zero','invalid_high']:
            if not item_state(menus.get(key)):raise ValueError(f'{path}: missing menu-item state {key}')
        if not isinstance(menus.get('set'),list) or len(menus['set'])!=6 or any(not item_state(row) for row in menus['set']):
            raise ValueError(f'{path}: incomplete menu byte-boundary observations')
        for key,count in [('hierarchy_inserted',3),('hierarchy_deleted',2),('hierarchy_restored',2)]:
            if not isinstance(menus.get(key),list) or len(menus[key])!=count or any(type(n) is not int for n in menus[key]):raise ValueError(f'{path}: incomplete hierarchy observation {key}')
    if value['probe']=='menu-semantics-v1':
        menus=value['menus'];geometry=menus.get('geometry')
        if not isinstance(geometry,list) or len(geometry)!=6 or any(not isinstance(row,list) or len(row)!=2 or any(type(n) is not int for n in row) for row in geometry):raise ValueError(f'{path}: incomplete menu invalidation observations')
        for key in ['unlinked','linked','parent_item_disabled','parent_menu_disabled','submenu_disabled','submenu_item_disabled','unlinked_again']:
            if type(menus.get(key)) is not int:raise ValueError(f'{path}: missing submenu lookup observation {key}')
    if value['probe']=='font-profile-v1':
        fonts=value['fonts']
        def byte_string(v):
            return isinstance(v,list) and len(v)<=255 and all(type(b) is int and 0<=b<=255 for b in v)
        names=fonts.get('names');queries=fonts.get('queries');menus=fonts.get('menus')
        if not isinstance(names,list) or len(names)!=7 or any(not isinstance(r,dict) or type(r.get('id')) is not int or not byte_string(r.get('name')) for r in names):
            raise ValueError(f'{path}: incomplete font aliases')
        if [r['id'] for r in names]!=[0,1,3,4,2002,32767,-1]:
            raise ValueError(f'{path}: invalid font alias inputs')
        if not isinstance(queries,list) or len(queries)!=7 or any(not isinstance(r,dict) or not byte_string(r.get('name')) or type(r.get('number')) is not int for r in queries):
            raise ValueError(f'{path}: incomplete font-number queries')
        if not isinstance(menus,list) or len(menus)!=2:
            raise ValueError(f'{path}: missing FONT/FOND observations')
        for menu in menus:
            if not isinstance(menu,dict) or type(menu.get('kind')) is not int or not byte_string(menu.get('prefix')) or not isinstance(menu.get('counts'),list) or len(menu['counts'])!=3 or any(type(v) is not int or v<0 for v in menu['counts']) or not isinstance(menu.get('known_order'),list) or any(not byte_string(v) for v in menu['known_order']):
                raise ValueError(f'{path}: invalid font-menu observations')
    if value['probe']=='system-styles-v1':
        records=value['styles'].get('calibration');labels=value['styles'].get('labels')
        if not isinstance(records,list) or len(records)!=128 or not isinstance(labels,list) or len(labels)!=128:
            raise ValueError(f'{path}: missing all128 system-font styles')
        for face,record in enumerate(records):
            if not isinstance(record,dict) or record.get('face')!=face or not isinstance(record.get('info'),list) or len(record['info'])!=4 or not isinstance(record.get('widths'),list) or len(record['widths'])!=256 or any(type(v) is not int for v in record['info']+record['widths']):
                raise ValueError(f'{path}: invalid system-font style metrics')
        if any(not isinstance(row,list) or len(row)!=8 or any(type(v) is not int for v in row) for row in labels):
            raise ValueError(f'{path}: invalid held-out system-font label widths')
    if value['probe']=='gestalt-v1':
        rows=value.get('gestalt')
        selectors=[int.from_bytes(s,'big') for s in [b'sysv',b'qd  ',b'proc',b'fpu ',b'zZ9!']]
        if not isinstance(rows,list) or len(rows)!=len(selectors) or any(not isinstance(r,list) or len(r)!=4 or any(type(v) is not int for v in r) for r in rows) or [r[0] for r in rows]!=selectors:
            raise ValueError(f'{path}: missing or invalid Gestalt register observations')
    if value['probe']=='title-bounds-v1':
        for key in ['advances','single','prefix_m','suffix_m','prefix_r','suffix_r','leading_space','trailing_space']:
            if not isinstance(value.get(key),list) or len(value[key])!=256:
                raise ValueError(f'{path}: invalid title-bound field {key}')
        if not isinstance(value.get('heldout'),list) or len(value['heldout'])!=128:
            raise ValueError(f'{path}: missing held-out title inputs')
    if value['probe']=='title-pairs-v1':
        pairs=value.get('pairs')
        if not isinstance(pairs,list) or len(pairs)!=256 or any(not isinstance(row,list) or len(row)!=256 for row in pairs):
            raise ValueError(f'{path}: expected all 65536 byte pairs')
        if value.get('complete') is not True or not isinstance(value.get('long_cases'),list) or len(value['long_cases'])!=32:
            raise ValueError(f'{path}: incomplete title-pair observation')
    if value['probe']=='save-confirm-v1':
        # Modal idle callback count depends on elapsed time and pending system
        # events. Preserve every lifecycle transition and at least one idle
        # callback in each run; collapse only adjacent identical idle calls.
        for case in ['new','replace_accept','replace_cancel']:
            hook=value['save'].get(case,{}).get('hook')
            if not isinstance(hook,list) or not hook:
                raise ValueError(f'{path}: missing Standard File hook lifecycle')
            normalized=[]
            for entry in hook:
                if not isinstance(entry,list) or len(entry)!=2 or any(type(v) is not int for v in entry):
                    raise ValueError(f'{path}: invalid Standard File hook observation')
                if entry[1]==100 and normalized and normalized[-1]==entry:continue
                normalized.append(entry)
            value['save'][case]['hook']=normalized
    return value

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference')
    parser.add_argument('candidate')
    args = parser.parse_args()
    reference=read_probe(args.reference);candidate=read_probe(args.candidate)
    result = differences(reference,candidate)
    report={'equal': not result, 'differences': result}
    if reference['probe']=='sane-cos-semantics-v1':
        from cossane_observations import statistics
        report.update(statistics(reference,candidate))
    if reference['probe']=='sane-tan-semantics-v1':
        from tansane_observations import statistics
        report.update(statistics(reference,candidate))
    if reference['probe']=='sane-sine-semantics-v1':
        from sinesane_observations import statistics
        report.update(statistics(reference,candidate))
    if reference['probe']=='sane-semantics-v1':
        from sane_observations import statistics
        report.update(statistics(reference,candidate))
    if reference['probe']=='control-thumb-semantics-v1':report['scope']='All6 fixed gesture results, points, entry button, value/hilite and first no-argument callback state. Raw native durations, repeat counts and later samples remain separate diagnostics.'
    if reference['probe']=='save-confirm-v1':report['normalization']='Only consecutive identical idle hook100 callbacks are collapsed; raw captures remain unchanged.'
    print(json.dumps(report, indent=2))
    return bool(result)

if __name__ == '__main__':
    raise SystemExit(main())
