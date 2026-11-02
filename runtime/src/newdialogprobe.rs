//! Native-shaped resource dialog creation through the application dispatcher.
use crate::{
    controls::Controls,
    dialogs::Dialogs,
    events::Events,
    fonts::Fonts,
    machine::Memory,
    managers::Heap,
    quickdraw::{self, QuickDraw},
    resource,
    textedit::TextEdit,
    windows::Windows,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
fn rect(m: &mut Memory, p: u32) -> [i16; 4] {
    let r = quickdraw::read_rect(m, p);
    [r.top, r.left, r.bottom, r.right]
}
pub fn probe() -> Result<String, String> {
    let bytes =
        std::fs::read("work/probes/newdialogdetails/NewDialogDetailsProbe.APPL/..namedfork/rsrc")
            .map_err(|e| e.to_string())?;
    let resources = resource::parse(&bytes)?;
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
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
    let original = q.get_port(&mut m)?;
    let mut records = Vec::new();
    let mut cpu = CpuCore::new();
    for (which, id) in [128, 129, 32760, 130, 131, 132, 133, 134]
        .into_iter()
        .enumerate()
    {
        let mut settled_free = None;
        for storage in 0..2 {
            for seed in [0x12345678u32, 0] {
                for flags in [16, 31, 0] {
                    q.set_port(&mut m, original)?;
                    let front = windows.front();
                    let buffer = if storage != 0 {
                        let p = heap
                            .new_ptr(&mut m, 178, false)
                            .map_err(|e| e.to_string())?;
                        for i in 0..178 {
                            m.write_byte(p + i, 0xa5);
                        }
                        m.write_long(p, 0xa1b2c3d4);
                        m.write_long(p + 174, 0x10203040);
                        p
                    } else {
                        0
                    };
                    let sp = 0x7e0000;
                    let destination = if storage != 0 { buffer + 4 } else { 0 };
                    m.write_long(sp, if which == 1 { 0 } else { u32::MAX });
                    m.write_long(sp + 4, destination);
                    m.write_word(sp + 8, id);
                    m.write_long(sp + 10, 0x7b7b7b7b);
                    m.write_long(sp + 14, 0x55667788);
                    m.write_word(0x220, (-123i16) as u16);
                    m.write_word(0xa60, (-192i16) as u16);
                    cpu.pc = 0x19000;
                    cpu.dar[15] = sp;
                    cpu.dar[0] = seed;
                    cpu.set_ccr(flags);
                    assert!(dialogs.dispatch_lifecycle(
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
                        &resources
                    )?);
                    assert_eq!(cpu.pc, 0x19002);
                    let d = m.read_long(sp + 10);
                    let mut row = json!({"which":which,"storage":storage,"input_d0":seed,"input_ccr":flags,"result_address":d,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(sp+10),"guard":m.read_long(sp+14),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"current_port_preserved":i32::from(q.get_port(&mut m)?==original),"front_changed":i32::from(windows.front()!=front),"dst_reused":i32::from(d!=0&&storage!=0&&d==destination),"prefix":if storage!=0{m.read_long(buffer)}else{0},"suffix":if storage!=0{m.read_long(buffer+174)}else{0}});
                    if d != 0 {
                        let title = m.read_long(d + 134);
                        let p = m.read_long(title);
                        let n = u32::from(m.read_byte(p));
                        let title = hex(&mut m, p + 1, n);
                        let text = m.read_long(d + 160);
                        let tp = m.read_long(text);
                        let bm = q.port_bitmap(&mut m, d)?;
                        let bounds = rect(&mut m, bm + 6);
                        let items=dialogs.records[&d].items.iter().map(|item|{
                    let basic=item.kind&127;let p=m.read_long(item.handle);
                    let (first,n)=if matches!(basic,4|5|6){(p+41,u32::from(m.read_byte(p+40)))}else{(p,heap.size(item.handle).map_err(|e|e.to_string())?)};
                    Ok(json!({"kind":item.kind,"nonnull":i32::from(item.handle!=0),"rect":[item.rect.top,item.rect.left,item.rect.bottom,item.rect.right],"text_hex":hex(&mut m,first,n)}))
                }).collect::<Result<Vec<_>,String>>()?;
                        let values = json!({"window_kind":m.read_word(d+108)as i16,"visible":m.read_byte(d+110),"hilited":m.read_byte(d+111),"go_away":m.read_byte(d+112),"refcon":m.read_long(d+152)as i32,"port_version":m.read_word(d+6)as i16 as i32 as u32,"port_rect":rect(&mut m,d+16),"edit_field":m.read_word(d+164)as i16,"edit_open":m.read_word(d+166)as i16,"default_item":m.read_word(d+168)as i16,"items_nonnull":i32::from(m.read_long(d+156)!=0),"textedit_nonnull":i32::from(text!=0),"title_hex":title,"items":items,"global_origin":[-bounds[0],-bounds[1]],"bitmap_bounds":bounds,"te_dest":rect(&mut m,tp),"te_view":rect(&mut m,tp+8),"te_font":m.read_word(tp+74)as i16,"te_size":m.read_word(tp+80)as i16,"te_text_length":m.read_word(tp+60)as i16,"te_port_matches":i32::from(m.read_long(tp+82)==d)});
                        row.as_object_mut()
                            .unwrap()
                            .extend(values.as_object().unwrap().clone());
                        m.write_long(sp, d);
                        cpu.pc = 0x19000;
                        cpu.dar[15] = sp;
                        assert!(dialogs.dispatch_lifecycle(
                            if storage != 0 { 0xa982 } else { 0xa983 },
                            &mut cpu,
                            &mut m,
                            &mut heap,
                            &mut q,
                            &fonts,
                            &mut windows,
                            &mut controls,
                            &mut events,
                            &mut te,
                            &resources
                        )?);
                        assert_eq!(q.get_port(&mut m)?, original);
                        assert_eq!(windows.front(), front);
                    }
                    if storage != 0 {
                        heap.dispose_ptr(buffer).map_err(|e| e.to_string())?;
                    }
                    assert!(te.documents(&mut m)?["documents"].as_array().unwrap().is_empty());
                    if let Some(free) = settled_free {
                        assert_eq!(
                            heap.free_bytes(),
                            free,
                            "dialog lifecycle leaked owned storage"
                        );
                    } else {
                        settled_free = Some(heap.free_bytes());
                    }
                    records.push(row);
                }
            }
        }
    }
    Ok(
        json!({"schema":1,"probe":"new-dialog-details-runtime-raw-v1","records":records})
            .to_string(),
    )
}
