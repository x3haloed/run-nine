//! Owned 68K filters execute through the application's ModalDialog continuation.
use crate::{
    callbacks::Callbacks,
    controls::Controls,
    dialogs::Dialogs,
    events::{Event, Events},
    fonts::Fonts,
    machine::Memory,
    managers::Heap,
    modal::ModalDialog,
    quickdraw::QuickDraw,
    resource,
    textedit::TextEdit,
    windows::Windows,
};
use m68k::{AddressBus, CpuCore, CpuType, StepResult};
use serde_json::json;
use std::collections::VecDeque;
pub fn probe() -> Result<String, String> {
    let bytes = std::fs::read("work/probes/modaldialog/ModalDialogProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = resource::parse(&bytes)?;
    let code =
        std::fs::read("work/probes/modaldialog/runtime-filter.bin").map_err(|e| e.to_string())?;
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let mut controls = Controls::new();
    let mut dialogs = Dialogs::new();
    let mut te = TextEdit::new();
    let mut modal = ModalDialog::default();
    let mut callbacks = Callbacks::silent();
    let mut script = VecDeque::new();
    let original = q.get_port(&mut m)?;
    m.put(0x19000, &code)?;
    let mut records = Vec::new();
    for mode in 0..8 {
        for seed in [0x12345678u32, 0] {
            for flags in [16, 31, 0] {
                let mut cpu = CpuCore::new();
                cpu.set_cpu_type(CpuType::M68020);
                let sp = 0x700000;
                m.write_long(sp, u32::MAX);
                m.write_long(sp + 4, 0);
                m.write_word(sp + 8, 128);
                cpu.pc = 0x6000;
                cpu.dar[15] = sp;
                dialogs.dispatch_lifecycle(
                    0xa97c,
                    &mut cpu,
                    &mut m,
                    &mut heap,
                    &mut q,
                    &fonts,
                    &mut windows,
                    &mut controls,
                    &mut events,
                    &mut te,
                    &resources,
                )?;
                let dialog = m.read_long(sp + 10);
                assert_ne!(dialog, 0);
                q.set_port(&mut m, dialog)?;
                windows.begin_update(&mut m, &mut heap, &q, dialog)?;
                dialogs.draw(&mut m, &mut heap, &q, &fonts, &controls, dialog)?;
                windows.end_update(&mut m, &mut heap, dialog)?;
                while windows.next_event(&mut m, &mut events, 0xffff, true)?.0 {}
                q.set_port(&mut m, original)?;
                // The first native invocation receives a deferred process activation;
                // reproduce that event-queue input, not a special case in ModalDialog.
                if records.is_empty() {
                    events
                        .post(Event {
                            what: 8,
                            message: dialog,
                            modifiers: 1,
                            ..Event::default()
                        })
                        .map_err(|e| e.to_string())?;
                }
                m.write_word(0x110000, mode);
                m.write_word(0x110002, 0);
                m.write_long(0x110004, dialog);
                m.write_long(0x110008, 0x111004);
                m.write_long(0x111000, 0xa1b2c3d4);
                m.write_word(0x111004, (-12345i16) as u16);
                m.write_long(0x111006, 0x10203040);
                m.write_long(sp, 0x111004);
                m.write_long(sp + 4, 0x19000);
                m.write_long(sp + 8, 0x55667788);
                m.write_word(0x220, (-123i16) as u16);
                m.write_word(0xa60, (-192i16) as u16);
                cpu.pc = 0x6000;
                cpu.dar[15] = sp;
                cpu.dar[0] = seed;
                cpu.set_ccr(flags);
                let mut done = false;
                for _ in 0..20000 {
                    if let Some((token, result)) = callbacks.finish(&mut cpu, &mut m)? {
                        if token == 9 {
                            modal.callback_result(result)?;
                        } else if token == 10 {
                            modal.track_result(result)?;
                        } else {
                            return Err("unexpected modal probe callback".into());
                        }
                        continue;
                    }
                    if cpu.pc == 0x6002 {
                        done = true;
                        break;
                    }
                    if cpu.pc == 0x6000 {
                        assert!(modal.dispatch(
                            0xa991,
                            &mut cpu,
                            &mut m,
                            &mut heap,
                            &q,
                            &fonts,
                            &mut windows,
                            &controls,
                            &mut events,
                            &mut dialogs,
                            &mut callbacks,
                            &mut script
                        )?);
                        continue;
                    }
                    match cpu.step(&mut m) {
                        StepResult::Ok { .. } => {}
                        other => return Err(format!("owned modal filter stopped: {other:?}")),
                    }
                }
                if !done {
                    return Err("ModalDialog did not complete owned filter execution".into());
                }
                let n = m.read_word(0x110002);
                if n > 32 {
                    return Err("modal filter overran observation capacity".into());
                }
                let calls: Vec<Vec<i16>> = (0..n)
                    .map(|i| {
                        (0..8)
                            .map(|j| m.read_word(0x110100 + u32::from(i) * 16 + j * 2) as i16)
                            .collect()
                    })
                    .collect();
                records.push(json!({"mode":mode,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(sp+8),"guard":m.read_long(sp+8),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"port_preserved":i32::from(q.get_port(&mut m)?==original),"item":m.read_word(0x111004)as i16,"prefix":m.read_long(0x111000),"suffix":m.read_long(0x111006),"calls":calls}));
                m.write_long(sp, dialog);
                cpu.pc = 0x6000;
                cpu.dar[15] = sp;
                dialogs.dispatch_lifecycle(
                    0xa983,
                    &mut cpu,
                    &mut m,
                    &mut heap,
                    &mut q,
                    &fonts,
                    &mut windows,
                    &mut controls,
                    &mut events,
                    &mut te,
                    &resources,
                )?;
                assert_eq!(windows.front(), 0);
                assert!(te.documents(&mut m)?["documents"]
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
        }
    }
    // Owned integration checks compose the independently measured standard
    // filter and TrackControl paths. These are not additional native cases.
    for input in 0..2 {
        let free = heap.free_bytes();
        let sp = 0x700000;
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(CpuType::M68020);
        m.write_long(sp, u32::MAX);
        m.write_long(sp + 4, 0);
        m.write_word(sp + 8, 128);
        cpu.pc = 0x6000;
        cpu.dar[15] = sp;
        dialogs.dispatch_lifecycle(
            0xa97c,
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            &fonts,
            &mut windows,
            &mut controls,
            &mut events,
            &mut te,
            &resources,
        )?;
        let dialog = m.read_long(sp + 10);
        assert_ne!(dialog, 0);
        windows.begin_update(&mut m, &mut heap, &q, dialog)?;
        dialogs.draw(&mut m, &mut heap, &q, &fonts, &controls, dialog)?;
        windows.end_update(&mut m, &mut heap, dialog)?;
        while windows.next_event(&mut m, &mut events, 0xffff, true)?.0 {}
        q.set_port(&mut m, original)?;
        if input == 0 {
            events
                .post(Event {
                    what: 3,
                    message: 13,
                    modifiers: 0x80,
                    ..Event::default()
                })
                .map_err(|e| e.to_string())?;
        } else {
            events.set_mouse(&mut m, (95, 115));
            events.modifiers = 0x80;
            events
                .post(Event {
                    what: 1,
                    v: 95,
                    h: 115,
                    modifiers: 0,
                    ..Event::default()
                })
                .map_err(|e| e.to_string())?;
        }
        m.write_long(sp, 0x111004);
        m.write_long(sp + 4, 0);
        m.write_long(sp + 8, 0x55667788);
        m.write_word(0x111004, 0x7b7b);
        cpu.pc = 0x6000;
        cpu.dar[15] = sp;
        let mut done = false;
        for _ in 0..20000 {
            if let Some((token, result)) = callbacks.finish(&mut cpu, &mut m)? {
                if token == 9 {
                    modal.callback_result(result)?;
                } else if token == 10 {
                    modal.track_result(result)?;
                } else {
                    return Err("unexpected modal integration callback".into());
                }
                continue;
            }
            if cpu.pc == 0x6002 {
                done = true;
                break;
            }
            if cpu.pc == 0x6000 {
                modal.dispatch(
                    0xa991,
                    &mut cpu,
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    &mut windows,
                    &controls,
                    &mut events,
                    &mut dialogs,
                    &mut callbacks,
                    &mut script,
                )?;
                continue;
            }
            let op = m.read_word(cpu.pc);
            if op == 0xa968 {
                controls.track(&mut cpu, &mut m, &q, &events, &mut callbacks)?;
                continue;
            }
            if dialogs.dispatch_filter(op, &mut cpu, &mut m, &mut heap, &q, &fonts, &controls)? {
                continue;
            }
            match cpu.step(&mut m) {
                StepResult::Ok { .. } => {}
                other => return Err(format!("modal integration stopped: {other:?}")),
            }
        }
        assert!(done);
        assert_eq!(cpu.dar[15], sp + 8);
        assert_eq!(m.read_long(sp + 8), 0x55667788);
        assert_eq!(m.read_word(0x111004), if input == 0 { 1 } else { 2 });
        let check = m.read_long(dialogs.records[&dialog].items[1].handle);
        assert_eq!(
            m.read_word(check + 18),
            0,
            "tracking must not invent a checkbox value change"
        );
        m.write_long(sp, dialog);
        cpu.pc = 0x6000;
        cpu.dar[15] = sp;
        dialogs.dispatch_lifecycle(
            0xa983,
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            &fonts,
            &mut windows,
            &mut controls,
            &mut events,
            &mut te,
            &resources,
        )?;
        assert_eq!(heap.free_bytes(), free);
        assert_eq!(q.get_port(&mut m)?, original);
    }
    Ok(json!({"schema":1,"probe":"modal-dialog-runtime-raw-v1","records":records}).to_string())
}
