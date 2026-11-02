use crate::{resource::Resource, vendor::compressed::decompress_if_needed};
use m68k::{AddressBus, CpuCore, CpuType, StepResult};

pub struct Memory {
    bytes: Vec<u8>,
}
impl Memory {
    pub(crate) fn zeroed(size: usize) -> Self {
        Self {
            bytes: vec![0; size],
        }
    }
    pub(crate) fn put(&mut self, addr: usize, bytes: &[u8]) -> Result<(), String> {
        self.bytes
            .get_mut(addr..addr.checked_add(bytes.len()).ok_or("address overflow")?)
            .ok_or("application exceeds address space")?
            .copy_from_slice(bytes);
        Ok(())
    }
}
impl AddressBus for Memory {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.bytes.get(a as usize).copied().unwrap_or(0)
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        if let Some(b) = self.bytes.get_mut(a as usize) {
            *b = v;
        }
    }
    fn read_word(&mut self, a: u32) -> u16 {
        u16::from_be_bytes([self.read_byte(a), self.read_byte(a + 1)])
    }
    fn write_word(&mut self, a: u32, v: u16) {
        self.put(a as usize, &v.to_be_bytes())
            .expect("word write outside application memory");
    }
    fn read_long(&mut self, a: u32) -> u32 {
        ((self.read_word(a) as u32) << 16) | self.read_word(a + 2) as u32
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.put(a as usize, &v.to_be_bytes())
            .expect("long write outside application memory");
    }
}
fn be32(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        b.get(p..p + 4).ok_or("short CODE 0")?.try_into().unwrap(),
    ))
}
pub(crate) fn load_resource<B: AddressBus>(
    r: &Resource<'_>,
    memory: &mut B,
    heap: &mut crate::managers::Heap,
    cache: &mut std::collections::BTreeMap<([u8; 4], i16), u32>,
) -> Result<u32, String> {
    if let Some(&h) = cache.get(&(r.kind, r.id)) {
        return Ok(h);
    }
    let decoded = decompress_if_needed(r.attributes, r.data).map_err(|e| format!("{e:?}"))?;
    let bytes = decoded.as_deref().unwrap_or(r.data);
    let h = heap
        .new_handle(memory, bytes.len() as u32, false)
        .map_err(|e| format!("resource allocation: {e}"))?;
    let data = heap.data(h).map_err(|e| format!("resource handle: {e}"))?;
    for (i, b) in bytes.iter().enumerate() {
        memory.write_byte(data + i as u32, *b);
    }
    heap.resource_state(h, r.attributes & 0x20 != 0, r.attributes & 0x10 != 0)
        .map_err(|e| e.to_string())?;
    let mut map_handle = memory.read_long(0xa50);
    for _ in 0..1024 {
        if map_handle == 0 {
            break;
        }
        let map = memory.read_long(map_handle);
        if memory.read_word(map + 20) == 2 {
            memory.write_long(map + r.reference_offset as u32 + 8, h);
            break;
        }
        map_handle = memory.read_long(map + 16);
    }
    cache.insert((r.kind, r.id), h);
    Ok(h)
}
// Single-file resource enumeration uses the same parsed map and load cache as
// the ID/name lookup paths. System map is currently an independent empty map.
pub(crate) fn resource_enumeration(
    opcode: u16,
    cpu: &mut CpuCore,
    m: &mut Memory,
    heap: &mut crate::managers::Heap,
    resources: &[Resource<'_>],
    cache: &mut std::collections::BTreeMap<([u8; 4], i16), u32>,
) -> Result<bool, String> {
    let sp = cpu.dar[15];
    let current = m.read_word(0xa5a);
    let available = if current == 2 { resources } else { &[] };
    let pop = match opcode {
        0xa80d => {
            let kind = m.read_long(sp).to_be_bytes();
            let count = available.iter().filter(|r| r.kind == kind).count();
            let count = u16::try_from(count).map_err(|_| "resource count exceeds word range")?;
            m.write_word(sp + 4, count);
            m.write_word(0xa60, 0);
            m.write_word(0x220, 0);
            cpu.dar[0] = 0;
            cpu.set_ccr(0);
            4
        }
        0xa80e => {
            let index = m.read_word(sp) as i16;
            let kind = m.read_long(sp + 2).to_be_bytes();
            let resource = if index > 0 {
                available
                    .iter()
                    .filter(|r| r.kind == kind)
                    .nth(index as usize - 1)
            } else {
                None
            };
            let h = if let Some(r) = resource {
                load_resource(r, m, heap, cache)?
            } else {
                0
            };
            m.write_long(sp + 6, h);
            m.write_word(0xa60, if h == 0 { -192i16 as u16 } else { 0 });
            6
        }
        0xa9a8 => {
            let name = m.read_long(sp);
            let kind = m.read_long(sp + 4);
            let id = m.read_long(sp + 8);
            let h = m.read_long(sp + 12);
            let resource = cache
                .iter()
                .find(|(_, v)| **v == h)
                .and_then(|(key, _)| resources.iter().find(|r| (r.kind, r.id) == *key));
            if let Some(r) = resource {
                let bytes = r.name.unwrap_or(&[]);
                let length =
                    u8::try_from(bytes.len()).map_err(|_| "resource name exceeds Pascal string")?;
                m.write_word(id, r.id as u16);
                m.write_long(kind, u32::from_be_bytes(r.kind));
                m.write_byte(name, length);
                for (i, b) in bytes.iter().enumerate() {
                    m.write_byte(name + 1 + i as u32, *b);
                }
                m.write_word(0xa60, 0);
            } else {
                m.write_word(0xa60, -192i16 as u16);
            }
            16
        }
        0xa9a4 => {
            let h = m.read_long(sp);
            if !cache.values().any(|v| *v == h) {
                return Err("HomeResFile on unknown resource handle".into());
            }
            m.write_word(sp + 4, 2);
            m.write_word(0xa60, 0);
            4
        }
        _ => return Ok(false),
    };
    cpu.dar[15] += pop;
    cpu.pc += 2;
    Ok(true)
}
fn memory_result(cpu: &mut CpuCore, memory: &mut Memory, error: i16) {
    classic_core::traps::memory_result(cpu, memory, error);
}
fn handle_state_trap(
    op: u16,
    cpu: &mut CpuCore,
    memory: &mut Memory,
    heap: &mut crate::managers::Heap,
) {
    if op == 0xa069 {
        let state = heap.state(cpu.dar[8]).unwrap_or_else(|e| e as u8);
        cpu.dar[0] = (cpu.dar[0] & !255) | state as u32;
        memory.write_word(0x220, heap.error as u16);
    } else {
        if op == 0xa049 || op == 0xa04a {
            let _ = heap.purgeable(cpu.dar[8], op == 0xa049);
        } else {
            let _ = heap.set_state(cpu.dar[8], cpu.dar[0] as u8);
        }
        cpu.dar[0] = heap.error as i32 as u32;
        memory_result(cpu, memory, heap.error);
    }
    cpu.pc += 2;
}

/// Fixed RAM reservations for this launch profile: low-memory vectors and
/// service entries, a contiguous application heap, screen storage, then the
/// CODE0-sized A5 world and a separate 64 KiB stack. Heap allocations must never
/// overwrite the screen or an application's globals/jump table.
#[derive(Debug)]
struct LaunchLayout {
    heap_start: u32,
    heap_end: u32,
    a5: usize,
    stack: u32,
}
impl LaunchLayout {
    fn new(above: usize, below: usize) -> Result<Self, String> {
        let world_end = 0x7e0000usize;
        let a5 = world_end.checked_sub(above).ok_or("A5 world exceeds RAM")? & !15;
        let world_start = a5.checked_sub(below.max(204)).ok_or("A5 world underflow")?;
        // QuickDraw's screen base is 0x500000. Keep a full MiB reserved for
        // display storage, including future color depths.
        if world_start < 0x600000 {
            return Err("A5 world overlaps reserved display storage".into());
        }
        Ok(Self {
            heap_start: 0x20000,
            heap_end: 0x500000,
            a5,
            stack: 0x7f0000,
        })
    }
}

#[cfg(test)]
mod launch_layout_tests {
    use super::*;
    #[test]
    fn reserves_actual_heap_screen_world_and_stack() {
        for (above, below) in [(64, 4096), (0x100000, 0x80000), (0, 0)] {
            let layout = LaunchLayout::new(above, below).unwrap();
            assert!(layout.heap_end - layout.heap_start > 2 * 1024 * 1024);
            assert_eq!(
                layout.heap_end,
                crate::quickdraw::QuickDraw::new().screen_base
            );
            assert!(layout.a5 - below.max(204) >= 0x600000);
            assert!(layout.a5 + above <= 0x7e0000);
            assert_eq!(layout.stack - 0x7e0000, 65536);
            assert_eq!(layout.a5 % 16, 0);
        }
        assert!(LaunchLayout::new(0x100000, 0x100000).is_err());
        assert!(LaunchLayout::new(usize::MAX, 0).is_err());
        assert!(LaunchLayout::new(0, usize::MAX).is_err());
    }
}
pub fn trace(resources: &[Resource<'_>], application_resource: &[u8]) -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or("missing application path")?;
    let args: Vec<String> = std::env::args().collect();
    let inferred_name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid application name")?;
    let selected_name = if let Some(i) = args.iter().position(|a| a == "--application-name") {
        args.get(i + 1).ok_or("missing application name")?.as_str()
    } else {
        inferred_name
    };
    let name = crate::mac_roman::encode(selected_name)?;
    let mut process =
        crate::processes::Process::from_resources(&name, resources, 0x20000, 0x7e0000)?;
    if let Some(i) = args.iter().position(|a| a == "--application-creator") {
        let creator =
            crate::mac_roman::encode(args.get(i + 1).ok_or("missing application creator")?)?;
        let creator: [u8; 4] = creator
            .try_into()
            .map_err(|_| "application creator must contain four Macintosh bytes")?;
        process.set_signature(u32::from_be_bytes(creator));
    }
    let mut memory = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let code0 = resources
        .iter()
        .find(|r| r.kind == *b"CODE" && r.id == 0)
        .ok_or("missing CODE 0")?;
    let above = be32(code0.data, 0)? as usize;
    let below = be32(code0.data, 4)? as usize;
    let jtlen = be32(code0.data, 8)? as usize;
    let jtoff = be32(code0.data, 12)? as usize;
    if above > 0x100000 || below > 0x100000 || jtlen % 8 != 0 || jtoff + jtlen > above {
        return Err("unsupported A5 world layout".into());
    }
    let layout = LaunchLayout::new(above, below)?;
    let a5 = layout.a5;
    if args.iter().any(|a| a == "--trace") {
        println!("application memory layout: {layout:?}");
    }
    let segments = crate::segments::Segments::new((a5 + jtoff) as u32, jtlen as u32);
    let table = code0.data.get(16..16 + jtlen).ok_or("short jump table")?;
    memory.put(a5 + jtoff, table)?;
    let stack = layout.stack;
    memory.write_long(0, stack);
    memory.write_long(4, (a5 + jtoff + 2) as u32);
    memory.write_long(0x904, a5 as u32);
    memory.write_long(0x908, stack);
    memory.write_word(0x934, jtoff as u16);
    memory.write_long(0x108, 0x800000);
    memory.write_long(0x130, layout.heap_end);
    memory.write_word(0x18000, 0xa1ad);
    memory.write_word(0x18002, 0x4e75);
    memory.write_word(0x18100, 0xa89f);
    memory.write_word(0x18102, 0x4e75);
    // Pascal call-through entry for HOpenResFile: remove the return address
    // before its stack-based trap and return through volatile A1 afterwards.
    memory.write_word(0x18300, 0x225f);
    memory.write_word(0x18302, 0xa81a);
    memory.write_word(0x18304, 0x4ed1);
    memory.write_word(0x15a, 0x0921);
    let mut heap = crate::managers::Heap::new(layout.heap_start, layout.heap_end);
    classic_core::drivers::initialize(&mut memory,&mut heap,classic_core::drivers::PROFILE_UNIT_COUNT).map_err(|e|format!("device unit table: {e}"))?;
    let system_map = heap
        .new_handle(&mut memory, 32, true)
        .map_err(|e| format!("system resource map: {e}"))?;
    let system_data = heap.data(system_map).map_err(|e| e.to_string())?;
    memory.write_word(system_data + 24, 28);
    memory.write_word(system_data + 26, 30);
    memory.write_word(system_data + 28, 0xffff);
    let raw_map = resources
        .first()
        .ok_or("empty application resource map")?
        .map;
    let app_map = heap
        .new_handle(&mut memory, raw_map.len() as u32, false)
        .map_err(|e| format!("application resource map: {e}"))?;
    let map_data = heap.data(app_map).map_err(|e| e.to_string())?;
    memory.put(map_data as usize, raw_map)?;
    memory.write_long(map_data + 16, system_map);
    memory.write_word(map_data + 20, 2);
    for r in resources {
        memory.write_long(map_data + r.reference_offset as u32 + 8, 0);
    }
    memory.write_long(0xa50, app_map);
    memory.write_long(0xa54, system_map);
    memory.write_word(0xa5a, 2);
    let mut resource_handles = std::collections::BTreeMap::new();
    let scripts = crate::scripts::Scripts::roman();
    let mut graphics = crate::quickdraw::QuickDraw::new();
    graphics.color_capable = args.iter().any(|a| a == "--color-screen");
    let mut fonts = crate::fonts::Fonts::new();
    let mut sound = crate::soundmanager::Sound::new(&args,memory.bytes.len() as u32)?;
    sound.initialize_guest(&mut memory);
    let mut environment = crate::environment::Environment::runtime();
    if sound.active() {environment.sound_version=0x03008000;}
    environment.initialize(&mut memory);
    classic_core::cursor::initialize(&mut memory);
    classic_core::mouse::initialize(&mut memory, graphics.width, graphics.height);
    let mut events = crate::events::Events::new();
    memory.write_long(0x2f0, 32);
    let mut windows = crate::windows::Windows::new();
    let mut menus = crate::menus::Menus::new();
    let mut appleevents = crate::appleevents::AppleEvents::new();
    let mut textedit = crate::textedit::TextEdit::new();
    let mut controls = crate::controls::Controls::new();
    let mut files = crate::files::Files::new();
    let mut resfiles = crate::resfiles::ResFiles::with_color_seeds(graphics.color_seeds());
    let mut callbacks = crate::callbacks::Callbacks::new();
    let mut dialogs = crate::dialogs::Dialogs::new();
    let mut alerts = crate::alerts::Alerts::new();
    let mut timing = crate::timing::Timing::default();
    let mut sane = classic_core::sane::Sane::new(memory.bytes.len() as u32);
    let trace_sane = args.iter().any(|a| a == "--trace-sane");
    let mut time_manager = crate::timemanager::TimeManager::new(memory.bytes.len() as u32);
    time_manager.trace = args.iter().any(|a| a == "--trace-timers");
    let mut vbl = crate::vbl::VBL::new(memory.bytes.len() as u32);
    // This owned software screen has an explicit 60Hz virtual pulse source.
    // It is not the reference VM's unbound legacy slot or physical HDMI sync.
    vbl.state
        .bind_slot(0)
        .map_err(|e| format!("virtual display VBL binding: {e}"))?;
    vbl.trace = args.iter().any(|a| a == "--trace-vbl");
    let mut run_clock = crate::timemanager::RunClock::new();
    let mut standardfile = crate::standardfile::StandardFile::new();
    let mut modal = crate::modal::ModalDialog::default();
    let mut cpu = CpuCore::new();
    cpu.set_cpu_type(crate::environment::CPU_TYPE);
    cpu.reset(&mut memory);
    cpu.dar[13] = a5 as u32;
    let interactive = args.iter().any(|a| a == "--gui");
    #[cfg(not(feature = "gui"))]
    if interactive {
        return Err("--gui requires cargo --features gui".into());
    }
    #[cfg(feature = "gui")]
    let mut frontend = if interactive {
        let presentation_log = args
            .iter()
            .position(|a| a == "--presentation-log")
            .map(|i| {
                args.get(i + 1)
                    .map(String::as_str)
                    .ok_or("missing presentation log path")
            })
            .transpose()?;
        Some(crate::frontend::Frontend::new(&graphics, presentation_log)?)
    } else {
        None
    };
    if let Some(i) = args.iter().position(|a| a == "--volume") {
        files = crate::files::Files::volume(std::path::Path::new(
            args.get(i + 1).ok_or("missing volume directory")?,
        ))?;
    }
    let application_data = if let Some(i) = args.iter().position(|a| a == "--application-data") {
        std::fs::read(args.get(i + 1).ok_or("missing application data fork")?)
            .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let mut finder = [0; 16];
    finder[..4].copy_from_slice(b"APPL");
    finder[4..8].copy_from_slice(&process.signature().to_be_bytes());
    if let Some(i) = args.iter().position(|a| a == "--application-files") {
        files.mount_application_files(
            std::path::Path::new(
                args.get(i + 1)
                    .ok_or("missing application bundle directory")?,
            ),
            &name,
            finder,
            &application_data,
            application_resource,
        )?;
    } else {
        files.mount_application(&name, finder, &application_data, application_resource)?;
    }

    process.initialize_application(&mut memory, crate::files::APPLICATION_RESOURCE_FILE);
    let mut script = if let Some(i) = args.iter().position(|a| a == "--event-script") {
        crate::events::read_script(args.get(i + 1).ok_or("missing event script path")?)?
    } else {
        std::collections::VecDeque::new()
    };
    let capture = if let Some(i) = args.iter().position(|a| a == "--capture-framebuffer") {
        Some(args.get(i + 1).ok_or("missing framebuffer path")?.as_str())
    } else {
        None
    };
    #[cfg(feature = "gui")]
    if let Some(frontend) = frontend.as_mut() { frontend.set_snapshot_path(capture)?; }
    let picture_capture = if let Some(i) = args.iter().position(|a| a == "--capture-picture-frames") {
        let path=args.get(i+1).ok_or("missing picture capture directory")?.clone();
        std::fs::create_dir_all(&path).map_err(|e|e.to_string())?;
        Some(path)
    } else {None};
    let mut drawing_capture_sequence=0usize;
    let dump_documents = if let Some(i) = args.iter().position(|a| a == "--dump-documents") {
        Some(
            args.get(i + 1)
                .ok_or("missing document snapshot path")?
                .as_str(),
        )
    } else {
        None
    };
    let instruction_limit =
        if let Some(index) = args.iter().position(|a| a == "--instruction-limit") {
            let value = args
                .get(index + 1)
                .ok_or("--instruction-limit requires a positive integer")?
                .parse::<usize>()
                .map_err(|_| "invalid --instruction-limit")?;
            if value == 0 {
                return Err("--instruction-limit must be positive".into());
            }
            value
        } else if interactive {
            usize::MAX
        } else {
            100000
        };
    let watch_byte = if let Some(i) = args.iter().position(|a| a == "--watch-byte") {
        Some(
            u32::from_str_radix(
                args.get(i + 1)
                    .ok_or("--watch-byte requires a hexadecimal address")?
                    .trim_start_matches("0x"),
                16,
            )
            .map_err(|_| "invalid --watch-byte address")?,
        )
    } else {
        None
    };
    let mut watched = watch_byte.map(|p| memory.read_byte(p));
    let mut writer = (cpu.pc, memory.read_word(cpu.pc));
    for instruction in 0..instruction_limit {
        if let Some(address) = watch_byte {
            let value = memory.read_byte(address);
            if watched != Some(value) {
                println!(
                    "memory watch {address:#x}: {:?}->{value:#04x} after PC={:#x} opcode={:#06x}",
                    watched, writer.0, writer.1
                );
                watched = Some(value);
            }
        }
        writer = (cpu.pc, memory.read_word(cpu.pc));

        #[cfg(feature = "gui")]
        if instruction % 16 == 0 {
            if let Some(frontend) = frontend.as_mut() {
                if !frontend.poll(
                    &mut memory,
                    &graphics,
                    &mut events,
                    &mut script,
                    crate::frontend::PresentationContext {
                        pc: cpu.pc,
                        instruction,
                        update_depth: windows.update_depth(),
                        audio_frame: &|| sound.output_frame(),
                    },
                )? {
                    break;
                }
            }
        }
        if let Some((token, result)) = callbacks.finish(&mut cpu, &mut memory)? {
            match token {
                1 | 3 => standardfile.callback_result(token, result)?,
                2 => textedit.click_result(result)?,
                4 => dialogs.finish_draw(&mut memory, &graphics)?,
                5 => controls.callback_result()?,
                6 => alerts.callback_result(result)?,
                7 => time_manager.callback_result()?,
                8 => vbl.callback_result(&mut memory)?,
                9 => modal.callback_result(result)?,
                10 => modal.track_result(result)?,
                11 => sound.callback_result(),
                _ => return Err("unknown callback owner".into()),
            }
            continue;
        }
        let now_us = run_clock.now(events.ticks);
        events.ticks = (now_us.saturating_mul(60) / 1_000_000) as u32;
        memory.write_long(0x16a, events.ticks);
        if sound.service(&mut cpu,&mut memory,now_us,&mut callbacks)? {continue;}
        if !vbl.is_busy() && time_manager.service(&mut cpu, &mut memory, now_us, &mut callbacks)? {
            continue;
        }
        if (!time_manager.is_running() || vbl.manual_pending())
            && vbl.service(
                &mut cpu,
                &mut memory,
                now_us.saturating_mul(60) / 1_000_000,
                &mut callbacks,
            )?
        {
            continue;
        }
        if dialogs.service_draw(&mut cpu, &mut memory, &graphics, &mut callbacks)? {
            continue;
        }
        let pc = cpu.pc;
        if pc as usize + 2 > memory.bytes.len() {
            return Err(format!("PC outside application memory: {pc:#x}"));
        }
        let opcode = memory.read_word(pc);
        if trace_sane && matches!(opcode, 0xa9eb | 0xa9ec) {
            let selector = memory.read_word(cpu.dar[15]);
            eprintln!("SANE package={opcode:#x} selector={selector:#x} pc={pc:#x}");
        }
        if classic_core::drivers::dispatch(opcode,&mut cpu,&mut memory).map_err(|e|format!("device manager: {e:?}"))? {continue;}
        if sane
            .dispatch(opcode, &mut cpu, &mut memory)
            .map_err(|e| e.to_string())?
        {
            continue;
        }
        if trace_sane && matches!(opcode, 0xa9eb | 0xa9ec) {
            let sp = cpu.dar[15];
            if (sp as usize).checked_add(10).is_some_and(|end| end <= memory.bytes.len()) {
                let dest = memory.read_long(sp + 2);
                let source = memory.read_long(sp + 6);
                let packed = |p: u32| -> String {
                    (p as usize).checked_add(10)
                        .and_then(|end| memory.bytes.get(p as usize..end))
                        .map(|b| b.iter().map(|v| format!("{v:02x}")).collect())
                        .unwrap_or_else(|| "outside-memory".into())
                };
                eprintln!("unsupported SANE operands: dest={dest:#x} packed10={} source={source:#x} packed10={}", packed(dest), packed(source));
            }
        }
        if crate::cursors::dispatch(opcode, &mut cpu, &mut memory)? { continue; }
        if crate::pictures::dispatch(opcode, &mut cpu, &mut memory, &mut heap, &graphics)? {
            capture_drawing(&mut memory,&graphics,&picture_capture,&mut drawing_capture_sequence,opcode,pc)?;
            continue;
        }
        if time_manager.dispatch(opcode, &mut cpu, &mut memory, now_us) {
            continue;
        }
        if vbl.dispatch(opcode, &mut cpu, &mut memory)? {
            continue;
        }
        if crate::quickdraw::origin_dispatch(opcode, &mut cpu, &mut memory, &mut heap, &graphics)? {
            continue;
        }
        if crate::quickdraw::max_device_dispatch(
            opcode,
            &mut cpu,
            &mut memory,
            &mut heap,
            &mut graphics,
        )? {
            continue;
        }
        if crate::quickdraw::device_lifecycle_dispatch(
            opcode,
            &mut cpu,
            &mut memory,
            &mut heap,
            &mut graphics,
        )? {
            continue;
        }
        if crate::roundrect::dispatch(opcode, &mut cpu, &mut memory, &graphics)? {
            continue;
        }
        if crate::dialogs::item_text_dispatch(opcode, &mut cpu, &mut memory, &mut heap)? {
            continue;
        }
        if crate::quickdraw::line_dispatch(opcode, &mut cpu, &mut memory, &graphics)? {
            continue;
        }
        if crate::quickdraw::pen_pattern_dispatch(opcode, &mut cpu, &mut memory, &graphics)? {
            continue;
        }
        if crate::colors::dispatch(opcode, &mut cpu, &mut memory, &mut heap, &mut graphics)? {
            if opcode==0xa8ec {capture_drawing(&mut memory,&graphics,&picture_capture,&mut drawing_capture_sequence,opcode,pc)?;}
            continue;
        }
        if let Some(complete) = timing.dispatch(opcode, &mut cpu, events.ticks)? {
            if !complete {
                if interactive {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                } else {
                    events.ticks = events.ticks.wrapping_add(1);
                    memory.write_long(0x16a, events.ticks);
                }
            }
            continue;
        }

        if matches!(opcode, 0xa93d | 0xa9d4 | 0xa925 | 0xa92b | 0xa968) {
            if let Some(i) = script.iter().position(|e| matches!(e.what, 0 | 1 | 2)) {
                let e = script.remove(i).unwrap();
                events.set_mouse(&mut memory, (e.v, e.h));
                events.modifiers = e.modifiers;
            }
        }
        if events.dispatch_state(opcode, &mut cpu, &mut memory, &graphics)? {
            continue;
        }
        if events.dispatch_post(opcode, &mut cpu) { continue; }
        if events.dispatch_os_event(opcode, &mut cpu, &mut memory) { continue; }
        if opcode & 0xf000 == 0xa000 {
            if args.iter().any(|a| a == "--trace-textedit")
                && [
                    0xa83e, 0xa83d, 0xa9cf, 0xa9d1, 0xa9d3, 0xa9d8, 0xa9d9, 0xa9dc,
                ]
                .contains(&opcode)
            {
                let sp = cpu.dar[15];
                let mut words = Vec::new();
                for i in 0..10 {
                    words.push(memory.read_word(sp + 2 * i));
                }
                println!(
                    "TextEdit trap={opcode:#06x} PC={:#x} args={words:x?}",
                    cpu.pc
                );
            }
            if args.iter().any(|a| a == "--trace-windows")
                && [
                    0xa913, 0xa915, 0xa916, 0xa91b, 0xa91d, 0xa922, 0xa923, 0xa904,
                ]
                .contains(&opcode)
            {
                let w = memory.read_long(cpu.dar[15]);
                let detail = if windows.windows.iter().any(|win| win.address == w) {
                    let mut fields = Vec::new();
                    for offset in [24, 114, 118, 122] {
                        let h = memory.read_long(w + offset);
                        let p = memory.read_long(h);
                        fields.push(crate::quickdraw::read_rect(&mut memory, p + 2));
                    }
                    format!("visible={} regions={fields:?}", memory.read_byte(w + 110))
                } else {
                    "creation/geometry arguments".into()
                };
                println!("window trap={opcode:#06x} window={w:#x} {detail}");
            }
            if opcode == 0xa985 && !alerts.is_active() && args.iter().any(|a| a == "--trace-alerts")
            {
                let id = memory.read_word(cpu.dar[15] + 4) as i16;
                println!(
                    "StopAlert request: id={id} text={:?}",
                    dialogs.alert_description(&mut memory, resources, id)
                );
            }
            if args.iter().any(|a| a == "--trace-scrap")
                && matches!(
                    opcode,
                    0xa9c2 | 0xa9d5 | 0xa9d6 | 0xa9db | 0xa9f9 | 0xa9fc | 0xa9fd | 0xa9fe
                )
            {
                println!(
                    "clipboard trap={opcode:#06x} PC={pc:#x} SP={:#x}",
                    cpu.dar[15]
                );
            }
            if crate::regions::dispatch(opcode, &mut cpu, &mut memory, &mut heap, &graphics)? {
                continue;
            }
            if opcode == 0xa902 && args.iter().any(|a| a == "--trace-fonts") {
                let sp = cpu.dar[15];
                println!(
                    "RealFont family={} size={}",
                    memory.read_word(sp + 2) as i16,
                    memory.read_word(sp) as i16
                );
            }
            if crate::icons::dispatch(opcode, &mut cpu, &mut memory, &graphics, resources)? {
                continue;
            }
            if fonts.dispatch(opcode, &mut cpu, &mut memory) {
                continue;
            }
            if crate::truncation::dispatch(opcode, &mut cpu, &mut memory, &graphics, &fonts)? {
                continue;
            }
            if scripts.dispatch(opcode, &mut cpu, &mut memory)? {
                continue;
            }
            if crate::stringutils::dispatch(opcode, &mut cpu, &mut memory)? {
                continue;
            }
            if environment.dispatch(opcode, &mut cpu, &mut memory) {
                continue;
            }
            if classic_core::traps::memory_dispatch(opcode, &mut cpu, &mut memory, &mut heap) {
                continue;
            }
            if sound.dispatch(opcode, &mut cpu, &mut memory, &mut heap)? {
                continue;
            }
            if appleevents.dispatch(opcode, &mut cpu, &mut memory)? {
                continue;
            }
            if resfiles.dispatch(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &mut files,
                resources,
                &mut resource_handles,
                load_resource,
            )? {
                continue;
            }
            if resource_enumeration(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                resources,
                &mut resource_handles,
            )? {
                continue;
            }
            let application_resources_visible = resfiles.application_visible(&mut memory);
            let available = if application_resources_visible {
                resources
            } else {
                &[]
            };
            if windows.dispatch(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &mut graphics,
                &fonts,
                &mut events,
                available,
                |r, m, heap| load_resource(r, m, heap, &mut resource_handles),
            )? {
                if matches!(opcode, 0xa925 | 0xa92b) && !windows.is_tracking() {
                    let w = memory.read_long(cpu.dar[15] - 4);
                    println!("window tracking: trap={opcode:#06x} window={w:#x} bounds={:?} grow_result={:?}",windows.windows.iter().find(|win|win.address==w).map(|win|win.bounds),if opcode==0xa92b {Some(memory.read_long(cpu.dar[15]))}else{None});
                }
                if interactive && windows.is_tracking() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                continue;
            }
            if menus.dispatch(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &fonts,
                available,
                |r, m, heap| load_resource(r, m, heap, &mut resource_handles),
            )? {
                continue;
            }
            if process.dispatch(opcode, &mut cpu, &mut memory, &heap)? {
                continue;
            }
            if opcode == 0xa9c2 {
                system_edit(&mut cpu, &mut memory, &windows)?;
                continue;
            }
            if crate::scrap::dispatch(opcode, &mut cpu, &mut memory, &mut heap)? {
                continue;
            }
            if textedit.dispatch(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &graphics,
                &fonts,
                &events,
                &mut callbacks,
            )? {
                continue;
            }
            if opcode == 0xa968 {
                let h = memory.read_long(cpu.dar[15] + 8);
                controls.track(&mut cpu, &mut memory, &graphics, &events, &mut callbacks)?;
                if !controls.is_tracking() {
                    let p = memory.read_long(h);
                    println!(
                        "control tracking: handle={h:#x} result={} value={} hilite={}",
                        memory.read_word(cpu.dar[15]),
                        memory.read_word(p + 18) as i16,
                        memory.read_byte(p + 17)
                    );
                }
                if interactive && controls.is_tracking() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                continue;
            }
            if controls.dispatch(opcode, &mut cpu, &mut memory, &mut heap, &graphics)? {
                continue;
            }
            if files.dispatch(opcode, &mut cpu, &mut memory)? {
                continue;
            }
            if modal.dispatch(opcode,&mut cpu,&mut memory,&mut heap,&graphics,&fonts,&mut windows,&controls,&mut events,&mut dialogs,&mut callbacks,&mut script)? {
                if interactive && modal.waiting_for_input(){std::thread::sleep(std::time::Duration::from_millis(1));}
                continue;
            }
            let trace_dialog = opcode == 0xa97c && args.iter().any(|a| a == "--trace" || a == "--trace-dialogs");
            if trace_dialog {
                let sp=cpu.dar[15];
                println!("GetNewDialog request: id={} storage={:#x} behind={:#x}",memory.read_word(sp+8)as i16,memory.read_long(sp+4),memory.read_long(sp));
            }
            if dialogs.dispatch_lifecycle(opcode,&mut cpu,&mut memory,&mut heap,&mut graphics,&fonts,&mut windows,&mut controls,&mut events,&mut textedit,resources)? {
                if trace_dialog {println!("GetNewDialog returned: address={:#x}",cpu.dar[0]);}
                continue;
            }
            if dialogs.dispatch_filter(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &graphics,
                &fonts,
                &controls,
            )? {
                continue;
            }
            if dialogs.dispatch_draw(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &graphics,
                &fonts,
                &controls,
            )? {
                continue;
            }
            if dialogs.dispatch(opcode, &mut cpu, &mut memory, &mut heap)? {
                continue;
            }
            if alerts.dispatch(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &graphics,
                &fonts,
                &mut windows,
                &mut controls,
                &mut events,
                &mut dialogs,
                &mut callbacks,
                available,
                &mut script,
            )? {
                if interactive && alerts.waiting_for_input() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                continue;
            }
            if standardfile.dispatch(
                opcode,
                &mut cpu,
                &mut memory,
                &mut heap,
                &graphics,
                &fonts,
                &mut windows,
                &mut controls,
                &mut events,
                &mut dialogs,
                &mut callbacks,
                &files,
                resources,
                &mut script,
            )? {
                if interactive && standardfile.waiting_for_input() && script.is_empty() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                continue;
            }
            match opcode {
                0xa93d => {
                    let sp = cpu.dar[15];
                    let start = (memory.read_word(sp) as i16, memory.read_word(sp + 2) as i16);
                    let cancel = events.take_key(27)
                        || script.iter().any(|e| e.what == 3 && e.message as u8 == 27);
                    script.retain(|e| !(e.what == 3 && e.message as u8 == 27));
                    events.flush(0x6, 0);
                    if let Some(result) = menus.track(
                        &mut memory,
                        &mut heap,
                        &graphics,
                        &fonts,
                        windows
                            .manager_port
                            .ok_or("MenuSelect before Window Manager initialization")?,
                        start,
                        &events,
                        cancel,
                    )? {
                        script.retain(|e| !matches!(e.what, 0 | 1 | 2));
                        memory.write_long(sp + 4, result);
                        cpu.dar[15] += 4;
                        cpu.pc += 2;
                    } else if interactive {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    continue;
                }
                0xa92c => {
                    let sp = cpu.dar[15];
                    let result = memory.read_long(sp);
                    let v = memory.read_word(sp + 4) as i16;
                    let h = memory.read_word(sp + 6) as i16;
                    let (part, window) = windows.find(&mut memory, v, h)?;
                    if args.iter().any(|a| a == "--trace-windows") {
                        println!(
                            "FindWindow: point=({h},{v}) part={part} window={window:#x} model={:?}",
                            windows
                                .windows
                                .iter()
                                .find(|w| w.address == window)
                                .map(|w| (w.proc_id, w.bounds))
                        );
                    }
                    memory.write_long(result, window);
                    memory.write_word(sp + 8, part as u16);
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa919 => {
                    let sp = cpu.dar[15];
                    let dest = memory.read_long(sp);
                    let w = memory.read_long(sp + 4);
                    if !windows.windows.iter().any(|entry| entry.address == w) {
                        return Err("GetWTitle on unknown window".into());
                    }
                    let h = memory.read_long(w + 134);
                    let p = memory.read_long(h);
                    let len = memory.read_byte(p) as u32;
                    for i in 0..=len {
                        let b = memory.read_byte(p + i);
                        memory.write_byte(dest + i, b);
                    }
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa904 => {
                    let sp = cpu.dar[15];
                    let w = memory.read_long(sp);
                    windows.grow_icon(&mut memory, &graphics, w)?;
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa920 | 0xa91f | 0xa915 | 0xa916 | 0xa922 | 0xa923 => {
                    let sp = cpu.dar[15];
                    let w = memory.read_long(sp);
                    match opcode {
                        0xa920 => windows.bring_front(&mut memory, w)?,
                        0xa91f => windows.select(&mut memory, &graphics, &fonts, &mut events, w)?,
                        0xa915 | 0xa916 => windows.visible(
                            &mut memory,
                            &mut heap,
                            &graphics,
                            &fonts,
                            &mut events,
                            w,
                            opcode == 0xa915,
                        )?,
                        0xa922 => windows.begin_update(&mut memory, &mut heap, &graphics, w)?,
                        0xa923 => windows.end_update(&mut memory, &mut heap, w)?,
                        _ => unreachable!(),
                    }
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa86a | 0xa86b => {
                    let sp = cpu.dar[15];
                    let value = memory.read_long(sp);
                    let value = if opcode == 0xa86a {
                        (value >> 16) as u16
                    } else {
                        value as u16
                    };
                    memory.write_word(sp + 4, value);
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa858 => {
                    let sp = cpu.dar[15];
                    let b = memory.read_long(sp);
                    let a = memory.read_long(sp + 4);
                    memory.write_long(sp + 8, a & b);
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa830 if cpu.dar[0] as u16 == 3 => {
                    // Balloon Help is off in this system profile.
                    memory.write_byte(cpu.dar[15], 0);
                    cpu.pc += 2;
                    continue;
                }
                0xa9b4 => {
                    // Cooperative housekeeping for this single-application
                    // profile; no desk accessories or device tasks installed.
                    if !interactive {
                        events.ticks = events.ticks.wrapping_add(1);
                    }
                    memory.write_long(0x16a, events.ticks);
                    cpu.pc += 2;
                    continue;
                }
                0xa8a7 => {
                    let sp = cpu.dar[15];
                    let bottom = memory.read_word(sp) as i16;
                    let right = memory.read_word(sp + 2) as i16;
                    let top = memory.read_word(sp + 4) as i16;
                    let left = memory.read_word(sp + 6) as i16;
                    let p = memory.read_long(sp + 8);
                    crate::quickdraw::write_rect(
                        &mut memory,
                        p,
                        crate::managers::Rect {
                            top,
                            left,
                            bottom,
                            right,
                        },
                    );
                    cpu.dar[15] += 12;
                    cpu.pc += 2;
                    continue;
                }
                0xa8a8 | 0xa8a9 => {
                    let sp = cpu.dar[15];
                    let dv = memory.read_word(sp) as i16;
                    let dh = memory.read_word(sp + 2) as i16;
                    let p = memory.read_long(sp + 4);
                    let mut r = crate::quickdraw::read_rect(&mut memory, p);
                    r.top = r.top.wrapping_add(dv);
                    r.left = r.left.wrapping_add(dh);
                    if opcode == 0xa8a8 {
                        r.bottom = r.bottom.wrapping_add(dv);
                        r.right = r.right.wrapping_add(dh);
                    } else {
                        r.bottom = r.bottom.wrapping_sub(dv);
                        r.right = r.right.wrapping_sub(dh);
                    }
                    crate::quickdraw::write_rect(&mut memory, p, r);
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa8ae | 0xa8a6 => {
                    let sp = cpu.dar[15];
                    let a = memory.read_long(sp);
                    let a = crate::quickdraw::read_rect(&mut memory, a);
                    let (value, pop) = if opcode == 0xa8a6 {
                        let b = memory.read_long(sp + 4);
                        let b = crate::quickdraw::read_rect(&mut memory, b);
                        (a == b, 8)
                    } else {
                        (a.top >= a.bottom || a.left >= a.right, 4)
                    };
                    memory.write_byte(sp + pop, value as u8);
                    cpu.dar[15] += pop;
                    cpu.pc += 2;
                    continue;
                }
                0xa97b => {
                    let sp = cpu.dar[15];
                    let resume = memory.read_long(sp);
                    memory.write_long(0xa8c, resume);
                    memory.write_word(0xafa, 0);
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xaa29 | 0xaa2a => {
                    let h = graphics.device(&mut memory, &mut heap)?;
                    memory.write_long(cpu.dar[15], h);
                    cpu.pc += 2;
                    continue;
                }
                0xaa2b => {
                    let sp = cpu.dar[15];
                    let h = memory.read_long(sp);
                    let p = heap.data(h).map_err(|e| format!("GetNextDevice: {e}"))?;
                    let next = memory.read_long(p + 30);
                    memory.write_long(sp + 4, next);
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xaa2c => {
                    let sp = cpu.dar[15];
                    let bit = memory.read_word(sp);
                    let h = memory.read_long(sp + 2);
                    let p = heap
                        .data(h)
                        .map_err(|e| format!("TestDeviceAttribute: {e}"))?;
                    if bit >= 16 {
                        return Err("device attribute outside flag word".into());
                    }
                    let value = memory.read_word(p + 20) & (1 << bit) != 0;
                    memory.write_byte(sp + 6, value as u8);
                    cpu.dar[15] += 6;
                    cpu.pc += 2;
                    continue;
                }
                0xa9ba => {
                    let sp = cpu.dar[15];
                    let id = memory.read_word(sp) as i16;
                    let h = if let Some(r) = resources
                        .iter()
                        .find(|r| application_resources_visible && r.kind == *b"STR " && r.id == id)
                    {
                        load_resource(r, &mut memory, &mut heap, &mut resource_handles)?
                    } else {
                        0
                    };
                    memory.write_word(0xa60, if h == 0 { -192i16 as u16 } else { 0 });
                    memory.write_long(sp + 2, h);
                    cpu.dar[15] += 2;
                    cpu.pc += 2;
                    continue;
                }
                0xa9b9 => {
                    let sp = cpu.dar[15];
                    let id = memory.read_word(sp) as i16;
                    let key = (*b"CURS", id);
                    let h = if let Some(r) = resources.iter().find(|r| (r.kind, r.id) == key) {
                        load_resource(r, &mut memory, &mut heap, &mut resource_handles)?
                    } else if let Some(&h) = resource_handles.get(&key) {
                        h
                    } else if let Some(bytes) = crate::cursors::builtin(id) {
                        let h = heap
                            .new_handle(&mut memory, 68, false)
                            .map_err(|e| format!("cursor: {e}"))?;
                        let p = heap.data(h).map_err(|e| format!("cursor: {e}"))?;
                        memory.put(p as usize, &bytes)?;
                        resource_handles.insert(key, h);
                        h
                    } else {
                        0
                    };
                    memory.write_word(0xa60, if h == 0 { -192i16 as u16 } else { 0 });
                    memory.write_long(sp + 2, h);
                    cpu.dar[15] += 2;
                    cpu.pc += 2;
                    continue;
                }
                0xa851 => {
                    let sp = cpu.dar[15];
                    let p = memory.read_long(sp);
                    let bytes: Vec<u8> = (0..68).map(|i| memory.read_byte(p + i)).collect();
                    memory.put(0x844, &bytes)?;
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa930 => {
                    menus.init(&mut memory, &mut heap)?;
                    cpu.pc += 2;
                    continue;
                }
                0xa937 => {
                    menus.draw(
                        &mut memory,
                        &graphics,
                        &fonts,
                        windows
                            .manager_port
                            .ok_or("menu drawing before Window Manager init")?,
                    )?;
                    cpu.pc += 2;
                    continue;
                }
                0xa938 => {
                    let sp = cpu.dar[15];
                    menus.highlighted = memory.read_word(sp) as i16;
                    memory.write_word(0xa26, menus.highlighted as u16);
                    menus.draw(
                        &mut memory,
                        &graphics,
                        &fonts,
                        windows.manager_port.ok_or("no menu drawing port")?,
                    )?;
                    cpu.dar[15] += 2;
                    cpu.pc += 2;
                    continue;
                }
                0xa912 => {
                    windows.init(&mut memory, &mut heap, &graphics)?;
                    cpu.pc += 2;
                    continue;
                }
                0xa913 | 0xaa45 => {
                    let sp = cpu.dar[15];
                    let refcon = memory.read_long(sp);
                    let go_away = memory.read_byte(sp + 4) != 0;
                    let behind = memory.read_long(sp + 6);
                    let proc_id = memory.read_word(sp + 10) as i16;
                    let visible = memory.read_byte(sp + 12) != 0;
                    let title = memory.read_long(sp + 14);
                    let length = memory.read_byte(title) as u32;
                    let title: Vec<u8> = (0..length)
                        .map(|i| memory.read_byte(title + 1 + i))
                        .collect();
                    let bounds = memory.read_long(sp + 18);
                    let bounds = crate::quickdraw::read_rect(&mut memory, bounds);
                    let storage = memory.read_long(sp + 22);
                    let window = windows.create(
                        &mut memory,
                        &mut heap,
                        &graphics,
                        &fonts,
                        &mut events,
                        storage,
                        bounds,
                        &title,
                        visible,
                        proc_id,
                        behind,
                        go_away,
                        refcon,
                    )?;
                    if opcode == 0xaa45 {
                        windows.colorize(
                            &mut memory,
                            &mut heap,
                            &mut graphics,
                            &fonts,
                            window,
                            None,
                        )?;
                    }
                    memory.write_long(sp + 26, window);
                    cpu.dar[15] += 26;
                    cpu.pc += 2;
                    continue;
                }
                0xa924 => {
                    memory.write_long(cpu.dar[15], windows.front());
                    cpu.pc += 2;
                    continue;
                }
                0xa914 => {
                    let sp = cpu.dar[15];
                    let w = memory.read_long(sp);
                    windows.dispose(&mut memory, &mut heap, &graphics, &mut events, w)?;
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa883 | 0xa884 | 0xa885 => {
                    let sp = cpu.dar[15];
                    let (text, pop) = match opcode {
                        0xa883 => (vec![memory.read_word(sp) as u8], 2),
                        0xa884 => {
                            let p = memory.read_long(sp);
                            let n = memory.read_byte(p) as u32;
                            ((0..n).map(|i| memory.read_byte(p + 1 + i)).collect(), 4)
                        }
                        0xa885 => {
                            let n = memory.read_word(sp) as u32;
                            let first = memory.read_word(sp + 2) as i16;
                            let p = memory.read_long(sp + 4).wrapping_add(first as i32 as u32);
                            ((0..n).map(|i| memory.read_byte(p + i)).collect(), 8)
                        }
                        _ => unreachable!(),
                    };
                    graphics.text(&mut memory, &fonts, &text)?;
                    cpu.dar[15] += pop;
                    cpu.pc += 2;
                    continue;
                }
                0xa8a1 | 0xa8a2 | 0xa8a3 | 0xa8a4 | 0xa8a5 => {
                    let sp = cpu.dar[15];
                    let rect = memory.read_long(sp + if opcode == 0xa8a5 { 4 } else { 0 });
                    let rect = crate::quickdraw::read_rect(&mut memory, rect);
                    if opcode == 0xa8a1 {
                        graphics.frame_rect(&mut memory, rect)?;
                    } else {
                        let port = graphics.get_port(&mut memory)?;
                        if port == 0 {
                            return Err("rectangle drawing without port".into());
                        }
                        let mode = if opcode == 0xa8a4 {
                            10
                        } else if opcode == 0xa8a2 {
                            memory.read_word(port + 56)
                        } else {
                            8
                        };
                        let mut pattern = [255; 8];
                        let pp = if opcode == 0xa8a5 {
                            Some(memory.read_long(sp))
                        } else if opcode == 0xa8a2 {
                            pattern = graphics.pattern(&mut memory, port, 58)?;
                            None
                        } else if opcode == 0xa8a3 {
                            pattern = graphics.pattern(&mut memory, port, 32)?;
                            None
                        } else {
                            None
                        };
                        if let Some(p) = pp {
                            for (i, b) in pattern.iter_mut().enumerate() {
                                *b = memory.read_byte(p + i as u32);
                            }
                        }
                        graphics.paint_rect(&mut memory, rect, pattern, mode)?;
                    }
                    cpu.dar[15] += if opcode == 0xa8a5 { 8 } else { 4 };
                    cpu.pc += 2;
                    continue;
                }
                0xa032 => {
                    events.flush(cpu.dar[0] as u16, (cpu.dar[0] >> 16) as u16);
                    cpu.dar[0] = 0;
                    cpu.pc += 2;
                    continue;
                }
                0xa970 | 0xa971 | 0xa860 => {
                    let sp = cpu.dar[15];
                    let (p, mask, pop) = if opcode == 0xa860 {
                        (memory.read_long(sp + 8), memory.read_word(sp + 12), 14)
                    } else {
                        (memory.read_long(sp), memory.read_word(sp + 4), 6)
                    };
                    if !events.next(mask, false).0 {
                        if let Some(mut event) = script.pop_front() {
                            event.when = events.ticks;
                            if event.what == 3 {
                                event.v = events.mouse.0;
                                event.h = events.mouse.1;
                            }
                            events.modifiers = event.modifiers;
                            if matches!(event.what, 0 | 1 | 2) {
                                events.set_mouse(&mut memory, (event.v, event.h));
                            }
                            if event.what != 0 {
                                events
                                    .post(event)
                                    .map_err(|e| format!("script event queue: {e}"))?;
                            }
                            println!(
                                "script event: what={} message={:#x} modifiers={:#x} point=({},{})",
                                event.what, event.message, event.modifiers, event.h, event.v
                            );
                        }
                    }
                    let (found, event) = windows.next_event(&mut memory,&mut events,mask,opcode != 0xa971)?;
                    if interactive && !found && script.is_empty() {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    memory.write_word(p, event.what);
                    memory.write_long(p + 2, event.message);
                    memory.write_long(p + 6, event.when);
                    memory.write_word(p + 10, event.v as u16);
                    memory.write_word(p + 12, event.h as u16);
                    memory.write_word(p + 14, event.modifiers);
                    memory.write_byte(sp + pop, found as u8);
                    cpu.dar[15] += pop;
                    cpu.pc += 2;
                    continue;
                }
                0xa8fe => {
                    fonts.initialize(&mut memory)?;
                    cpu.pc += 2;
                    continue;
                }
                0xa88b | 0xa88c | 0xa88d | 0xa886 => {
                    let sp = cpu.dar[15];
                    let port = graphics.get_port(&mut memory)?;
                    if port == 0 {
                        return Err("font operation without a current port".into());
                    }
                    let font = memory.read_word(port + 68) as i16;
                    let size = memory.read_word(port + 74) as i16;
                    let face = memory.read_byte(port + 70);
                    if opcode == 0xa88b {
                        let p = memory.read_long(sp);
                        let info = fonts.select(font, size, face)?.info;
                        for (i, v) in info.iter().enumerate() {
                            memory.write_word(p + 2 * i as u32, *v as u16);
                        }
                        cpu.dar[15] += 4;
                    } else {
                        let (text, argument_bytes) = match opcode {
                            0xa88d => (vec![memory.read_word(sp) as u8], 2),
                            0xa88c => {
                                let p = memory.read_long(sp);
                                let n = memory.read_byte(p) as u32;
                                ((0..n).map(|i| memory.read_byte(p + 1 + i)).collect(), 4)
                            }
                            0xa886 => {
                                let n = memory.read_word(sp) as u32;
                                let first = memory.read_word(sp + 2) as i16;
                                let p = memory.read_long(sp + 4).wrapping_add(first as i32 as u32);
                                ((0..n).map(|i| memory.read_byte(p + i)).collect(), 8)
                            }
                            _ => unreachable!(),
                        };
                        let width = fonts.text_width(font, size, face, &text)?;
                        memory.write_word(sp + argument_bytes, width as u16);
                        cpu.dar[15] += argument_bytes;
                    }
                    cpu.pc += 2;
                    continue;
                }
                0xa893 | 0xa89b => {
                    let sp = cpu.dar[15];
                    let v = memory.read_word(sp) as i16;
                    let h = memory.read_word(sp + 2) as i16;
                    if opcode == 0xa893 {
                        graphics.move_to(&mut memory, h, v)?;
                    } else {
                        graphics.pen_size(&mut memory, h, v)?;
                    }
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa89c | 0xa887 | 0xa888 | 0xa889 | 0xa88a => {
                    let sp = cpu.dar[15];
                    let value = memory.read_word(sp);
                    let offset = match opcode {
                        0xa89c => 56,
                        0xa887 => 68,
                        0xa888 => 70,
                        0xa889 => 72,
                        0xa88a => 74,
                        _ => unreachable!(),
                    };
                    if opcode == 0xa888 {
                        let port = graphics.get_port(&mut memory)?;
                        if port == 0 {
                            return Err("TextFace without current port".into());
                        }
                        let value = memory.read_byte(sp);
                        memory.write_byte(port + 70, value);
                    } else {
                        graphics.set_word(&mut memory, offset, value)?;
                    }
                    cpu.dar[15] += 2;
                    cpu.pc += 2;
                    continue;
                }
                0xa8aa => {
                    let sp = cpu.dar[15];
                    let destination = memory.read_long(sp);
                    let b = memory.read_long(sp + 4);
                    let a = memory.read_long(sp + 8);
                    let a = crate::quickdraw::read_rect(&mut memory, a);
                    let b = crate::quickdraw::read_rect(&mut memory, b);
                    let intersection = a.intersection(b);
                    crate::quickdraw::write_rect(
                        &mut memory,
                        destination,
                        intersection.unwrap_or(crate::managers::Rect {
                            top: 0,
                            left: 0,
                            bottom: 0,
                            right: 0,
                        }),
                    );
                    memory.write_byte(sp + 12, intersection.is_some() as u8);
                    cpu.dar[15] += 12;
                    cpu.pc += 2;
                    continue;
                }
                0xa880 => {
                    let sp = cpu.dar[15];
                    let v = memory.read_word(sp);
                    let h = memory.read_word(sp + 2);
                    let p = memory.read_long(sp + 4);
                    memory.write_word(p, v);
                    memory.write_word(p + 2, h);
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa8ad => {
                    let sp = cpu.dar[15];
                    let rect = memory.read_long(sp);
                    let rect = crate::quickdraw::read_rect(&mut memory, rect);
                    let v = memory.read_word(sp + 4) as i16;
                    let h = memory.read_word(sp + 6) as i16;
                    memory.write_byte(sp + 8, rect.contains(v, h) as u8);
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa86e => {
                    let slot = memory.read_long(cpu.dar[15]);
                    graphics.init(&mut memory, slot)?;
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa86f | 0xa874 | 0xa87d | 0xa873 => {
                    let sp = cpu.dar[15];
                    let p = memory.read_long(sp);
                    match opcode {
                        0xa86f => {
                            graphics.open_port(&mut memory, &mut heap, p)?;
                        }
                        0xa874 => {
                            let port = graphics.get_port(&mut memory)?;
                            memory.write_long(p, port);
                        }
                        0xa873 => {
                            graphics.set_port(&mut memory, p)?;
                        }
                        0xa87d => {
                            graphics.close_port(&mut memory, &mut heap, p)?;
                        }
                        _ => unreachable!(),
                    }
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa02e | 0xa22e => {
                    let source = cpu.dar[8] as usize;
                    let destination = cpu.dar[9] as usize;
                    let count = cpu.dar[0] as usize;
                    let end = source
                        .checked_add(count)
                        .ok_or("BlockMove source overflow")?;
                    let destination_end = destination
                        .checked_add(count)
                        .ok_or("BlockMove destination overflow")?;
                    if end > memory.bytes.len() || destination_end > memory.bytes.len() {
                        return Err("BlockMove outside application memory".into());
                    }
                    memory.bytes.copy_within(source..end, destination);
                    cpu.dar[0] = 0;
                    memory_result(&mut cpu, &mut memory, 0);
                    cpu.pc += 2;
                    continue;
                }
                0xa063 => {
                    // The initial application zone already reserves the full
                    // configured application heap, up to ApplLimit.
                    cpu.dar[0] = 0;
                    memory_result(&mut cpu, &mut memory, 0);
                    cpu.pc += 2;
                    continue;
                }
                0xa820 | 0xa9a1 => {
                    let sp = cpu.dar[15];
                    let nameptr = memory.read_long(sp);
                    let kind = memory.read_long(sp + 4).to_be_bytes();
                    let namelen = memory.read_byte(nameptr) as usize;
                    let name: Vec<u8> = (0..namelen)
                        .map(|i| memory.read_byte(nameptr + 1 + i as u32))
                        .collect();
                    let resource = resources.iter().find(|r| {
                        application_resources_visible
                            && r.kind == kind
                            && r.name == Some(name.as_slice())
                    });
                    let handle = if let Some(r) = resource {
                        load_resource(r, &mut memory, &mut heap, &mut resource_handles)?
                    } else {
                        0
                    };
                    memory.write_word(0xa60, if handle == 0 { (-192i16) as u16 } else { 0 });
                    memory.write_long(sp + 8, handle);
                    cpu.dar[15] += 8;
                    cpu.pc += 2;
                    continue;
                }
                0xa9f0 => {
                    let id = memory.read_word(cpu.dar[15]) as i16;
                    let resource = resources
                        .iter()
                        .find(|r| r.kind == *b"CODE" && r.id == id)
                        .ok_or_else(|| format!("LoadSeg missing CODE {id}"))?;
                    let h = load_resource(resource, &mut memory, &mut heap, &mut resource_handles)?;
                    segments.load(id, h, &mut memory, &mut heap)?;
                    println!(
                        "loaded CODE {id}: {} bytes at {:#x}",
                        heap.size(h).map_err(|e| e.to_string())? - 4,
                        heap.data(h).map_err(|e| e.to_string())? + 4
                    );
                    cpu.dar[15] += 2;
                    cpu.pc = pc - 4;
                    continue;
                }
                0xa9f1 => {
                    let pointer = memory.read_long(cpu.dar[15]);
                    segments.unload(pointer, &mut memory, &mut heap, &resource_handles)?;
                    cpu.dar[15] += 4;
                    cpu.pc += 2;
                    continue;
                }
                0xa069 | 0xa06a | 0xa049 | 0xa04a => {
                    handle_state_trap(opcode, &mut cpu, &mut memory, &mut heap);
                    continue;
                }
                0xa346 | 0xa146 | 0xa746 => {
                    let requested=cpu.dar[0] as u16;
                    cpu.dar[8] = if opcode == 0xa746 && cpu.dar[0] as u16 == 0xa81a {
                        0x18300
                    } else if opcode!=0xa746 && cpu.dar[0] as u16 & 0xff == 0xad {
                        0x18000
                    } else {
                        0x18100
                    };
                    if args.iter().any(|a|a=="--trace-sound") {println!("discovery: trap_address={requested:#06x} address={:#x}",cpu.dar[8]);}
                    cpu.pc += 2;
                    continue;
                }
                _ => {}
            }
            if let Some(path) = capture {
                capture_framebuffer(&mut memory, &graphics, path)?;
            }
            if let Some(path) = dump_documents {
                std::fs::write(path, textedit.documents(&mut memory)?.to_string())
                    .map_err(|e| e.to_string())?;
            }
            return Err(format!("unimplemented Macintosh trap {opcode:#06x} at PC {pc:#x} after {instruction} execution iterations; D0={:#x} A0={:#x} SP={:#x}",cpu.dar[0],cpu.dar[8],cpu.dar[15]));
        }
        match cpu.step(&mut memory) {
            StepResult::Ok { .. } => {}
            other => return Err(format!("execution stopped at {pc:#x}: {other:?}")),
        }
    }
    if let Some(path) = capture {
        capture_framebuffer(&mut memory, &graphics, path)?;
    }
    if let Some(path) = dump_documents {
        std::fs::write(path, textedit.documents(&mut memory)?.to_string())
            .map_err(|e| e.to_string())?;
    }
    if interactive {
        Ok(())
    } else {
        Err("instruction limit reached; inspect the trace for delivered events and unsupported behavior".into())
    }
}

/// Replay the exact core-v1 experiment against the same managers used by traps.
pub fn core_probe() -> Result<String, String> {
    let mut memory = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    classic_core::core_probe(&mut memory).map_err(|e| format!("core experiment: {e}"))
}

#[cfg(test)]
mod manager_tests {
    use super::*;
    #[test]
    fn register_hook_runs_cpu_and_returns_d0_without_losing_application_registers() {
        let mut m = Memory {
            bytes: vec![0; 0x20000],
        };
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(CpuType::M68020);
        cpu.pc = 0x4000;
        cpu.dar[15] = 0x10000;
        cpu.dar[2] = 0x12345678;
        cpu.dar[13] = 0x7000;
        // move.l a3,$3000; moveq #1,d0; rts. Then repeat with false.
        for result in [1u16, 0] {
            for (i, word) in [0x21cb, 0x3000, 0x7000 | result, 0x4e75].iter().enumerate() {
                m.write_word(0x2000 + i as u32 * 2, *word);
            }
            let before = cpu.dar;
            let mut callbacks = crate::callbacks::Callbacks::new();
            callbacks
                .call_register(&mut cpu, &mut m, 0x2000, &[(11, 0x5678), (12, 0x9876)], 2)
                .unwrap();
            for _ in 0..3 {
                assert!(matches!(cpu.step(&mut m), StepResult::Ok { .. }));
            }
            assert_eq!(
                callbacks.finish(&mut cpu, &mut m).unwrap(),
                Some((2, result as i16))
            );
            assert_eq!(m.read_long(0x3000), 0x5678);
            assert_eq!(cpu.dar, before);
            assert_eq!(cpu.pc, 0x4000);
        }
    }
    #[test]
    fn menu_tracking_respects_disabled_items_and_restores_pixels_on_cancel() {
        use crate::{
            events::Events, fonts::Fonts, managers::Heap, menus::Menus, quickdraw::QuickDraw,
            windows::Windows,
        };
        let mut m = Memory {
            bytes: vec![0; 8 * 1024 * 1024],
        };
        let mut heap = Heap::new(0x20000, 0xe0000);
        let mut q = QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let mut fonts = Fonts::new();
        fonts.init().unwrap();
        let mut events = Events::new();
        let mut windows = Windows::new();
        windows.init(&mut m, &mut heap, &q).unwrap();
        let manager = windows.manager_port.unwrap();
        let mut bytes = vec![0; 14];
        bytes[0..2].copy_from_slice(&128u16.to_be_bytes());
        bytes[10..14].copy_from_slice(&3u32.to_be_bytes());
        bytes.extend_from_slice(b"\x04File\x03New\0\0\0\0\x08Disabled\0\0\0\0\0");
        let handle = heap.new_handle(&mut m, bytes.len() as u32, true).unwrap();
        let p = m.read_long(handle);
        m.put(p as usize, &bytes).unwrap();
        let mut menus = Menus::new();
        menus.init(&mut m, &mut heap).unwrap();
        menus.calculate(&mut m, &mut heap, &fonts, handle).unwrap();
        menus.insert(&mut m, &mut heap, handle, 0).unwrap();
        menus.draw(&mut m, &q, &fonts, manager).unwrap();
        let old_port = q.get_port(&mut m).unwrap();
        let before: Vec<_> = (0..q.row_bytes() as u32 * q.height as u32)
            .map(|i| m.read_byte(q.screen_base + i))
            .collect();
        events.set_mouse(&mut m, (10, 20));
        events.modifiers = 0;
        assert_eq!(
            menus
                .track(
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    manager,
                    (10, 20),
                    &events,
                    false
                )
                .unwrap(),
            None
        );
        events.modifiers = 0x80;
        assert_eq!(
            menus
                .track(
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    manager,
                    (10, 20),
                    &events,
                    false
                )
                .unwrap(),
            None
        );
        events.set_mouse(&mut m, (48, 30));
        events.modifiers = 0;
        assert_eq!(
            menus
                .track(
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    manager,
                    (10, 20),
                    &events,
                    false
                )
                .unwrap(),
            None
        );
        events.modifiers = 0x80;
        assert_eq!(
            menus
                .track(
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    manager,
                    (10, 20),
                    &events,
                    false
                )
                .unwrap(),
            Some(0)
        );
        assert_eq!(q.get_port(&mut m).unwrap(), old_port);
        assert_eq!(
            (0..before.len())
                .map(|i| m.read_byte(q.screen_base + i as u32))
                .collect::<Vec<_>>(),
            before
        );
        events.set_mouse(&mut m, (10, 20));
        events.modifiers = 0;
        menus
            .track(
                &mut m,
                &mut heap,
                &q,
                &fonts,
                manager,
                (10, 20),
                &events,
                false,
            )
            .unwrap();
        events.set_mouse(&mut m, (30, 30));
        events.modifiers = 0x80;
        assert_eq!(
            menus
                .track(
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    manager,
                    (10, 20),
                    &events,
                    false
                )
                .unwrap(),
            Some(0x00800001)
        );
        assert_eq!(m.read_word(0xa26), 128);
    }
    use crate::managers::Heap;
    #[test]
    fn pascal_callback_runs_cpu_code_preserves_caller_and_keeps_memory_changes() {
        use crate::callbacks::{Arg, Callbacks, RETURN};
        let mut m = Memory {
            bytes: vec![0; 0x20000],
        };
        // move.w 12(sp),d0; addi.w #7,d0; move.w d0,14(sp);
        // move.l #$12345678,$3000; move.l (sp)+,a0; lea 10(sp),sp; jmp (a0).
        let words: [u16; 14] = [
            0x302f, 0x000c, 0x0640, 0x0007, 0x3f40, 0x000e, 0x21fc, 0x1234, 0x5678, 0x3000, 0x205f,
            0x4fef, 0x000a, 0x4ed0,
        ];
        for (i, w) in words.iter().enumerate() {
            m.write_word(0x1000 + i as u32 * 2, *w);
        }
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(CpuType::M68020);
        cpu.pc = 0x2200;
        cpu.dar = [0xdeadbeef; 16];
        cpu.dar[15] = 0x8000;
        cpu.set_sr_noint_nosp(0x2015);
        let registers = cpu.dar;
        let sr = cpu.get_sr();
        let mut callbacks = Callbacks::new();
        callbacks
            .call_word(
                &mut cpu,
                &mut m,
                0x1000,
                &[Arg::Word(5), Arg::Long(0x4000), Arg::Long(0)],
                71,
            )
            .unwrap();
        for _ in 0..30 {
            if cpu.pc == RETURN {
                break;
            }
            assert!(matches!(cpu.step(&mut m), StepResult::Ok { .. }));
        }
        assert_eq!(callbacks.finish(&mut cpu, &mut m).unwrap(), Some((71, 12)));
        assert_eq!(cpu.dar, registers);
        assert_eq!(cpu.pc, 0x2200);
        assert_eq!(cpu.get_sr(), sr);
        assert_eq!(m.read_long(0x3000), 0x12345678);
        cpu.pc = RETURN;
        assert!(callbacks.finish(&mut cpu, &mut m).is_err());
    }
    #[test]
    fn high_level_file_traps_marshal_fsspec_results_and_pascal_arguments() {
        let mut m = Memory {
            bytes: vec![0; 0x10000],
        };
        let mut files = crate::files::Files::new();
        let mut cpu = CpuCore::new();
        m.put(0x1100, b"\x06Sample").unwrap();
        // FSMakeFSSpec fills the output even when the leaf does not exist.
        cpu.dar[15] = 0x8000;
        cpu.dar[0] = 1;
        m.write_long(0x8000, 0x1000);
        m.write_long(0x8004, 0x1100);
        m.write_long(0x8008, 2);
        m.write_word(0x800c, 0xffff);
        assert!(files.dispatch(0xaa52, &mut cpu, &mut m).unwrap());
        assert_eq!(cpu.dar[15], 0x800e);
        assert_eq!(m.read_word(0x800e) as i16, -43);
        assert_eq!(m.read_word(0x1000), 0xffff);
        assert_eq!(m.read_long(0x1002), 2);
        assert_eq!(m.read_byte(0x1006), 6);
        cpu.dar[15] = 0x8000;
        cpu.dar[0] = 4;
        m.write_word(0x8000, 0);
        m.write_long(0x8002, u32::from_be_bytes(*b"TEXT"));
        m.write_long(0x8006, u32::from_be_bytes(*b"ttxt"));
        m.write_long(0x800a, 0x1000);
        files.dispatch(0xaa52, &mut cpu, &mut m).unwrap();
        assert_eq!(cpu.dar[15], 0x800e);
        assert_eq!(m.read_word(0x800e), 0);
        cpu.dar[15] = 0x8000;
        cpu.dar[0] = 2;
        m.write_long(0x8000, 0x2000);
        m.write_byte(0x8004, 3);
        m.write_byte(0x8005, 0xa5);
        m.write_long(0x8006, 0x1000);
        files.dispatch(0xaa52, &mut cpu, &mut m).unwrap();
        assert_eq!(cpu.dar[15], 0x800a);
        assert_eq!(m.read_word(0x800a), 0);
        let reference = m.read_word(0x2000) as i16;
        assert_eq!(files.write(reference, b"abc"), Ok(3));
        files.close(reference).unwrap();
        cpu.dar[15] = 0x8000;
        cpu.dar[0] = 7;
        m.write_long(0x8000, 0x3000);
        m.write_long(0x8004, 0x1000);
        files.dispatch(0xaa52, &mut cpu, &mut m).unwrap();
        assert_eq!(cpu.dar[15], 0x8008);
        assert_eq!(m.read_word(0x8008), 0);
        assert_eq!(&m.bytes[0x3000..0x3008], b"TEXTttxt");
    }
    #[test]
    fn changing_window_title_preserves_document_pixels_and_port() {
        use crate::{managers::Rect, quickdraw::QuickDraw};
        let mut m = Memory {
            bytes: vec![0; 8 * 1024 * 1024],
        };
        let mut heap = Heap::new(0x20000, 0xe0000);
        let mut q = QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let mut fonts = crate::fonts::Fonts::new();
        fonts.init().unwrap();
        let mut events = crate::events::Events::new();
        let mut ws = crate::windows::Windows::new();
        ws.init(&mut m, &mut heap, &q).unwrap();
        let w = ws
            .create(
                &mut m,
                &mut heap,
                &q,
                &fonts,
                &mut events,
                0,
                Rect {
                    top: 60,
                    left: 50,
                    bottom: 180,
                    right: 250,
                },
                b"Old",
                true,
                0,
                u32::MAX,
                true,
                0,
            )
            .unwrap();
        q.set_port(&mut m, w).unwrap();
        q.paint_rect(
            &mut m,
            Rect {
                top: 20,
                left: 20,
                bottom: 30,
                right: 40,
            },
            [255; 8],
            8,
        )
        .unwrap();
        fn content(m: &mut Memory) -> Vec<bool> {
            let mut pixels = Vec::new();
            for y in 60..180 {
                for x in 50..250 {
                    pixels.push(m.read_byte(0x500000 + y * 80 + x / 8) & (0x80 >> (x & 7)) != 0);
                }
            }
            pixels
        }
        let before = content(&mut m);
        assert!(before.iter().any(|b| *b));
        ws.set_title(&mut m, &mut heap, &q, &fonts, w, b"A different title")
            .unwrap();
        assert_eq!(content(&mut m), before);
        assert_eq!(q.get_port(&mut m).unwrap(), w);
        let vis = m.read_long(w + 24);
        let original = crate::regions::Region::read(&mut m, vis).unwrap();
        ws.invalidate(
            &mut m,
            &mut heap,
            &q,
            &mut events,
            Rect {
                top: 10,
                left: 10,
                bottom: 40,
                right: 50,
            },
        )
        .unwrap();
        ws.begin_update(&mut m, &mut heap, &q, w).unwrap();
        assert!(ws.begin_update(&mut m, &mut heap, &q, w).is_err());
        ws.end_update(&mut m, &mut heap, w).unwrap();
        assert_eq!(crate::regions::Region::read(&mut m, vis).unwrap(), original);
        assert!(ws.end_update(&mut m, &mut heap, w).is_err());
    }
    #[test]
    fn oversized_textedit_insert_and_paste_preserve_document_and_selection() {
        let mut m = Memory {
            bytes: vec![0; 8 * 1024 * 1024],
        };
        let mut heap = Heap::new(0x20000, 0xe0000);
        let mut q = crate::quickdraw::QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let port = heap.new_ptr(&mut m, 108, true).unwrap();
        q.open_port(&mut m, &mut heap, port).unwrap();
        let mut fonts = crate::fonts::Fonts::new();
        fonts.init().unwrap();
        let mut te = crate::textedit::TextEdit::new();
        let r = crate::managers::Rect {
            top: 0,
            left: 0,
            bottom: 100,
            right: 200,
        };
        let h = te.create(&mut m, &mut heap, &q, &fonts, r, r).unwrap();
        te.set_text(&mut m, &mut heap, &fonts, h, b"keep").unwrap();
        te.select(&mut m, h, 1, 3).unwrap();
        assert!(te
            .insert(&mut m, &mut heap, &fonts, h, &vec![b'x'; 32768])
            .is_err());
        assert_eq!(te.text(&mut m, h).unwrap(), b"keep");
        let p = m.read_long(h);
        assert_eq!((m.read_word(p + 32), m.read_word(p + 34)), (1, 3));
        let scrap = crate::scrap::init_te(&mut m, &mut heap).unwrap();
        heap.resize(&mut m, scrap, 32767).unwrap();
        let data = m.read_long(scrap);
        for i in 0..32767 {
            m.write_byte(data + i, b'x');
        }
        m.write_word(0xab0, 32767);
        assert!(te.paste(&mut m, &mut heap, &fonts, h).is_err());
        assert_eq!(te.text(&mut m, h).unwrap(), b"keep");
        let p = m.read_long(h);
        assert_eq!((m.read_word(p + 32), m.read_word(p + 34)), (1, 3));
    }
    #[test]
    fn textedit_update_preserves_callers_port_and_complex_clip() {
        use crate::{
            managers::Rect,
            regions::{Operation, Region},
        };
        let mut m = Memory {
            bytes: vec![0; 8 * 1024 * 1024],
        };
        let mut heap = Heap::new(0x20000, 0xe0000);
        let mut q = crate::quickdraw::QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let port = heap.new_ptr(&mut m, 108, true).unwrap();
        q.open_port(&mut m, &mut heap, port).unwrap();
        let mut fonts = crate::fonts::Fonts::new();
        fonts.init().unwrap();
        let mut te = crate::textedit::TextEdit::new();
        let r = Rect {
            top: 0,
            left: 0,
            bottom: 100,
            right: 200,
        };
        let h = te.create(&mut m, &mut heap, &q, &fonts, r, r).unwrap();
        te.set_text(&mut m, &mut heap, &fonts, h, b"hello\rthere")
            .unwrap();
        let clip = m.read_long(port + 28);
        let shape = Region::rectangle(r).combine(
            &Region::rectangle(Rect {
                top: 5,
                left: 10,
                bottom: 40,
                right: 25,
            }),
            Operation::Difference,
        );
        shape.store(&mut m, &mut heap, clip).unwrap();
        let before: Vec<u8> = (0..108).map(|i| m.read_byte(port + i)).collect();
        let other = heap.new_ptr(&mut m, 108, true).unwrap();
        q.open_port(&mut m, &mut heap, other).unwrap();
        te.update(&mut m, &mut heap, &q, &fonts, h, r).unwrap();
        assert_eq!(q.get_port(&mut m).unwrap(), other);
        assert_eq!(Region::read(&mut m, clip).unwrap(), shape);
        assert_eq!(
            (0..108).map(|i| m.read_byte(port + i)).collect::<Vec<_>>(),
            before
        );
        assert!(m.bytes[0x500000..0x500000 + 80 * 100]
            .iter()
            .any(|b| *b != 0));
    }
    #[test]
    fn complex_region_boolean_operations_and_native_membership() {
        use crate::{
            managers::Rect,
            regions::{Operation, Region},
        };
        let outer = Rect {
            top: -8,
            left: -10,
            bottom: 12,
            right: 14,
        };
        let hole = Rect {
            top: -2,
            left: -3,
            bottom: 6,
            right: 7,
        };
        let cutter = Rect {
            top: 0,
            left: 0,
            bottom: 16,
            right: 20,
        };
        let ring =
            Region::rectangle(outer).combine(&Region::rectangle(hole), Operation::Difference);
        let mut m = Memory {
            bytes: vec![0; 0x10000],
        };
        let mut heap = Heap::new(0x1000, 0xf000);
        let h = Region::new_handle(&mut m, &mut heap).unwrap();
        for op in [
            Operation::Intersection,
            Operation::Union,
            Operation::Difference,
            Operation::Xor,
        ] {
            let shape = ring.combine(&Region::rectangle(cutter), op);
            shape.store(&mut m, &mut heap, h).unwrap();
            assert_eq!(Region::read(&mut m, h).unwrap(), shape);
            for y in -12..20 {
                for x in -14..24 {
                    let a = outer.contains(y, x) && !hole.contains(y, x);
                    let b = cutter.contains(y, x);
                    let expected = match op {
                        Operation::Intersection => a && b,
                        Operation::Union => a || b,
                        Operation::Difference => a && !b,
                        Operation::Xor => a ^ b,
                    };
                    assert_eq!(shape.contains(y, x), expected, "shape at ({x},{y})");
                    assert_eq!(
                        crate::regions::contains(&mut m, h, y, x).unwrap(),
                        expected,
                        "native at ({x},{y})"
                    );
                }
            }
        }
    }
    #[test]
    fn failed_growth_preserves_handle_and_contents() {
        let mut m = Memory {
            bytes: vec![0; 0x4000],
        };
        let mut heap = Heap::new(0x1000, 0x2000);
        let h = heap.new_handle(&mut m, 16, false).unwrap();
        let original = heap.data(h).unwrap();
        m.put(original as usize, b"preserve sixteen").unwrap();
        assert_eq!(heap.resize(&mut m, h, u32::MAX), Err(-108));
        assert_eq!(heap.data(h).unwrap(), original);
        assert_eq!(heap.size(h).unwrap(), 16);
        assert_eq!(
            &m.bytes[original as usize..original as usize + 16],
            b"preserve sixteen"
        );
    }
    #[test]
    fn locked_handle_cannot_move_past_another_allocation() {
        let mut m = Memory {
            bytes: vec![0; 0x4000],
        };
        let mut heap = Heap::new(0x1000, 0x2000);
        let h = heap.new_handle(&mut m, 16, false).unwrap();
        let p = heap.data(h).unwrap();
        let blocker = heap.new_handle(&mut m, 16, false).unwrap();
        heap.lock(h, true).unwrap();
        assert_eq!(heap.resize(&mut m, h, 64), Err(-108));
        assert_eq!(heap.data(h).unwrap(), p);
        heap.lock(h, false).unwrap();
        heap.resize(&mut m, h, 64).unwrap();
        assert_ne!(heap.data(h).unwrap(), p);
        assert_eq!(heap.size(blocker).unwrap(), 16);
    }
}

pub fn quickdraw_probe() -> Result<String, String> {
    use crate::quickdraw::{read_rect, QuickDraw};
    use serde_json::json;
    fn rectangle(r: crate::managers::Rect) -> serde_json::Value {
        json!([r.top, r.left, r.bottom, r.right])
    }
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    let slot = 0x100000;
    q.init(&mut m, slot)?;
    let initial = q.get_port(&mut m)?;
    let screen_bounds = read_rect(&mut m, slot - 116);
    let globals = json!({"screen_bounds":rectangle(screen_bounds),"row_bytes":m.read_word(slot-118),"screen_nonnull":(m.read_long(slot-122)!=0) as u8,"port_nonnull":(initial!=0) as u8});
    let p = heap.new_ptr(&mut m, 108, true).map_err(|e| e.to_string())?;
    q.open_port(&mut m, &mut heap, p)?;
    let vis = m.read_long(p + 24);
    let clip = m.read_long(p + 28);
    let vis_data = m.read_long(vis);
    let clip_data = m.read_long(clip);
    let pen_pattern: Vec<u8> = (0..8).map(|i| m.read_byte(p + 58 + i)).collect();
    let port = json!({
        "device":m.read_word(p),"bounds":rectangle(read_rect(&mut m,p+16)),"row_bytes":m.read_word(p+6),
        "pen":[m.read_word(p+48),m.read_word(p+50)],"pen_size":[m.read_word(p+52),m.read_word(p+54)],
        "pen_mode":m.read_word(p+56),"pen_visibility":m.read_word(p+66),
        "text_font":m.read_word(p+68),"text_face":m.read_byte(p+70),"text_mode":m.read_word(p+72),"text_size":m.read_word(p+74),
        "foreground":m.read_long(p+80),"background":m.read_long(p+84),"pen_pattern":pen_pattern,
        "visible_bounds":rectangle(read_rect(&mut m,vis_data+2)),"clip_bounds":rectangle(read_rect(&mut m,clip_data+2))
    });
    q.move_to(&mut m, 12, 34)?;
    q.pen_size(&mut m, 3, 4)?;
    q.set_word(&mut m, 56, 10)?;
    q.set_word(&mut m, 68, 3)?;
    q.set_word(&mut m, 74, 9)?;
    q.set_word(&mut m, 72, 0)?;
    let changed = json!({"pen":[m.read_word(p+48),m.read_word(p+50)],"pen_size":[m.read_word(p+52),m.read_word(p+54)],"pen_mode":m.read_word(p+56),"text_font":m.read_word(p+68),"text_size":m.read_word(p+74),"text_mode":m.read_word(p+72)});
    q.close_port(&mut m, &mut heap, p)?;
    q.set_port(&mut m, initial)?;
    Ok(
        json!({"schema":1,"probe":"quickdraw-v1","globals":globals,"port":port,"changed":changed})
            .to_string(),
    )
}

pub fn window_probe() -> Result<String, String> {
    use crate::managers::Rect;
    use crate::quickdraw::{read_rect, QuickDraw};
    use serde_json::json;
    fn r(r: Rect) -> serde_json::Value {
        json!([r.top, r.left, r.bottom, r.right])
    }
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let before = q.get_port(&mut m)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 180,
            right: 250,
        },
        b"Reference",
        true,
        0,
        u32::MAX,
        true,
        0x12345678,
    )?;
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--capture-framebuffer") {
        capture_framebuffer(
            &mut m,
            &q,
            args.get(i + 1).ok_or("missing framebuffer filename")?,
        )?;
    }
    let after = q.get_port(&mut m)?;
    let sr = m.read_long(w + 114);
    let sr = m.read_long(sr);
    let cr = m.read_long(w + 118);
    let cr = m.read_long(cr);
    let ur = m.read_long(w + 122);
    let ur = m.read_long(ur);
    let result = json!({"schema":1,"probe":"windows-v1","initial":{"port_nonnull":(before!=0) as u8},"window":{
        "kind":m.read_word(w+108),"visible":m.read_byte(w+110),"hilited":m.read_byte(w+111),"go_away":m.read_byte(w+112),"refcon":m.read_long(w+152),
        "port_bounds":r(read_rect(&mut m,w+16)),"bitmap_bounds":r(read_rect(&mut m,w+8)),"structure_bounds":r(read_rect(&mut m,sr+2)),"content_bounds":r(read_rect(&mut m,cr+2)),"update_bounds":r(read_rect(&mut m,ur+2)),"title_width":m.read_word(w+138),"front_matches":(windows.front()==w) as u8,"current_matches":(after==w) as u8
    }});
    windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
    q.set_port(&mut m, before)?;
    Ok(result.to_string())
}

fn capture_drawing(m:&mut Memory,q:&crate::quickdraw::QuickDraw,directory:&Option<String>,sequence:&mut usize,opcode:u16,pc:u32) -> Result<(),String> {
    if let Some(directory)=directory {
        if *sequence<32 {
            let path=std::path::Path::new(directory).join(format!("picture-frame-{:03}-{opcode:04x}.ppm",*sequence));
            capture_framebuffer(m,q,path.to_str().ok_or("non-UTF8 drawing capture path")?)?;
            eprintln!("drawing framebuffer: sequence={} opcode={opcode:#06x} pc={pc:#x} file={}",*sequence,path.display());
            *sequence+=1;
        }
    }
    Ok(())
}
fn capture_framebuffer(
    m: &mut Memory,
    q: &crate::quickdraw::QuickDraw,
    path: &str,
) -> Result<(), String> {
    let mut ppm = format!("P6\n{} {}\n255\n", q.width, q.height).into_bytes();
    let palette = q.screen_colors(m)?;
    for y in 0..q.height as u32 {
        for x in 0..q.width as u32 {
            let byte = m.read_byte(
                q.screen_base + y * q.row_bytes() as u32 + if q.depth == 8 { x } else { x / 8 },
            );
            let index = if q.depth == 8 {
                byte
            } else {
                u8::from(byte & (0x80 >> (x & 7)) != 0)
            };
            let color = palette[index as usize];
            ppm.extend_from_slice(&[(color >> 16) as u8, (color >> 8) as u8, color as u8]);
        }
    }
    std::fs::write(path, ppm).map_err(|e| e.to_string())
}

pub fn memory_flags_probe() -> Result<String, String> {
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut cpu = CpuCore::new();
    let small = heap
        .new_handle(&mut m, 16, false)
        .map_err(|e| e.to_string())?;
    cpu.dar[0] = heap.error as i32 as u32;
    memory_result(&mut cpu, &mut m, heap.error);
    let small_allocation = json!([cpu.flag_z() as u8, cpu.flag_n() as u8]);
    let size = heap.size(small).map_err(|e| e.to_string())?;
    cpu.dar[0] = size;
    memory_result(&mut cpu, &mut m, heap.error);
    let small_size = json!([size, cpu.flag_z() as u8, cpu.flag_n() as u8]);
    let large = heap
        .new_handle(&mut m, 65536, false)
        .map_err(|e| e.to_string())?;
    cpu.dar[0] = heap.error as i32 as u32;
    memory_result(&mut cpu, &mut m, heap.error);
    let large_allocation = json!([cpu.flag_z() as u8, cpu.flag_n() as u8]);
    let size = heap.size(large).map_err(|e| e.to_string())?;
    cpu.dar[0] = size;
    memory_result(&mut cpu, &mut m, heap.error);
    let large_size = json!([size, cpu.flag_z() as u8, cpu.flag_n() as u8]);
    let failed = heap.new_handle(&mut m, 0x7fffffff, false).unwrap_or(0);
    cpu.dar[0] = heap.error as i32 as u32;
    memory_result(&mut cpu, &mut m, heap.error);
    let failure = json!([cpu.dar[0] as i32, cpu.flag_z() as u8, cpu.flag_n() as u8]);
    heap.dispose(&mut m, small).map_err(|e| e.to_string())?;
    heap.dispose(&mut m, large).map_err(|e| e.to_string())?;
    Ok(json!({"schema":1,"probe":"memory-flags-v1","flags":{"small_allocation":small_allocation,"small_size":small_size,"large_allocation":large_allocation,"large_size":large_size,"failure":failure,"failure_null":(failed==0) as u8}}).to_string())
}

pub fn region_probe() -> Result<String, String> {
    use crate::{
        managers::Rect,
        regions::{Operation, Region},
    };
    use serde_json::json;
    let a = Region::rectangle(Rect {
        top: 1,
        left: 2,
        bottom: 8,
        right: 9,
    });
    let b = Region::rectangle(Rect {
        top: 4,
        left: 6,
        bottom: 12,
        right: 14,
    });
    let union = a.combine(&b, Operation::Union);
    let intersection = a.combine(&b, Operation::Intersection);
    let difference = a.combine(&b, Operation::Difference);
    let xor = a.combine(&b, Operation::Xor);
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let h = Region::new_handle(&mut m, &mut heap)?;
    union.store(&mut m, &mut heap, h)?;
    let inside = crate::regions::contains(&mut m, h, 5, 7)?;
    let outside = crate::regions::contains(&mut m, h, 10, 3)?;
    if Region::read(&mut m, h)? != union {
        return Err("region encoding round trip changed its geometry".into());
    }
    Ok(json!({"schema":1,"probe":"regions-v1","regions":{
        "empty":Region::default().words()?,"union":union.words()?,"inside_union":inside as u8,"outside_union":outside as u8,
        "intersection":intersection.words()?,"difference":difference.words()?,"xor":xor.words()?,"offset_xor":xor.offset(-5,3)?.words()?,
        "inverted":Region::rectangle(Rect{top:8,left:9,bottom:1,right:2}).words()?
    }}).to_string())
}

pub fn color_port_probe() -> Result<String, String> {
    use crate::{managers::Rect, quickdraw::read_rect};
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let before = q.get_port(&mut m)?;
    let p = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 180,
            right: 250,
        },
        b"Reference",
        true,
        0,
        u32::MAX,
        true,
        0x12345678,
    )?;
    q.make_color(&mut m, &mut heap, p)?;
    let after = q.get_port(&mut m)?;
    fn r(r: Rect) -> [i16; 4] {
        [r.top, r.left, r.bottom, r.right]
    }
    let bitmap = q.port_bitmap(&mut m, p)?;
    let mut pats = Vec::new();
    for offset in [32, 58, 62] {
        let h = m.read_long(p + offset);
        let d = m.read_long(h);
        pats.push(m.read_word(d));
    }
    let result = json!({"schema":1,"probe":"color-ports-v1","port":{
        "version":m.read_word(p+6),"rect":r(read_rect(&mut m,p+16)),"bitmap_bounds":r(read_rect(&mut m,bitmap+6)),
        "fg":[m.read_word(p+36),m.read_word(p+38),m.read_word(p+40)],"bg":[m.read_word(p+42),m.read_word(p+44),m.read_word(p+46)],
        "pen":[m.read_word(p+48),m.read_word(p+50),m.read_word(p+52),m.read_word(p+54),m.read_word(p+56),m.read_word(p+66)],
        "text":[m.read_word(p+68),m.read_byte(p+70),m.read_word(p+72),m.read_word(p+74)],
        "extras":[m.read_word(p+12) as i16,m.read_word(p+14) as i16,m.read_long(p+76) as i32],"patterns":pats,"current_unchanged":(after==before) as u8,"grafvars_nonnull":(m.read_long(p+8)!=0) as u8
    }});
    windows.dispose(&mut m, &mut heap, &q, &mut events, p)?;
    q.set_port(&mut m, before)?;
    Ok(result.to_string())
}

pub fn textedit_probe() -> Result<String, String> {
    textedit_replay(false)
}
pub fn textlayout_probe() -> Result<String, String> {
    textedit_replay(true)
}
fn textedit_replay(layout: bool) -> Result<String, String> {
    use crate::managers::Rect;
    use serde_json::{json, Map};
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 240,
            right: 350,
        },
        b"Text",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    q.set_port(&mut m, w)?;
    q.set_word(&mut m, 68, 1)?;
    q.set_word(&mut m, 74, 12)?;
    let mut te = crate::textedit::TextEdit::new();
    let rect = Rect {
        top: 4,
        left: 4,
        bottom: 164,
        right: if layout { 64 } else { 284 },
    };
    let h = te.create(&mut m, &mut heap, &q, &fonts, rect, rect)?;
    let mut observations = Map::new();
    if layout {
        for (name, text) in [
            ("words", "one two three four"),
            ("unbroken", "abcdefghijklmnop"),
            ("return_end", "a\r"),
            ("return_only", "\r"),
            ("empty_line", "a\r\rb"),
            ("spaces", "one   two   three"),
            ("space_overflow", "abcdefghij   klm"),
        ] {
            te.set_text(&mut m, &mut heap, &fonts, h, text.as_bytes())?;
            observations.insert(name.into(), textedit_snapshot(&mut m, h));
        }
        te.set_text(&mut m, &mut heap, &fonts, h, b"abc")?;
        te.activate(&mut m, h, true)?;
        observations.insert("active".into(), textedit_snapshot(&mut m, h));
        te.activate(&mut m, h, false)?;
        observations.insert("inactive".into(), textedit_snapshot(&mut m, h));
        te.dispose(&mut m, &mut heap, h)?;
        windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
        return Ok(
            json!({"schema":1,"probe":"text-layout-v1","textedit":observations}).to_string(),
        );
    }
    observations.insert("initial".into(), textedit_snapshot(&mut m, h));
    te.set_text(&mut m, &mut heap, &fonts, h, b"one two\rthree")?;
    observations.insert("set".into(), textedit_snapshot(&mut m, h));
    te.select(&mut m, h, 4, 7)?;
    te.insert(&mut m, &mut heap, &fonts, h, b"X")?;
    observations.insert("insert".into(), textedit_snapshot(&mut m, h));
    te.key(&mut m, &mut heap, &fonts, h, 8)?;
    observations.insert("backspace".into(), textedit_snapshot(&mut m, h));
    te.key(&mut m, &mut heap, &fonts, h, b'Z')?;
    observations.insert("key".into(), textedit_snapshot(&mut m, h));
    te.select(&mut m, h, 0, 32767)?;
    observations.insert("select_all".into(), textedit_snapshot(&mut m, h));
    te.delete(&mut m, &mut heap, &fonts, h)?;
    observations.insert("delete".into(), textedit_snapshot(&mut m, h));
    te.dispose(&mut m, &mut heap, h)?;
    windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
    Ok(json!({"schema":1,"probe":"textedit-v1","textedit":observations}).to_string())
}

pub fn control_probe() -> Result<String, String> {
    use crate::{
        controls::Controls,
        managers::{Heap, Rect},
        quickdraw::{read_rect, QuickDraw},
    };
    use serde_json::{json, Map, Value};
    fn snapshot(m: &mut Memory, h: u32, w: u32) -> Value {
        let p = m.read_long(h);
        let r = read_rect(m, p + 8);
        json!({"rect":[r.top,r.left,r.bottom,r.right],"visible":m.read_byte(p+16),"hilite":m.read_byte(p+17),"range":[m.read_word(p+18) as i16,m.read_word(p+20) as i16,m.read_word(p+22) as i16],"refcon":m.read_long(p+36),"owner_matches":(m.read_long(p+4)==w) as u8,"head_matches":(m.read_long(w+140)==h) as u8,"title_length":m.read_byte(p+40)})
    }
    fn call(
        c: &mut Controls,
        m: &mut Memory,
        heap: &mut Heap,
        q: &QuickDraw,
        op: u16,
        args: &[(u32, usize)],
    ) -> Result<(), String> {
        let mut bytes = Vec::new();
        for (value, size) in args {
            match size {
                2 => bytes.extend((*value as u16).to_be_bytes()),
                4 => bytes.extend(value.to_be_bytes()),
                _ => return Err("invalid probe argument width".into()),
            }
        }
        m.put(0x7e0000, &bytes)?;
        let mut cpu = CpuCore::new();
        cpu.dar[15] = 0x7e0000;
        if !c.dispatch(op, &mut cpu, m, heap, q)? {
            return Err("control probe trap not handled".into());
        }
        if cpu.dar[15] != 0x7e0000 + bytes.len() as u32 {
            return Err("control probe trap stack imbalance".into());
        }
        Ok(())
    }
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 240,
            right: 350,
        },
        b"Controls",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    let mut controls = Controls::new();
    let h = controls.create(
        &mut m,
        &mut heap,
        w,
        Rect {
            top: 0,
            left: 284,
            bottom: 180,
            right: 300,
        },
        b"Scroll",
        true,
        25,
        0,
        100,
        16,
        0x12345678,
    )?;
    let mut states = Map::new();
    states.insert("initial".into(), snapshot(&mut m, h, w));
    call(
        &mut controls,
        &mut m,
        &mut heap,
        &q,
        0xa963,
        &[(200, 2), (h, 4)],
    )?;
    states.insert("clamp_high".into(), snapshot(&mut m, h, w));
    for (op, value) in [(0xa964, 50u32), (0xa965, 80), (0xa963, 0xffff)] {
        call(
            &mut controls,
            &mut m,
            &mut heap,
            &q,
            op,
            &[(value, 2), (h, 4)],
        )?;
    }
    states.insert("clamp_low".into(), snapshot(&mut m, h, w));
    call(
        &mut controls,
        &mut m,
        &mut heap,
        &q,
        0xa959,
        &[(34, 2), (12, 2), (h, 4)],
    )?;
    call(
        &mut controls,
        &mut m,
        &mut heap,
        &q,
        0xa95c,
        &[(120, 2), (16, 2), (h, 4)],
    )?;
    states.insert("geometry".into(), snapshot(&mut m, h, w));
    call(
        &mut controls,
        &mut m,
        &mut heap,
        &q,
        0xa95d,
        &[(255, 2), (h, 4)],
    )?;
    call(&mut controls, &mut m, &mut heap, &q, 0xa958, &[(h, 4)])?;
    states.insert("hidden".into(), snapshot(&mut m, h, w));
    call(&mut controls, &mut m, &mut heap, &q, 0xa957, &[(h, 4)])?;
    call(
        &mut controls,
        &mut m,
        &mut heap,
        &q,
        0xa95d,
        &[(0, 2), (h, 4)],
    )?;
    states.insert("shown".into(), snapshot(&mut m, h, w));
    call(&mut controls, &mut m, &mut heap, &q, 0xa955, &[(h, 4)])?;
    states.insert(
        "disposed_head_null".into(),
        json!((m.read_long(w + 140) == 0) as u8),
    );
    m.put(0x1000, b"\x12.NoSuchCodexDriver")?;
    m.write_long(0x1100 + 18, 0x1000);
    let mut file_manager = crate::files::Files::new();
    let mut file_cpu = CpuCore::new();
    file_cpu.dar[8] = 0x1100;
    file_manager.dispatch(0xa000, &mut file_cpu, &mut m)?;
    let error = file_cpu.dar[0] as i16;
    let services =
        json!({"missing_driver":error,"missing_driver_pb":m.read_word(0x1100+16) as i16});
    windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
    Ok(json!({"schema":1,"probe":"controls-v1","controls":states,"services":services}).to_string())
}

pub fn windowflow_probe() -> Result<String, String> {
    use crate::{
        managers::{Heap, Rect},
        quickdraw::{read_rect, QuickDraw},
        regions::Region,
        windows::Windows,
    };
    use serde_json::{json, Map, Value};
    fn snap(m: &mut Memory, q: &QuickDraw, ws: &Windows, a: u32, b: u32) -> Result<Value, String> {
        fn bounds(m: &mut Memory, p: u32) -> [i16; 4] {
            let h = m.read_long(p);
            let data = m.read_long(h);
            let r = read_rect(m, data + 2);
            [r.top, r.left, r.bottom, r.right]
        }
        let front = ws.front();
        let current = q.get_port(m)?;
        Ok(
            json!({"visible":[(m.read_byte(a+110)!=0) as u8,(m.read_byte(b+110)!=0) as u8],"hilite":[(m.read_byte(a+111)!=0) as u8,(m.read_byte(b+111)!=0) as u8],"front":if front==a{1}else if front==b{2}else{0},"current":if current==a{1}else if current==b{2}else{0},"title_width":m.read_word(a+138),"update_a":bounds(m,a+122),"update_b":bounds(m,b+122),"vis_a":bounds(m,a+24)}),
        )
    }
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut ws = Windows::new();
    ws.init(&mut m, &mut heap, &q)?;
    let a = ws.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 180,
            right: 250,
        },
        b"One",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    let b = ws.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 220,
            left: 300,
            bottom: 340,
            right: 500,
        },
        b"Two",
        false,
        0,
        u32::MAX,
        true,
        0,
    )?;
    let mut states = Map::new();
    states.insert("initial".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.set_title(&mut m, &mut heap, &q, &fonts, a, b"Renamed")?;
    states.insert("title".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.bring_front(&mut m, b)?;
    states.insert("bring_hidden".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.visible(&mut m, &mut heap, &q, &fonts, &mut events, b, true)?;
    states.insert("show".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.select(&mut m, &q, &fonts, &mut events, a)?;
    states.insert("select".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.visible(&mut m, &mut heap, &q, &fonts, &mut events, a, false)?;
    states.insert("hide".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.visible(&mut m, &mut heap, &q, &fonts, &mut events, a, true)?;
    states.insert("reshow".into(), snap(&mut m, &q, &ws, a, b)?);
    for w in [a, b] {
        let h = m.read_long(w + 122);
        Region::default().store(&mut m, &mut heap, h)?;
    }
    q.set_port(&mut m, a)?;
    ws.invalidate(
        &mut m,
        &mut heap,
        &q,
        &mut events,
        Rect {
            top: 10,
            left: 20,
            bottom: 40,
            right: 80,
        },
    )?;
    states.insert("invalidated".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.begin_update(&mut m, &mut heap, &q, a)?;
    states.insert("begin".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.end_update(&mut m, &mut heap, a)?;
    states.insert("end".into(), snap(&mut m, &q, &ws, a, b)?);
    ws.dispose(&mut m, &mut heap, &q, &mut events, b)?;
    ws.dispose(&mut m, &mut heap, &q, &mut events, a)?;
    Ok(json!({"schema":1,"probe":"window-flow-v1","windows":states}).to_string())
}

pub fn file_probe() -> Result<String, String> {
    use crate::files::{Files, Fork};
    use serde_json::{json, Map};
    fn err<T>(result: Result<T, i16>) -> i16 {
        result.err().unwrap_or(0)
    }
    fn result<T>(value: Result<T, i16>) -> Result<T, String> {
        value.map_err(|e| format!("file replay: {e}"))
    }
    let mut files = Files::new();
    let name = b"FileProbe.data";
    let mut info = [0; 16];
    info[..4].copy_from_slice(b"TEXT");
    info[4..8].copy_from_slice(b"ttxt");
    let mut states = Map::new();
    states.insert("create".into(), json!(err(files.create(name, info))));
    states.insert("duplicate".into(), json!(err(files.create(name, info))));
    states.insert(
        "missing".into(),
        json!(err(files.open(b"NoSuchFileProbe.data", Fork::Data, 0))),
    );
    let f = result(files.open(name, Fork::Data, 0))?;
    let n = result(files.write(f, b"abcdef"))?;
    states.insert("write".into(), json!([0, n, result(files.position(f))?]));
    result(files.seek(f, 1, 2))?;
    let n = result(files.write(f, b"XY"))?;
    states.insert(
        "overwrite".into(),
        json!([0, n, result(files.position(f))?]),
    );
    result(files.seek(f, 2, -1))?;
    let (bytes, error) = result(files.read(f, 4))?;
    states.insert(
        "partial".into(),
        json!([
            error as i32,
            bytes.len() as i32,
            result(files.position(f))? as i32,
            bytes[0] as i32
        ]),
    );
    let eof = result(files.eof(f))?;
    let error = err(files.set_eof(f, 3));
    states.insert(
        "truncate".into(),
        json!([eof as i32, error as i32, result(files.position(f))? as i32]),
    );
    result(files.seek(f, 2, 0))?;
    result(files.write(f, b"z"))?;
    result(files.close(f))?;
    let f = result(files.open(name, Fork::Data, 0))?;
    let (bytes, error) = result(files.read(f, 8))?;
    states.insert(
        "reopen".into(),
        json!([
            error as i32,
            bytes.len() as i32,
            bytes[0] as i32,
            bytes[1] as i32,
            bytes[2] as i32,
            bytes[3] as i32
        ]),
    );
    result(files.close(f))?;
    let rf = result(files.open(name, Fork::Resource, 0))?;
    result(files.write(rf, b"RS"))?;
    let eof = result(files.eof(rf))?;
    result(files.seek(rf, 1, 0))?;
    let (bytes, error) = result(files.read(rf, 4))?;
    states.insert(
        "resource".into(),
        json!([
            eof as i32,
            error as i32,
            bytes.len() as i32,
            bytes[0] as i32,
            bytes[1] as i32
        ]),
    );
    result(files.close(rf))?;
    let finder = result(files.finder(name))?;
    states.insert(
        "finder".into(),
        json!([
            u32::from_be_bytes(finder[..4].try_into().unwrap()),
            u32::from_be_bytes(finder[4..8].try_into().unwrap())
        ]),
    );
    let ro = result(files.open(name, Fork::Data, 1))?;
    states.insert(
        "readonly_write".into(),
        json!([err(files.write(ro, b"!")), 0]),
    );
    result(files.close(ro))?;
    let f = result(files.open(name, Fork::Data, 0))?;
    let e = err(files.seek(f, 1, -1));
    states.insert(
        "negative_seek".into(),
        json!([e as i32, result(files.position(f))? as i32]),
    );
    let e = err(files.seek(f, 1, 20));
    states.insert(
        "past_eof_seek".into(),
        json!([e as i32, result(files.position(f))? as i32]),
    );
    result(files.close(f))?;
    states.insert("bad_ref".into(), json!(err(files.eof(32000))));
    Ok(json!({"schema":1,"probe":"files-v1","files":states}).to_string())
}

pub fn resfile_probe() -> Result<String, String> {
    let mut s = ResIOState::new()?;
    s.files
        .create(b"EmptyResource.data", [0; 16])
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for name in [b"EmptyResource.data".as_slice(), b"NoSuchResource.data"] {
        s.m.write_byte(0x110000, name.len() as u8);
        s.m.put(0x110001, name)?;
        let result = (s.api(0xa9c4, 0, &[(1, 1), (0, 2), (0x110000, 4)])? >> 16) as u16 as i16;
        rows.push((result, s.error()));
    }
    Ok(serde_json::json!({"schema":1,"probe":"res-files-v1","resources":{"empty_result":rows[0].0,"empty_error":rows[0].1,"missing_result":rows[1].0,"missing_error":rows[1].1}}).to_string())
}
pub fn textdirect_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let port = heap.new_ptr(&mut m, 108, true).map_err(|e| e.to_string())?;
    q.open_port(&mut m, &mut heap, port)?;
    let mut te = crate::textedit::TextEdit::new();
    let rect = crate::managers::Rect {
        top: 0,
        left: 0,
        bottom: 100,
        right: 200,
    };
    let h = te.create(&mut m, &mut heap, &q, &fonts, rect, rect)?;
    let p = m.read_long(h);
    let text = m.read_long(p + 62);
    let mut states = serde_json::Map::new();
    for (name, bytes) in [("grow", b"direct" as &[u8]), ("shrink", b"A\rB" as &[u8])] {
        if name == "shrink" {
            te.select(&mut m, h, 5, 6)?;
        }
        heap.resize(&mut m, text, bytes.len() as u32)
            .map_err(|e| e.to_string())?;
        let data = m.read_long(text);
        m.put(data as usize, bytes)?;
        te.calculate(&mut m, &mut heap, &fonts, h)?;
        let p = m.read_long(h);
        let n = m.read_word(p + 94) as u32;
        let lines: Vec<u16> = (0..=n).map(|i| m.read_word(p + 96 + i * 2)).collect();
        states.insert(
            format!("{name}_length"),
            serde_json::json!(m.read_word(p + 60)),
        );
        states.insert(format!("{name}_lines"), serde_json::json!(lines));
    }
    let p = m.read_long(h);
    states.insert(
        "shrink_selection".into(),
        serde_json::json!([m.read_word(p + 32), m.read_word(p + 34)]),
    );
    te.set_text(&mut m, &mut heap, &fonts, h, b"reference text")?;
    heap.resize(&mut m, text, 0).map_err(|e| e.to_string())?;
    te.calculate(&mut m, &mut heap, &fonts, h)?;
    let p = m.read_long(h);
    states.insert(
        "zero_selection".into(),
        serde_json::json!([m.read_word(p + 32), m.read_word(p + 34)]),
    );
    te.key(&mut m, &mut heap, &fonts, h, b'Z')?;
    let p = m.read_long(h);
    let data = m.read_long(text);
    states.insert(
        "zero_key".into(),
        serde_json::json!([
            m.read_word(p + 60),
            m.read_word(p + 32),
            m.read_word(p + 34),
            m.read_byte(data) as u16
        ]),
    );
    Ok(serde_json::json!({"schema":1,"probe":"text-direct-v1","textedit":states}).to_string())
}

pub fn hittest_probe() -> Result<String, String> {
    use crate::{
        events::Events,
        fonts::Fonts,
        managers::{Heap, Rect},
        quickdraw::QuickDraw,
        textedit::TextEdit,
        windows::Windows,
    };
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 80,
            left: 60,
            bottom: 220,
            right: 300,
        },
        b"Hit",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    let points = [
        (10, 100),
        (300, 400),
        (100, 100),
        (70, 150),
        (70, 70),
        (215, 295),
        (80, 60),
        (79, 59),
        (220, 300),
        (220, 100),
        (100, 59),
    ];
    let mut hits = |windows: &Windows| -> Result<Vec<[i32; 2]>, String> {
        points
            .iter()
            .map(|&(v, h)| {
                let (part, found) = windows.find(&mut m, v, h)?;
                Ok([
                    part as i32,
                    if found == w {
                        1
                    } else if found == 0 {
                        0
                    } else {
                        -1
                    },
                ])
            })
            .collect()
    };
    let visible = hits(&windows)?;
    let hilited = m.read_byte(w + 111) as u8;
    let mut header_grid = Vec::new();
    for v in 58..80 {
        let mut row = Vec::new();
        for h in 54..116 {
            row.push(windows.find(&mut m, v, h)?.0);
        }
        header_grid.push(row);
    }
    windows.visible(&mut m, &mut heap, &q, &fonts, &mut events, w, false)?;
    let hidden: Vec<[i32; 2]> = points
        .iter()
        .map(|&(v, h)| {
            let (part, found) = windows.find(&mut m, v, h)?;
            Ok([
                part as i32,
                if found == w {
                    1
                } else if found == 0 {
                    0
                } else {
                    -1
                },
            ])
        })
        .collect::<Result<_, String>>()?;
    windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
    let port = heap.new_ptr(&mut m, 108, true).map_err(|e| e.to_string())?;
    q.open_port(&mut m, &mut heap, port)?;
    q.set_word(&mut m, 68, 3)?;
    q.set_word(&mut m, 74, 12)?;
    m.write_byte(port + 70, 0);
    let rect = Rect {
        top: 20,
        left: 10,
        bottom: 100,
        right: 190,
    };
    let mut te = TextEdit::new();
    let h = te.create(&mut m, &mut heap, &q, &fonts, rect, rect)?;
    te.set_text(&mut m, &mut heap, &fonts, h, b"One two\rABC")?;
    let record = m.read_long(h);
    let line_height = m.read_word(record + 24);
    let ascent = m.read_word(record + 26);
    let mut offsets = Vec::new();
    for v in [0, 20, 24, 34, 35, 39, 50, 100] {
        let mut row = Vec::new();
        for x in [-10, 10, 14, 15, 16, 20, 24, 28, 45, 100, 200] {
            row.push(te.offset(&mut m, &fonts, h, v, x)?);
        }
        offsets.push(row);
    }
    let mut offset_sweep = Vec::new();
    for just in [0i16, 1, -1] {
        m.write_word(record + 58, just as u16);
        let mut lines = Vec::new();
        for v in [20, 39] {
            let mut row = Vec::new();
            for x in -10..=200 {
                row.push(te.offset(&mut m, &fonts, h, v, x)?);
            }
            lines.push(row);
        }
        offset_sweep.push(lines);
    }
    Ok(serde_json::json!({"schema":1,"probe":"hit-test-v1","windows":{"visible":visible,"hidden":hidden,"hilited":hilited,"header_grid":header_grid},"textedit":{"line_height":line_height,"ascent":ascent,"offsets":offsets,"offset_sweep":offset_sweep}}).to_string())
}

fn title_fixture() -> Result<
    (
        Memory,
        crate::managers::Heap,
        crate::quickdraw::QuickDraw,
        crate::fonts::Fonts,
        crate::windows::Windows,
        u32,
    ),
    String,
> {
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let mut events = crate::events::Events::new();
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        crate::managers::Rect {
            top: 60,
            left: 30,
            bottom: 180,
            right: 630,
        },
        b"",
        false,
        0,
        u32::MAX,
        true,
        0,
    )?;
    Ok((m, heap, q, fonts, windows, w))
}
fn measured_title(
    m: &mut Memory,
    heap: &mut crate::managers::Heap,
    q: &crate::quickdraw::QuickDraw,
    fonts: &crate::fonts::Fonts,
    windows: &mut crate::windows::Windows,
    w: u32,
    text: &[u8],
) -> Result<i16, String> {
    windows.set_title(m, heap, q, fonts, w, text)?;
    Ok(m.read_word(w + 138) as i16)
}
pub fn titlemetrics_probe() -> Result<String, String> {
    let (mut m, mut heap, q, fonts, mut windows, w) = title_fixture()?;
    let mut titles = Vec::new();
    for byte in 0..=255 {
        titles.push(measured_title(
            &mut m,
            &mut heap,
            &q,
            &fonts,
            &mut windows,
            w,
            &[byte],
        )?);
    }
    let widths = fonts.select(0, 12, 0)?.widths.clone();
    let mut strings = Vec::new();
    for byte in 0..=255 {
        strings.push(fonts.text_width(0, 12, 0, &[byte])?);
    }
    let mut samples = serde_json::Map::new();
    for text in [
        "One",
        "Renamed",
        "Reference",
        "Untitled",
        "AV",
        "To",
        "WA",
        "d",
        "dd",
        "Rd",
        "dR",
        "R",
    ] {
        samples.insert(
            text.into(),
            serde_json::json!([
                measured_title(
                    &mut m,
                    &mut heap,
                    &q,
                    &fonts,
                    &mut windows,
                    w,
                    text.as_bytes()
                )?,
                fonts.text_width(0, 12, 0, text.as_bytes())?
            ]),
        );
    }
    Ok(serde_json::json!({"schema":1,"probe":"title-metrics-v1","metrics":{"title_single":titles,"char_single":widths,"string_single":strings,"samples":samples}}).to_string())
}
pub fn titlebounds_probe() -> Result<String, String> {
    let (mut m, mut heap, q, fonts, mut windows, w) = title_fixture()?;
    let advances = fonts.select(0, 12, 0)?.widths.clone();
    let mut result = serde_json::json!({"schema":1,"probe":"title-bounds-v1","advances":advances});
    for (name, anchor, mode) in [
        ("single", 0, 0),
        ("prefix_m", b'M', 1),
        ("suffix_m", b'M', 2),
        ("prefix_r", b'R', 1),
        ("suffix_r", b'R', 2),
        ("leading_space", b' ', 1),
        ("trailing_space", b' ', 2),
    ] {
        let mut values = Vec::new();
        for byte in 0..=255 {
            let text = match mode {
                0 => vec![byte],
                1 => vec![anchor, byte],
                _ => vec![byte, anchor],
            };
            values.push(measured_title(
                &mut m,
                &mut heap,
                &q,
                &fonts,
                &mut windows,
                w,
                &text,
            )?);
        }
        result[name] = serde_json::json!(values);
    }
    let alphabet=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 !?.,-/\x80\x81\x8e\xc4\xff";
    let mut seed = 0x51444d47u32;
    let mut heldout = Vec::new();
    for _ in 0..128 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let len = 1 + ((seed >> 24) % 31) as usize;
        let text: Vec<u8> = (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                alphabet[(seed >> 24) as usize % alphabet.len()]
            })
            .collect();
        let width = measured_title(&mut m, &mut heap, &q, &fonts, &mut windows, w, &text)?;
        heldout.push(serde_json::json!({"bytes":text,"width":width}));
    }
    result["heldout"] = serde_json::json!(heldout);
    Ok(result.to_string())
}
pub fn titlepairs_probe() -> Result<String, String> {
    let (mut m, mut heap, q, fonts, mut windows, w) = title_fixture()?;
    let mut pairs = Vec::new();
    for a in 0..=255 {
        let mut row = Vec::new();
        for b in 0..=255 {
            row.push(measured_title(
                &mut m,
                &mut heap,
                &q,
                &fonts,
                &mut windows,
                w,
                &[a, b],
            )?);
        }
        pairs.push(row);
    }
    let mut seed = 0x474c5950u32;
    let mut long_cases = Vec::new();
    for _ in 0..32 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let len = 128 + ((seed >> 24) % 128) as usize;
        let text: Vec<u8> = (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect();
        let width = measured_title(&mut m, &mut heap, &q, &fonts, &mut windows, w, &text)?;
        long_cases.push(serde_json::json!({"bytes":text,"width":width}));
    }
    Ok(serde_json::json!({"schema":1,"probe":"title-pairs-v1","pairs":pairs,"long_cases":long_cases,"complete":true}).to_string())
}

fn textedit_snapshot(m: &mut Memory, h: u32) -> serde_json::Value {
    use crate::{managers::Rect, quickdraw::read_rect};
    use serde_json::json;
    let p = m.read_long(h);
    fn r(r: Rect) -> [i16; 4] {
        [r.top, r.left, r.bottom, r.right]
    }
    let count = m.read_word(p + 94) as u32;
    let lines: Vec<i16> = (0..=count)
        .map(|i| m.read_word(p + 96 + i * 2) as i16)
        .collect();
    let text = m.read_long(p + 62);
    let text = m.read_long(text);
    let length = m.read_word(p + 60) as u32;
    let bytes: Vec<u8> = (0..length).map(|i| m.read_byte(text + i)).collect();
    json!({"dest":r(read_rect(m,p)),"view":r(read_rect(m,p+8)),"selection":[m.read_word(p+32),m.read_word(p+34)],"metrics":[m.read_word(p+24),m.read_word(p+26)],"font":[m.read_word(p+74),m.read_byte(p+76),m.read_word(p+78),m.read_word(p+80)],"flags":[m.read_word(p+36) as i16,m.read_word(p+58) as i16,m.read_word(p+72) as i16],"length":length,"lines":lines,"text":bytes})
}

// This execution profile has no desk accessories. SystemEdit reports that
// the application must handle the edit command; it must not edit TE itself.
fn system_edit<B: AddressBus>(
    cpu: &mut CpuCore,
    m: &mut B,
    windows: &crate::windows::Windows,
) -> Result<(), String> {
    let sp = cpu.dar[15];
    let command = m.read_word(sp);
    if command > 7 {
        return Err("unmeasured SystemEdit command".into());
    }
    let front = windows.front();
    if front != 0 && (m.read_word(front + 108) as i16) < 0 {
        return Err("SystemEdit desk accessories are not implemented".into());
    }
    m.write_byte(sp + 2, 0);
    cpu.dar[15] += 2;
    cpu.pc += 2;
    Ok(())
}

pub fn clipboard_probe() -> Result<String, String> {
    use crate::{managers::Rect, scrap};
    use serde_json::{json, Map};
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 240,
            right: 350,
        },
        b"Clipboard",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    q.set_port(&mut m, w)?;
    q.set_word(&mut m, 68, 1)?;
    q.set_word(&mut m, 74, 12)?;
    let mut te = crate::textedit::TextEdit::new();
    scrap::init_te(&mut m, &mut heap)?;
    let rect = Rect {
        top: 4,
        left: 4,
        bottom: 164,
        right: 284,
    };
    let h = te.create(&mut m, &mut heap, &q, &fonts, rect, rect)?;
    let mut cpu = CpuCore::new();
    cpu.dar[15] = 0x7e0000;
    let mut callbacks = crate::callbacks::Callbacks::new();
    scrap::zero(&mut m, &mut heap)?;
    let base = m.read_word(0x968);
    te.set_text(&mut m, &mut heap, &fonts, h, b"one two\rthree")?;
    fn state(
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
        h: u32,
        base: u16,
    ) -> Result<serde_json::Value, String> {
        let document = textedit_snapshot(m, h);
        let len = m.read_word(0xab0) as u32;
        let handle = m.read_long(0xab4);
        let p = m.read_long(handle);
        let local: Vec<u8> = (0..len).map(|i| m.read_byte(p + i)).collect();
        let destination = heap.new_handle(m, 0, true).map_err(|e| e.to_string())?;
        let offset = 0x110000;
        m.write_long(offset, 0x12345678);
        let result = crate::scrap::get(m, heap, u32::from_be_bytes(*b"TEXT"), destination, offset)?;
        let p = m.read_long(destination);
        let system: Vec<u8> = (0..result.max(0) as u32)
            .map(|i| m.read_byte(p + i))
            .collect();
        let result = json!({"document":{"state":document},"te_scrap":local,"system_size":m.read_long(0x960),"count_delta":m.read_word(0x968).wrapping_sub(base) as i16,"get_text":[result,m.read_long(offset)],"system_text":system});
        heap.dispose(m, destination).map_err(|e| e.to_string())?;
        Ok(result)
    }
    let mut observations = Map::new();
    observations.insert("initial".into(), state(&mut m, &mut heap, h, base)?);
    let mut edits = Vec::new();
    for command in 0..8 {
        cpu.dar[15] = 0x7e0000;
        cpu.pc = 0x18000;
        m.write_word(0x7e0000, command);
        m.write_word(0x7e0002, 0x7777);
        system_edit(&mut cpu, &mut m, &windows)?;
        if cpu.dar[15] != 0x7e0002 || cpu.pc != 0x18002 {
            return Err("SystemEdit ABI mismatch".into());
        }
        edits.push(m.read_byte(0x7e0002));
    }
    observations.insert("system_edit".into(), json!(edits));
    let mut call = |op: u16,
                    m: &mut Memory,
                    heap: &mut crate::managers::Heap,
                    te: &mut crate::textedit::TextEdit|
     -> Result<(), String> {
        let sp = 0x7e0000;
        cpu.dar[15] = sp;
        cpu.pc = 0x18000;
        m.write_long(sp, h);
        if !te.dispatch(op, &mut cpu, m, heap, &q, &fonts, &events, &mut callbacks)? {
            return Err("clipboard TextEdit trap unhandled".into());
        }
        if cpu.dar[15] != sp + 4 || cpu.pc != 0x18002 {
            return Err("clipboard TextEdit trap ABI mismatch".into());
        }
        Ok(())
    };
    for (name, start, end, op) in [
        ("copy", 4, 7, 0xa9d5),
        ("paste", 0, 3, 0xa9db),
        ("cut", 3, 8, 0xa9d6),
        ("empty_copy", 1, 1, 0xa9d5),
        ("empty_paste", 1, 1, 0xa9db),
        ("empty_paste_selection", 0, 2, 0xa9db),
    ] {
        te.select(&mut m, h, start, end)?;
        call(op, &mut m, &mut heap, &mut te)?;
        observations.insert(name.into(), state(&mut m, &mut heap, h, base)?);
    }
    // System clipboard calls go through the same production trap marshalling.
    let mut put_results = Vec::new();
    for (kind, data) in [
        (*b"TEXT", b"odd".as_slice()),
        (*b"TEST", b"xy".as_slice()),
        (*b"TEXT", b"last".as_slice()),
    ] {
        let sp = 0x7e0000;
        cpu.dar[15] = sp;
        cpu.pc = 0x18000;
        for (i, b) in data.iter().enumerate() {
            m.write_byte(0x110010 + i as u32, *b);
        }
        m.write_long(sp, 0x110010);
        m.write_long(sp + 4, u32::from_be_bytes(kind));
        m.write_long(sp + 8, data.len() as u32);
        if !scrap::dispatch(0xa9fe, &mut cpu, &mut m, &mut heap)? || cpu.dar[15] != sp + 12 {
            return Err("PutScrap ABI mismatch".into());
        }
        put_results.push(m.read_long(sp + 12) as i32);
    }
    observations.insert("put_results".into(), json!(put_results));
    observations.insert("system_put".into(), state(&mut m, &mut heap, h, base)?);
    scrap::zero(&mut m, &mut heap)?;
    observations.insert("system_zero".into(), state(&mut m, &mut heap, h, base)?);
    scrap::zero(&mut m, &mut heap)?;
    observations.insert(
        "system_zero_again".into(),
        state(&mut m, &mut heap, h, base)?,
    );
    Ok(json!({"schema":1,"probe":"clipboard-v1","clipboard":observations}).to_string())
}

pub fn process_probe() -> Result<String, String> {
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let process = crate::processes::Process::new(
        b"ProcessProbe",
        u32::from_be_bytes(*b"pPrb"),
        0x80,
        0x20000,
        0x7e0000,
    )?;
    let mut cpu = CpuCore::new();
    fn call(
        process: &crate::processes::Process,
        cpu: &mut CpuCore,
        m: &mut Memory,
        heap: &crate::managers::Heap,
        selector: u16,
        args: &[u32],
    ) -> Result<i16, String> {
        let sp = 0x7e0000;
        cpu.dar[15] = sp;
        cpu.pc = 0x18000;
        m.write_word(sp, selector);
        for (i, arg) in args.iter().enumerate() {
            m.write_long(sp + 2 + i as u32 * 4, *arg);
        }
        let pop = 2 + args.len() as u32 * 4;
        m.write_word(sp + pop, 0x7777);
        if !process.dispatch(0xa88f, cpu, m, heap)? || cpu.dar[15] != sp + pop || cpu.pc != 0x18002
        {
            return Err("Process Manager trap ABI mismatch".into());
        }
        Ok(m.read_word(sp + pop) as i16)
    }
    fn string(m: &mut Memory, p: u32) -> Vec<u8> {
        let n = m.read_byte(p) as u32;
        (0..n).map(|i| m.read_byte(p + 1 + i)).collect()
    }
    let current = 0x110000;
    let alias = current + 8;
    let none = alias + 8;
    let bad = none + 8;
    let info = 0x110100;
    let name = 0x110200;
    let spec = 0x110400;
    let same = 0x110500;
    m.write_long(alias, 0);
    m.write_long(alias + 4, 2);
    m.write_long(bad, 0x12345678);
    m.write_long(bad + 4, 0x76543210);
    let current_error = call(&process, &mut cpu, &mut m, &heap, 0x37, &[current])?;
    m.write_long(info, 60);
    m.write_long(info + 4, name);
    m.write_long(info + 56, spec);
    let info_error = call(&process, &mut cpu, &mut m, &heap, 0x3a, &[info, current])?;
    let mut result = json!({"current_error":current_error,"current_shape":[(m.read_long(current)==0) as u8,(m.read_long(current+4)>2) as u8],"info_error":info_error,"info_length":m.read_long(info),"name":string(&mut m,name),"identity":[m.read_long(info+16),m.read_long(info+20),m.read_long(info+24)],"number_matches":((m.read_long(info+8)==m.read_long(current))&&(m.read_long(info+12)==m.read_long(current+4))) as u8,"memory_relations":[(m.read_long(info+28)!=0) as u8,(m.read_long(info+32)>0) as u8,(m.read_long(info+36)<=m.read_long(info+32)) as u8],"app_name":string(&mut m,spec+6)});
    let mut comparisons = Vec::new();
    for other in [current, alias, none, bad] {
        m.write_byte(same, 99);
        let error = call(
            &process,
            &mut cpu,
            &mut m,
            &heap,
            0x3d,
            &[same, other, current],
        )?;
        comparisons.push(json!([error, m.read_byte(same)]));
    }
    result["same"] = json!(comparisons);
    let mut lengths = Vec::new();
    for len in [0, 1, 59, 60, 61, 64, 68] {
        for i in 0..60 {
            m.write_byte(info + i, 0);
        }
        m.write_long(info, len);
        let error = call(&process, &mut cpu, &mut m, &heap, 0x3a, &[info, current])?;
        lengths.push(json!([len, error, m.read_long(info + 16)]));
    }
    result["lengths"] = json!(lengths);
    for i in 0..60 {
        m.write_byte(info + i, 0);
    }
    m.write_long(info, 60);
    let error = call(&process, &mut cpu, &mut m, &heap, 0x3a, &[info, bad])?;
    result["bad_info"] = json!([error, m.read_long(info + 16)]);
    let error = call(&process, &mut cpu, &mut m, &heap, 0x38, &[bad])?;
    result["bad_next"] = json!([
        error,
        (m.read_long(bad) == 0) as u8,
        (m.read_long(bad + 4) > 2) as u8
    ]);
    m.write_long(none, 0);
    m.write_long(none + 4, 0);
    let mut found = 0;
    let mut error = 0;
    for _ in 0..100 {
        error = call(&process, &mut cpu, &mut m, &heap, 0x38, &[none])?;
        if error != 0 {
            break;
        }
        if m.read_long(none) == m.read_long(current)
            && m.read_long(none + 4) == m.read_long(current + 4)
        {
            found += 1;
        }
    }
    result["enumeration"] = json!([(found == 1) as u8, error]);
    Ok(json!({"schema":1,"probe":"processes-v1","processes":result}).to_string())
}

pub fn saveconfirm_probe() -> Result<String, String> {
    let bytes = std::fs::read("work/apps/TeachText/TeachText.rsrc").map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::standardfile::probe(&mut m, &resources)
}

pub fn filedelete_probe() -> Result<String, String> {
    use crate::files::{Files, Fork};
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut cpu = CpuCore::new();
    let mut files = Files::new();
    let mut info = [0; 16];
    info[..8].copy_from_slice(b"TEXTttxt");
    let name = b"FileDelete.data";
    files.create(name, info).map_err(|e| e.to_string())?;
    let f = files.open(name, Fork::Data, 0).map_err(|e| e.to_string())?;
    files.write(f, b"DATA").map_err(|e| e.to_string())?;
    let rf = files
        .open(name, Fork::Resource, 0)
        .map_err(|e| e.to_string())?;
    files.write(rf, b"RS").map_err(|e| e.to_string())?;
    files.close(rf).map_err(|e| e.to_string())?;
    fn delete(
        files: &mut Files,
        m: &mut Memory,
        cpu: &mut CpuCore,
        volume: i16,
        directory: u32,
    ) -> Result<serde_json::Value, String> {
        let p = 0x110000;
        for i in 0..80 {
            m.write_byte(p + i, 0);
        }
        let name = b"FileDelete.data";
        m.write_byte(p + 80, name.len() as u8);
        for (i, b) in name.iter().enumerate() {
            m.write_byte(p + 81 + i as u32, *b);
        }
        m.write_long(p + 18, p + 80);
        m.write_word(p + 22, volume as u16);
        m.write_long(p + 48, directory);
        cpu.dar[8] = p;
        cpu.pc = 0x19000;
        cpu.dar[15] = 0x7e0000;
        if !files.dispatch(0xa209, cpu, m)? || cpu.pc != 0x19002 || cpu.dar[15] != 0x7e0000 {
            return Err("HDelete trap ABI mismatch".into());
        }
        Ok(serde_json::json!([
            cpu.dar[0] as i16,
            m.read_word(p + 16) as i16
        ]))
    }
    let mut result = json!({"open_delete":delete(&mut files,&mut m,&mut cpu,0,0)?});
    files.close(f).map_err(|e| e.to_string())?;
    result["bad_volume"] = delete(&mut files, &mut m, &mut cpu, 1234, 0)?;
    result["bad_directory"] = delete(&mut files, &mut m, &mut cpu, 0, 0x12345678)?;
    result["closed_delete"] = delete(&mut files, &mut m, &mut cpu, 0, 0)?;
    result["deleted_info"] = json!(files.finder(name).err().unwrap_or(0));
    for (key, fork) in [
        ("deleted_data", Fork::Data),
        ("deleted_resource", Fork::Resource),
    ] {
        let error = match files.open(name, fork, 0) {
            Ok(r) => {
                files.close(r).map_err(|e| e.to_string())?;
                0
            }
            Err(e) => e,
        };
        result[key] = json!(error);
    }
    result["missing_delete"] = delete(&mut files, &mut m, &mut cpu, 0, 0)?;
    files.create(name, info).map_err(|e| e.to_string())?;
    let mut sizes = Vec::new();
    for fork in [Fork::Data, Fork::Resource] {
        let r = files.open(name, fork, 0).map_err(|e| e.to_string())?;
        sizes.push(files.eof(r).map_err(|e| e.to_string())?);
        files.close(r).map_err(|e| e.to_string())?;
    }
    result["recreate_eof"] = json!(sizes);
    Ok(json!({"schema":1,"probe":"file-delete-v1","files":result}).to_string())
}

pub fn dialogparams_probe() -> Result<String, String> {
    use serde_json::{json, Map};
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut dialogs = crate::dialogs::Dialogs::new();
    let mut cpu = CpuCore::new();
    fn state(
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
    ) -> Result<serde_json::Value, String> {
        let mut parameters = Vec::new();
        for i in 0..4 {
            let h = m.read_long(0xaa0 + i * 4);
            if h == 0 {
                parameters.push(serde_json::Value::Null);
            } else {
                let size = heap.size(h).map_err(|e| e.to_string())?;
                let p = m.read_long(h);
                let n = m.read_byte(p) as u32;
                let text: Vec<u8> = (0..n).map(|j| m.read_byte(p + 1 + j)).collect();
                parameters.push(json!({"size":size,"text":text}));
            }
        }
        Ok(json!(parameters))
    }
    let mut result = Map::new();
    result.insert("initial".into(), state(&mut m, &mut heap)?);
    for (name, parameters) in [
        (
            "set",
            [
                Some(b"one".as_slice()),
                Some(b"".as_slice()),
                Some(b"\x80".as_slice()),
                Some(b"^0".as_slice()),
            ],
        ),
        (
            "partial",
            [None, Some(b"TWO".as_slice()), None, Some(b"".as_slice())],
        ),
        ("nil", [None, None, None, None]),
    ] {
        let sp = 0x7e0000;
        cpu.dar[15] = sp;
        cpu.pc = 0x19000;
        for (i, text) in parameters.iter().enumerate() {
            let p = if let Some(text) = text {
                let p = 0x110000 + i as u32 * 256;
                m.write_byte(p, text.len() as u8);
                for (j, b) in text.iter().enumerate() {
                    m.write_byte(p + 1 + j as u32, *b);
                }
                p
            } else {
                0
            };
            m.write_long(sp + (3 - i) as u32 * 4, p);
        }
        if !dialogs.dispatch(0xa98b, &mut cpu, &mut m, &mut heap)?
            || cpu.dar[15] != sp + 16
            || cpu.pc != 0x19002
        {
            return Err("ParamText ABI mismatch".into());
        }
        result.insert(name.into(), state(&mut m, &mut heap)?);
    }
    Ok(json!({"schema":1,"probe":"dialog-params-v1","parameters":result}).to_string())
}

pub fn appsource_probe() -> Result<String, String> {
    use crate::files::Files;
    use crate::processes::Process;
    use serde_json::json;
    let resource = std::fs::read("work/probes/appsource/AppSourceProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut files = Files::new();
    let mut finder = [0; 16];
    finder[..8].copy_from_slice(b"APPLpPrb");
    let data = std::fs::read("probes/appsource/data-fork.bin").map_err(|e| e.to_string())?;
    files.mount_application(b"AppSourceProbe", finder, &data, &resource)?;
    let process = Process::new(
        b"AppSourceProbe",
        u32::from_be_bytes(*b"pPrb"),
        0x80,
        0x20000,
        0x7e0000,
    )?;
    let mut cpu = CpuCore::new();
    let info = 0x110000;
    let app = 0x110100;
    let alias = 0x110200;
    let sp = 0x7e0000;
    m.write_long(alias + 4, 2);
    m.write_long(info, 60);
    m.write_long(info + 56, app);
    m.write_word(sp, 0x3a);
    m.write_long(sp + 2, info);
    m.write_long(sp + 6, alias);
    cpu.dar[15] = sp;
    cpu.pc = 0x19000;
    if !process.dispatch(0xa88f, &mut cpu, &mut m, &heap)? {
        return Err("GetProcessInformation unhandled".into());
    }
    let process_info = m.read_word(sp + 10) as i16;
    fn fs(
        files: &mut Files,
        m: &mut Memory,
        cpu: &mut CpuCore,
        selector: u32,
        sp: u32,
    ) -> Result<i16, String> {
        cpu.dar[0] = selector;
        cpu.dar[15] = sp;
        cpu.pc = 0x19000;
        if !files.dispatch(0xaa52, cpu, m)? {
            return Err("source FSSpec operation unhandled".into());
        }
        Ok(m.read_word(cpu.dar[15]) as i16)
    }
    let found = 0x110300;
    m.write_long(sp, found);
    m.write_long(sp + 4, app + 6);
    let dir = m.read_long(app + 2);
    let vol = m.read_word(app);
    m.write_long(sp + 8, dir);
    m.write_word(sp + 12, vol);
    let lookup_error = fs(&mut files, &mut m, &mut cpu, 1, sp)?;
    let lookup_same = (0..70).all(|i| m.read_byte(found + i) == m.read_byte(app + i));
    let finder = 0x110400;
    m.write_long(sp, finder);
    m.write_long(sp + 4, app);
    let finder_error = fs(&mut files, &mut m, &mut cpu, 7, sp)?;
    let mut result = json!({"process_info":process_info,"lookup":[lookup_error,lookup_same as u8],"finder":[finder_error,m.read_long(finder),m.read_long(finder+4)]});
    for (label, selector) in [("data", 2), ("resource", 3)] {
        let output = 0x110500;
        m.write_long(sp, output);
        m.write_byte(sp + 4, 1);
        m.write_long(sp + 6, app);
        let opened = fs(&mut files, &mut m, &mut cpu, selector, sp)?;
        if opened != 0 {
            return Err(format!("source fork open failed {opened}"));
        }
        let reference = m.read_word(output);
        let pb = 0x110600;
        for i in 0..80 {
            m.write_byte(pb + i, 0);
        }
        m.write_word(pb + 24, reference);
        cpu.dar[8] = pb;
        cpu.pc = 0x19000;
        files.dispatch(0xa011, &mut cpu, &mut m)?;
        let size = m.read_long(pb + 28);
        {
            let buffer = 0x110700;
            m.write_long(pb + 32, buffer);
            m.write_long(pb + 36, 16);
            m.write_word(pb + 44, 0);
            cpu.dar[8] = pb;
            cpu.pc = 0x19000;
            files.dispatch(0xa002, &mut cpu, &mut m)?;
            let actual = m.read_long(pb + 40);
            let header: Vec<u8> = (0..actual).map(|i| m.read_byte(buffer + i)).collect();
            result[label] = json!([cpu.dar[0] as i16, size, actual]);
            result[if label == "data" {
                "data_bytes"
            } else {
                "header"
            }] = json!(header);
            if label == "resource" {
                m.write_byte(buffer, b'X');
                m.write_long(pb + 40, 77);
                m.write_long(pb + 36, 1);
                cpu.pc = 0x19000;
                files.dispatch(0xa003, &mut cpu, &mut m)?;
                let error = cpu.dar[0] as i16;
                let actual = m.read_long(pb + 40);
                cpu.pc = 0x19000;
                files.dispatch(0xa018, &mut cpu, &mut m)?;
                result["readonly_write"] = json!([error, actual, m.read_long(pb + 46)]);
            }
        }
        cpu.pc = 0x19000;
        files.dispatch(0xa001, &mut cpu, &mut m)?;
    }
    let pb = 0x110600;
    m.write_word(pb + 24, crate::files::APPLICATION_RESOURCE_FILE as u16);
    cpu.dar[8] = pb;
    cpu.pc = 0x19000;
    files.dispatch(0xa011, &mut cpu, &mut m)?;
    result["current_resource_file"] = json!([cpu.dar[0] as i16, m.read_long(pb + 28)]);
    Ok(json!({"schema":1,"probe":"app-source-v1","source":result}).to_string())
}

#[cfg(test)]
mod application_source_tests {
    use super::*;
    use crate::files::{Files, Fork, APPLICATION_RESOURCE_FILE, APPLICATION_VOLUME};

    #[test]
    fn source_volume_routes_forks_and_current_directory_without_shadowing_documents() {
        let mut files = Files::new();
        let mut finder = [0; 16];
        finder[..8].copy_from_slice(b"APPLtest");
        files.create(b"Same Name", [0; 16]).unwrap();
        let doc = files.open(b"Same Name", Fork::Data, 0).unwrap();
        files.write(doc, b"document").unwrap();
        files
            .mount_application(b"Same Name", finder, b"source\0\x80", b"resource")
            .unwrap();
        let mut memory = Memory {
            bytes: vec![0; 4096],
        };
        let p = 256;
        let name = 512;
        memory.write_long(p + 18, name);
        memory.write_byte(name, 9);
        memory.put(name as usize + 1, b"Same Name").unwrap();
        memory.write_word(p + 22, APPLICATION_VOLUME as u16);
        memory.write_byte(p + 27, 1);
        files.parameter_block(&mut memory, 0xa000, p).unwrap();
        let source = memory.read_word(p + 24);
        assert_ne!(source as i16, doc);
        memory.write_long(p + 32, 768);
        memory.write_long(p + 36, 16);
        assert_eq!(files.parameter_block(&mut memory, 0xa002, p), Err(-39));
        assert_eq!(memory.read_long(p + 40), 8);
        assert_eq!(&memory.bytes[768..776], b"source\0\x80");
        memory.write_word(p + 24, APPLICATION_RESOURCE_FILE as u16);
        files.parameter_block(&mut memory, 0xa011, p).unwrap();
        assert_eq!(memory.read_long(p + 28), 8);
        memory.write_word(p + 24, doc as u16);
        files.parameter_block(&mut memory, 0xa011, p).unwrap();
        assert_eq!(memory.read_long(p + 28), 8);
        memory.write_long(p + 48, 2);
        files.parameter_block(&mut memory, 0xa215, p).unwrap();
        memory.write_word(p + 22, 0);
        files.parameter_block(&mut memory, 0xa214, p).unwrap();
        assert_eq!(memory.read_word(p + 22) as i16, APPLICATION_VOLUME);
        assert_eq!(
            &memory.bytes[name as usize + 1..name as usize + 12],
            b"Application"
        );
        memory.write_byte(name, 9);
        memory.put(name as usize + 1, b"Same Name").unwrap();
        memory.write_word(p + 22, 0);
        files.parameter_block(&mut memory, 0xa000, p).unwrap();
        let current_source = memory.read_word(p + 24);
        assert_ne!(current_source, source);
        memory.write_word(p + 22, (-1i16) as u16);
        files.parameter_block(&mut memory, 0xa215, p).unwrap();
        memory.write_word(p + 22, 0);
        files.parameter_block(&mut memory, 0xa000, p).unwrap();
        let current_doc = memory.read_word(p + 24) as i16;
        assert_eq!(files.read(current_doc, 8).unwrap().0, b"document");
        assert_eq!(
            files.fork_bytes(b"Same Name", Fork::Data).unwrap(),
            b"document"
        );
        assert_eq!(files.list(&[]), vec![b"Same Name".to_vec()]);
    }
}

pub fn segment_probe() -> Result<String, String> {
    use serde_json::json;
    let bytes =
        std::fs::read("work/probes/segments/SegmentProbe.rsrc").map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes).map_err(|e| format!("{e:?}"))?;
    let zero = resources
        .iter()
        .find(|r| r.kind == *b"CODE" && r.id == 0)
        .ok_or("missing fixture CODE0")?;
    let length = be32(zero.data, 8)?;
    let table = 0x100000 + be32(zero.data, 12)?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    m.put(table as usize, &zero.data[16..16 + length as usize])?;
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let map = heap
        .new_handle(&mut m, zero.map.len() as u32, false)
        .map_err(|e| e.to_string())?;
    let p = heap.data(map).map_err(|e| e.to_string())?;
    m.put(p as usize, zero.map)?;
    m.write_long(0xa50, map);
    let code = resources
        .iter()
        .find(|r| r.kind == *b"CODE" && r.id == 2)
        .ok_or("missing fixture CODE2")?;
    let mut handles = std::collections::BTreeMap::new();
    let h = load_resource(code, &mut m, &mut heap, &mut handles)?;
    let manager = crate::segments::Segments::new(table, length);
    let first = table + length - 16 + 2;
    let second = first + 8;
    fn snapshot(
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
        h: u32,
        first: u32,
        second: u32,
        value: i16,
    ) -> Result<serde_json::Value, String> {
        let mut entries = Vec::new();
        let base = heap.data(h).map_err(|e| e.to_string())? + 4;
        for p in [first - 2, second - 2] {
            entries.push(if m.read_word(p + 2) == 0x4ef9 {
                json!([
                    m.read_word(p),
                    m.read_word(p + 2),
                    m.read_long(p + 4) - base
                ])
            } else {
                json!([
                    m.read_word(p),
                    m.read_word(p + 2),
                    m.read_word(p + 4),
                    m.read_word(p + 6)
                ])
            });
        }
        Ok(
            json!({"value":value,"entries":entries,"handle_state":heap.state(h).map_err(|e|e.to_string())?}),
        )
    }
    fn call(
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
        manager: &crate::segments::Segments,
        h: u32,
        pointer: u32,
    ) -> Result<i16, String> {
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(CpuType::M68020);
        cpu.pc = pointer;
        cpu.dar[15] = 0x7e0000;
        m.write_long(cpu.dar[15], 0x19000);
        for _ in 0..20 {
            if cpu.pc == 0x19000 {
                return Ok(cpu.dar[0] as i16);
            }
            if m.read_word(cpu.pc) == 0xa9f0 {
                let id = m.read_word(cpu.dar[15]) as i16;
                if id != 2 {
                    return Err("fixture called unexpected code segment".into());
                }
                manager.load(id, h, m, heap)?;
                cpu.dar[15] += 2;
                cpu.pc -= 4;
            } else if !matches!(cpu.step(m), StepResult::Ok { .. }) {
                return Err("fixture CPU execution failed".into());
            }
        }
        Err("fixture did not return".into())
    }
    let mut result = serde_json::Map::new();
    result.insert(
        "initial".into(),
        snapshot(&mut m, &mut heap, h, first, second, 0)?,
    );
    let value = call(&mut m, &mut heap, &manager, h, first)?;
    result.insert(
        "called".into(),
        snapshot(&mut m, &mut heap, h, first, second, value)?,
    );
    manager.unload(first, &mut m, &mut heap, &handles)?;
    result.insert(
        "unloaded".into(),
        snapshot(&mut m, &mut heap, h, first, second, value)?,
    );
    manager.unload(first, &mut m, &mut heap, &handles)?;
    result.insert(
        "unloaded_twice".into(),
        snapshot(&mut m, &mut heap, h, first, second, value)?,
    );
    let value = call(&mut m, &mut heap, &manager, h, second)?;
    result.insert(
        "recalled".into(),
        snapshot(&mut m, &mut heap, h, first, second, value)?,
    );
    Ok(json!({"schema":1,"probe":"segments-v1","segments":result}).to_string())
}

#[cfg(test)]
mod segment_tests {
    use super::*;
    #[test]
    fn reload_uses_the_relocated_resource_handle_and_failed_transitions_are_atomic() {
        let mut m = Memory {
            bytes: vec![0; 0x10000],
        };
        let mut heap = crate::managers::Heap::new(0x2000, 0xa000);
        let table = 0x1000;
        let manager = crate::segments::Segments::new(table, 16);
        for (i, offset) in [0, 4].iter().enumerate() {
            let p = table + i as u32 * 8;
            m.write_word(p, *offset);
            m.write_word(p + 2, 0x3f3c);
            m.write_word(p + 4, 2);
            m.write_word(p + 6, 0xa9f0);
        }
        let h = heap.new_handle(&mut m, 12, false).unwrap();
        let old = heap.data(h).unwrap();
        m.put(
            old as usize,
            &[0, 0, 0, 2, 0x70, 7, 0x4e, 0x75, 0x70, 11, 0x4e, 0x75],
        )
        .unwrap();
        heap.resource_state(h, false, false).unwrap();
        let handles = std::collections::BTreeMap::from([((*b"CODE", 2), h)]);
        m.write_word(table + 10, 0x4e71);
        assert!(manager.load(2, h, &mut m, &mut heap).is_err());
        assert_eq!(m.read_word(table + 2), 0x3f3c);
        assert_eq!(heap.state(h).unwrap(), 0x20);
        m.write_word(table + 10, 0x3f3c);
        manager.load(2, h, &mut m, &mut heap).unwrap();
        assert_eq!(m.read_long(table + 4), old + 4);
        assert_eq!(heap.state(h).unwrap(), 0xa0);
        m.write_long(table + 12, old + 100);
        assert!(manager
            .unload(table + 2, &mut m, &mut heap, &handles)
            .is_err());
        assert_eq!(m.read_word(table + 2), 0x4ef9);
        assert_eq!(heap.state(h).unwrap(), 0xa0);
        m.write_long(table + 12, old + 8);
        manager
            .unload(table + 2, &mut m, &mut heap, &handles)
            .unwrap();
        assert_eq!(heap.state(h).unwrap(), 0x20);
        heap.new_ptr(&mut m, 16, false).unwrap();
        heap.resize(&mut m, h, 16).unwrap();
        let new = heap.data(h).unwrap();
        assert_ne!(new, old);
        manager.load(2, h, &mut m, &mut heap).unwrap();
        assert_eq!(m.read_long(table + 4), new + 4);
        assert_eq!(m.read_long(table + 12), new + 8);
        assert_eq!(m.read_word(new + 4), 0x7007);
        assert!(manager.unload(0, &mut m, &mut heap, &handles).is_err());
    }
}

pub fn environs_probe() -> Result<String, String> {
    use crate::environment::Environment;
    use serde_json::json;
    // Declared inputs for the pinned Mac99 reference configuration. The
    // application runtime uses its own profile, not this reference identity.
    let environment = Environment {
        machine: 404,
        system: 0x0921,
        processor: 3,
        fpu_type: 0,
        quickdraw: 0x251,
        keyboard: 4,
        appletalk: 0,
        system_volume: -32605,
        sound_version: 0,
    };
    let mut m = Memory {
        bytes: vec![0xa5; 4096],
    };
    let mut cpu = CpuCore::new();
    cpu.dar[0] = 2;
    cpu.dar[8] = 258;
    environment.dispatch(0xa090, &mut cpu, &mut m);
    let profile: Vec<u8> = (258..274).map(|p| m.read_byte(p)).collect();
    let mut queries = Vec::new();
    for version in [-1i16, 0, 1, 2, 3, 32767] {
        for p in 256..276 {
            m.write_byte(p, 0xa5);
        }
        cpu.dar[0] = 0x12340000 | version as u16 as u32;
        cpu.dar[8] = 258;
        cpu.dar[15] = 0xe00;
        cpu.pc = 0x100;
        cpu.set_ccr(0x10);
        if !environment.dispatch(0xa090, &mut cpu, &mut m)
            || cpu.pc != 0x102
            || cpu.dar[15] != 0xe00
        {
            return Err("SysEnvirons trap/stack ABI mismatch".into());
        }
        let flags = (if cpu.flag_x() { 16 } else { 0 })
            | (if cpu.flag_n() { 8 } else { 0 })
            | (if cpu.flag_z() { 4 } else { 0 })
            | (if cpu.flag_v() { 2 } else { 0 })
            | (if cpu.flag_c() { 1 } else { 0 });
        let bytes: Vec<u8> = (256..276).map(|p| m.read_byte(p)).collect();
        queries.push(json!({"version":version,"d0":cpu.dar[0],"ccr":flags,"a0_same":(cpu.dar[8]==258) as u8,"bytes":bytes}));
    }
    Ok(json!({"schema":1,"probe":"environs-v1","environment":{"profile":profile,"queries":queries}}).to_string())
}

pub fn gestalt_probe() -> Result<String, String> {
    use serde_json::json;
    let environment = crate::environment::Environment {
        machine: 404,
        system: 0x0921,
        processor: 3,
        fpu_type: 0,
        quickdraw: 0x251,
        keyboard: 4,
        appletalk: 0,
        system_volume: -32605,
        sound_version: 0,
    };
    let mut cpu = CpuCore::new();
    let mut m = Memory {
        bytes: vec![0; 4096],
    };
    let mut result = Vec::new();
    for selector in [*b"sysv", *b"qd  ", *b"proc", *b"fpu ", *b"zZ9!"] {
        let selector = u32::from_be_bytes(selector);
        cpu.dar[0] = selector;
        cpu.dar[8] = 0x12345678;
        cpu.dar[15] = 0xe00;
        cpu.pc = 0x100;
        cpu.set_ccr(0x10);
        if !environment.dispatch(0xa1ad, &mut cpu, &mut m)
            || cpu.pc != 0x102
            || cpu.dar[15] != 0xe00
        {
            return Err("Gestalt trap ABI mismatch".into());
        }
        let flags = (if cpu.flag_x() { 16 } else { 0 })
            | (if cpu.flag_n() { 8 } else { 0 })
            | (if cpu.flag_z() { 4 } else { 0 })
            | (if cpu.flag_v() { 2 } else { 0 })
            | (if cpu.flag_c() { 1 } else { 0 });
        result.push(json!([selector, cpu.dar[0], cpu.dar[8], flags]));
    }
    Ok(json!({"schema":1,"probe":"gestalt-v1","gestalt":result}).to_string())
}

pub fn menubar_probe() -> Result<String, String> {
    use crate::{fonts::Fonts, managers::Heap, menus::Menus};
    use serde_json::json;
    use std::collections::BTreeMap;
    fn api(
        menus: &mut Menus,
        m: &mut Memory,
        heap: &mut Heap,
        fonts: &Fonts,
        resources: &[Resource<'_>],
        cache: &mut BTreeMap<([u8; 4], i16), u32>,
        op: u16,
        arg: u32,
    ) -> Result<u32, String> {
        let sp = 0x7e0000;
        let mut cpu = CpuCore::new();
        cpu.dar[15] = sp;
        cpu.pc = 0x19000;
        if op == 0xa93c {
            m.write_long(sp, arg);
        } else if op != 0xa93b {
            m.write_word(sp, arg as u16);
        }
        if !menus.dispatch(op, &mut cpu, m, heap, fonts, resources, |r, m, h| {
            load_resource(r, m, h, cache)
        })? {
            return Err("menu bar trap unhandled".into());
        }
        let pop = if op == 0xa93c {
            4
        } else if op == 0xa93b {
            0
        } else {
            2
        };
        if cpu.pc != 0x19002 || cpu.dar[15] != sp + pop {
            return Err("menu bar Pascal ABI mismatch".into());
        }
        Ok(if op == 0xa93c {
            0
        } else {
            m.read_long(sp + pop)
        })
    }
    let bytes = std::fs::read("work/probes/menubars/MenuBarProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut fonts = Fonts::new();
    fonts.init()?;
    let map = resources[0].map;
    let map_h = heap
        .new_handle(&mut m, map.len() as u32, false)
        .map_err(|e| e.to_string())?;
    let p = heap.data(map_h).map_err(|e| e.to_string())?;
    m.put(p as usize, map)?;
    m.write_long(0xa50, map_h);
    let mut cache = BTreeMap::new();
    let mut menus = Menus::new();
    menus.init(&mut m, &mut heap)?;
    // Initial application-supplied Old menu, equivalent to native NewMenu/
    // AppendMenu setup. The observed API operations below use actual managers.
    let mut old_bytes = vec![0u8; 14];
    old_bytes[..2].copy_from_slice(&20999u16.to_be_bytes());
    old_bytes[10..14].copy_from_slice(&u32::MAX.to_be_bytes());
    old_bytes.extend_from_slice(b"\x03Old\x07Earlier\x00E\x00\x00\x00");
    let old = heap
        .new_handle(&mut m, old_bytes.len() as u32, false)
        .map_err(|e| e.to_string())?;
    let p = heap.data(old).map_err(|e| e.to_string())?;
    m.put(p as usize, &old_bytes)?;
    menus.calculate(&mut m, &mut heap, &fonts, old)?;
    menus.insert(&mut m, &mut heap, old, 0)?;
    let base = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93b, 0,
    )?;
    let bar = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa9c0, 21000,
    )?;
    let error = m.read_word(0xa60) as i16;
    let mut result = serde_json::Map::new();
    result.insert(
        "new".into(),
        json!([
            (bar != 0) as u8,
            error,
            (menus.handle(&mut m, 20999) == old) as u8,
            (menus.handle(&mut m, 21000) == 0) as u8,
            menus.key(&mut m, &mut heap, b'e')?
        ]),
    );
    api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93c, bar,
    )?;
    let a = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa949, 21000,
    )?;
    let b = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa949, 21001,
    )?;
    let again = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa9bf, 21000,
    )?;
    result.insert(
        "installed".into(),
        json!([
            (a != 0) as u8,
            (b != 0) as u8,
            (menus.handle(&mut m, 20999) == 0) as u8,
            (again == a) as u8,
            menus.key(&mut m, &mut heap, b'n')?,
            menus.key(&mut m, &mut heap, b'c')?
        ]),
    );
    let snapshot = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93b, 0,
    )?;
    let second = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93b, 0,
    )?;
    let r = resources
        .iter()
        .find(|r| r.kind == *b"MENU" && r.id == 21000)
        .ok_or("missing menu fixture")?;
    let raw = load_resource(r, &mut m, &mut heap, &mut cache)?;
    result.insert(
        "snapshots".into(),
        json!([
            (snapshot != bar) as u8,
            (second != snapshot) as u8,
            (menus.handle(&mut m, 21000) == a) as u8,
            (raw == a) as u8
        ]),
    );
    heap.dispose(&mut m, second).map_err(|e| e.to_string())?;
    menus.remove(&mut m, &mut heap, 21000)?;
    result.insert(
        "deleted".into(),
        json!([
            (menus.handle(&mut m, 21000) == 0) as u8,
            menus.key(&mut m, &mut heap, b'n')?,
            menus.key(&mut m, &mut heap, b'c')?
        ]),
    );
    api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93c, snapshot,
    )?;
    heap.dispose(&mut m, snapshot).map_err(|e| e.to_string())?;
    result.insert(
        "restored".into(),
        json!([
            (menus.handle(&mut m, 21000) == a) as u8,
            (menus.handle(&mut m, 21001) == b) as u8,
            menus.key(&mut m, &mut heap, b'n')?
        ]),
    );
    api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93c, base,
    )?;
    heap.dispose(&mut m, bar).map_err(|e| e.to_string())?;
    result.insert(
        "base".into(),
        json!([
            (menus.handle(&mut m, 20999) == old) as u8,
            (menus.handle(&mut m, 21000) == 0) as u8,
            menus.key(&mut m, &mut heap, b'e')?
        ]),
    );
    let missing = api(
        &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa9c0, 31000,
    )?;
    result.insert(
        "missing".into(),
        json!([
            (missing == 0) as u8,
            m.read_word(0xa60) as i16,
            (menus.handle(&mut m, 20999) == old) as u8
        ]),
    );
    for (label, id) in [("empty", 21001), ("partial", 21002)] {
        let bar = api(
            &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa9c0, id,
        )?;
        let error = m.read_word(0xa60) as i16;
        result.insert(
            label.into(),
            if label == "empty" {
                json!([(bar != 0) as u8, error])
            } else {
                json!([
                    (bar != 0) as u8,
                    error,
                    (menus.handle(&mut m, 20999) == old) as u8
                ])
            },
        );
        if bar != 0 {
            api(
                &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93c, bar,
            )?;
            result.insert(
                format!("{label}_installed"),
                if label == "empty" {
                    json!([
                        (menus.handle(&mut m, 20999) == 0) as u8,
                        menus.key(&mut m, &mut heap, b'e')?
                    ])
                } else {
                    json!([
                        (menus.handle(&mut m, 21000) != 0) as u8,
                        (menus.handle(&mut m, 21001) != 0) as u8,
                        (menus.handle(&mut m, 30000) == 0) as u8,
                        menus.key(&mut m, &mut heap, b'n')?
                    ])
                },
            );
            api(
                &mut menus, &mut m, &mut heap, &fonts, &resources, &mut cache, 0xa93c, base,
            )?;
            heap.dispose(&mut m, bar).map_err(|e| e.to_string())?;
        }
    }
    Ok(json!({"schema":1,"probe":"menu-bars-v1","menus":result}).to_string())
}

#[cfg(test)]
mod menu_bar_tests {
    use super::*;
    #[test]
    fn malformed_menu_lists_leave_the_installed_bar_unchanged() {
        let mut m = Memory {
            bytes: vec![0; 0x10000],
        };
        let mut heap = crate::managers::Heap::new(0x2000, 0xe000);
        let mut menus = crate::menus::Menus::new();
        menus.init(&mut m, &mut heap).unwrap();
        let menu = heap.new_handle(&mut m, 20, true).unwrap();
        let data = heap.data(menu).unwrap();
        m.write_word(data, 100);
        menus.insert(&mut m, &mut heap, menu, 0).unwrap();
        let original_bar = m.read_long(0xa1c);
        let bad = heap.new_handle(&mut m, 6, true).unwrap();
        let data = heap.data(bad).unwrap();
        m.write_long(data, u32::from_be_bytes(*b"MBAR"));
        m.write_word(data + 4, 2);
        assert!(menus.set_bar(&mut m, &mut heap, bad).is_err());
        assert_eq!(menus.installed, vec![menu]);
        assert_eq!(m.read_long(0xa1c), original_bar);
        let fixture = Resource {
            kind: *b"MBAR",
            id: 7,
            name: None,
            attributes: 0,
            data: &[0, 2, 0, 100],
            map: &[],
            reference_offset: 0,
        };
        assert!(menus
            .resource_bar(
                &mut m,
                &mut heap,
                &crate::fonts::Fonts::new(),
                &[fixture],
                7,
                |_, _, _| panic!("truncated bar must fail before resource loading")
            )
            .is_err());
        assert_eq!(menus.installed, vec![menu]);
        assert_eq!(m.read_long(0xa1c), original_bar);
    }
}

pub fn systemstyle_probe() -> Result<String, String> {
    use serde_json::json;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut calibration = Vec::new();
    let mut labels = Vec::new();
    for face in 0..128u8 {
        let metrics = fonts.select(0, 12, face)?;
        calibration.push(json!({"face":face,"info":metrics.info,"widths":metrics.widths}));
        let widths: Result<Vec<i16>, String> = [
            b"Italic".as_slice(),
            b"Underline",
            b"Outline",
            b"Shadow",
            b"Condense",
            b"Extend",
            b"WAVy \x80 m",
            b"A A",
        ]
        .iter()
        .map(|text| fonts.text_width(0, 12, face, text))
        .collect();
        labels.push(widths?);
    }
    Ok(json!({"schema":1,"probe":"system-styles-v1","styles":{"calibration":calibration,"labels":labels}}).to_string())
}

pub fn fontcatalog_probe() -> Result<String, String> {
    use serde_json::json;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut cpu = CpuCore::new();
    let sp = 0x7e0000;
    let buffer = 0x110000;
    let output = 0x110200;
    let mut names = Vec::new();
    for id in [0i16, 1, 3, 4, 2002, 32767, -1] {
        for i in 0..256 {
            m.write_byte(buffer + i, 0xa5);
        }
        m.write_long(sp, buffer);
        m.write_word(sp + 4, id as u16);
        cpu.dar[15] = sp;
        cpu.pc = 0x19000;
        if !fonts.dispatch(0xa8ff, &mut cpu, &mut m) || cpu.dar[15] != sp + 6 || cpu.pc != 0x19002 {
            return Err("GetFontName ABI mismatch".into());
        }
        let n = m.read_byte(buffer) as u32;
        let text: Vec<u8> = (0..n).map(|i| m.read_byte(buffer + 1 + i)).collect();
        names.push(json!({"id":id,"name":text}));
    }
    let mut numbers = Vec::new();
    for name in [
        b"Chicago".as_slice(),
        b"chicago",
        b"Geneva",
        b"GENEVA",
        b"Monaco",
        b"Charcoal",
        b"System",
        b"Application",
        b"not-a-font",
    ] {
        m.write_byte(buffer, name.len() as u8);
        for (i, b) in name.iter().enumerate() {
            m.write_byte(buffer + 1 + i as u32, *b);
        }
        m.write_word(output, 12345);
        m.write_long(sp, output);
        m.write_long(sp + 4, buffer);
        cpu.dar[15] = sp;
        cpu.pc = 0x19000;
        if !fonts.dispatch(0xa900, &mut cpu, &mut m) || cpu.dar[15] != sp + 8 || cpu.pc != 0x19002 {
            return Err("GetFNum ABI mismatch".into());
        }
        numbers.push(m.read_word(output) as i16);
    }
    let mut menus = crate::menus::Menus::new();
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut observations = Vec::new();
    for (i, kind) in [*b"FONT", *b"FOND"].iter().enumerate() {
        let mut bytes = vec![0; 14];
        bytes[..2].copy_from_slice(&(21000u16 + i as u16).to_be_bytes());
        bytes[10..14].copy_from_slice(&u32::MAX.to_be_bytes());
        bytes.extend_from_slice(b"\x05Fonts\x08Existing\x00\x00\x00\x00\x00");
        let h = heap
            .new_handle(&mut m, bytes.len() as u32, false)
            .map_err(|e| e.to_string())?;
        let p = heap.data(h).map_err(|e| e.to_string())?;
        m.put(p as usize, &bytes)?;
        m.write_long(sp, u32::from_be_bytes(*kind));
        m.write_long(sp + 4, h);
        cpu.dar[15] = sp;
        cpu.pc = 0x19000;
        if !menus.dispatch(
            0xa94d,
            &mut cpu,
            &mut m,
            &mut heap,
            &fonts,
            &[],
            |_, _, _| Err("font metadata must not load an application resource".into()),
        )? || cpu.dar[15] != sp + 8
            || cpu.pc != 0x19002
        {
            return Err("AppendResMenu font ABI mismatch".into());
        }
        let items: Vec<Vec<u8>> = menus
            .items(&mut m, &mut heap, h)?
            .into_iter()
            .map(|item| item.text)
            .collect();
        observations.push(json!({"kind":u32::from_be_bytes(*kind),"items":items}));
    }
    Ok(json!({"schema":1,"probe":"font-catalog-v1","fonts":{"names":names,"numbers":numbers,"menus":observations}}).to_string())
}

pub fn fontprofile_probe() -> Result<String, String> {
    use serde_json::json;
    let raw: serde_json::Value =
        serde_json::from_str(&fontcatalog_probe()?).map_err(|e| e.to_string())?;
    let fonts = &raw["fonts"];
    let families: Vec<serde_json::Value> = [b"Charcoal".as_slice(), b"Geneva", b"Monaco"]
        .iter()
        .map(|s| json!(s))
        .collect();
    let queries: Vec<serde_json::Value> = [
        (2, b"Geneva".as_slice()),
        (3, b"GENEVA"),
        (4, b"Monaco"),
        (5, b"Charcoal"),
        (6, b"System"),
        (7, b"Application"),
        (8, b"not-a-font"),
    ]
    .iter()
    .map(|(i, name)| json!({"name":name,"number":fonts["numbers"][*i]}))
    .collect();
    let mut menus = Vec::new();
    for menu in fonts["menus"]
        .as_array()
        .ok_or("missing font menu observations")?
    {
        let items = menu["items"].as_array().ok_or("missing font menu items")?;
        let counts: Vec<usize> = families
            .iter()
            .map(|f| items.iter().filter(|item| *item == f).count())
            .collect();
        let known: Vec<&serde_json::Value> = items
            .iter()
            .skip(1)
            .filter(|item| families.contains(item))
            .collect();
        menus.push(
            json!({"kind":menu["kind"],"prefix":items[0],"counts":counts,"known_order":known}),
        );
    }
    Ok(json!({"schema":1,"probe":"font-profile-v1","fonts":{"names":fonts["names"],"queries":queries,"menus":menus}}).to_string())
}

/// Owned setup fixtures; every measured operation goes through the same trap
/// dispatcher as original applications, including Pascal stack layout.
struct MenuProbeState {
    m: Memory,
    heap: crate::managers::Heap,
    fonts: crate::fonts::Fonts,
    menus: crate::menus::Menus,
}
impl MenuProbeState {
    fn new() -> Result<Self, String> {
        let mut state = Self {
            m: Memory {
                bytes: vec![0; 8 * 1024 * 1024],
            },
            heap: crate::managers::Heap::new(0x20000, 0xe0000),
            fonts: crate::fonts::Fonts::new(),
            menus: crate::menus::Menus::new(),
        };
        state.fonts.init()?;
        state.menus.init(&mut state.m, &mut state.heap)?;
        Ok(state)
    }
    fn menu(&mut self, id: i16, title: &[u8], items: &[(&[u8], u8)]) -> Result<u32, String> {
        let mut bytes = vec![0; 14];
        bytes[0..2].copy_from_slice(&id.to_be_bytes());
        bytes[10..14].copy_from_slice(&u32::MAX.to_be_bytes());
        bytes.push(title.len() as u8);
        bytes.extend_from_slice(title);
        for (text, key) in items {
            bytes.push(text.len() as u8);
            bytes.extend_from_slice(text);
            bytes.extend_from_slice(&[0, *key, 0, 0]);
        }
        bytes.push(0);
        let h = self
            .heap
            .new_handle(&mut self.m, bytes.len() as u32, false)
            .map_err(|e| e.to_string())?;
        let p = self.m.read_long(h);
        self.m.put(p as usize, &bytes)?;
        self.menus
            .calculate(&mut self.m, &mut self.heap, &self.fonts, h)?;
        Ok(h)
    }
    fn api(&mut self, op: u16, h: u32, index: i16, arg: u32) -> Result<u32, String> {
        let sp = 0x7e0000;
        let output = 0x110000;
        let pop;
        match op {
            0xa84e | 0xa943 | 0xa93f | 0xa941 | 0xa946 => {
                for i in 0..256 {
                    self.m.write_byte(output + i, 0xa5);
                }
                self.m.write_word(output, 0x1234);
                self.m.write_long(sp, output);
                self.m.write_word(sp + 4, index as u16);
                self.m.write_long(sp + 6, h);
                pop = 10;
            }
            0xa84f | 0xa944 | 0xa940 | 0xa942 | 0xa945 => {
                self.m.write_word(sp, arg as u16);
                if op == 0xa940 || op == 0xa945 {
                    self.m.write_byte(sp, arg as u8);
                }
                self.m.write_word(sp + 2, index as u16);
                self.m.write_long(sp + 4, h);
                pop = 8;
            }
            0xa947 => {
                self.m.write_long(sp, arg);
                self.m.write_word(sp + 4, index as u16);
                self.m.write_long(sp + 6, h);
                pop = 10;
            }
            0xa950 | 0xa93c => {
                self.m.write_long(sp, h);
                pop = 4;
            }
            0xa935 | 0xa939 | 0xa93a => {
                self.m.write_word(sp, index as u16);
                self.m.write_long(sp + 2, h);
                pop = 6;
            }
            0xa936 | 0xa949 => {
                self.m.write_word(sp, index as u16);
                pop = 2;
            }
            0xa93e => {
                self.m.write_word(sp, arg as u16);
                pop = 2;
            }
            0xa93b => {
                pop = 0;
            }
            _ => return Err("unknown menu probe operation".into()),
        }
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        if !self.menus.dispatch(
            op,
            &mut cpu,
            &mut self.m,
            &mut self.heap,
            &self.fonts,
            &[],
            |_, _, _| Err("unexpected resource load".into()),
        )? || cpu.pc != 0x19002
            || cpu.dar[15] != sp + pop
        {
            return Err(format!("menu item Pascal ABI mismatch for {op:04x}"));
        }
        Ok(match op {
            0xa84e | 0xa943 | 0xa93f | 0xa941 => self.m.read_word(output) as u32,
            0xa946 => output,
            0xa950 => self.m.read_word(sp + pop) as u32,
            0xa93b | 0xa949 | 0xa93e => self.m.read_long(sp + pop),
            _ => 0,
        })
    }
    fn state(&mut self, h: u32, item: i16) -> Result<serde_json::Value, String> {
        let mut values = Vec::new();
        for op in [0xa84e, 0xa943, 0xa93f, 0xa941] {
            values.push(self.api(op, h, item, 0)?);
        }
        let p = self.api(0xa946, h, item, 0)?;
        let n = self.m.read_byte(p) as u32;
        let text: Vec<u8> = (0..n).map(|i| self.m.read_byte(p + 1 + i)).collect();
        Ok(serde_json::json!({"count":self.api(0xa950,h,0,0)?,"values":values,"text":text}))
    }
    fn rename(&mut self, h: u32, item: i16, text: &[u8]) -> Result<(), String> {
        let p = 0x110400;
        self.m.write_byte(p, text.len() as u8);
        for (i, b) in text.iter().enumerate() {
            self.m.write_byte(p + 1 + i as u32, *b);
        }
        self.api(0xa947, h, item, p)?;
        Ok(())
    }
    fn geometry(&mut self, h: u32) -> [i16; 2] {
        let p = self.m.read_long(h);
        [
            self.m.read_word(p + 2) as i16,
            self.m.read_word(p + 4) as i16,
        ]
    }
}
pub fn menuitems_probe() -> Result<String, String> {
    use serde_json::json;
    let mut s = MenuProbeState::new()?;
    let base = s.api(0xa93b, 0, 0, 0)?;
    let h = s.menu(
        21000,
        b"Items",
        &[(b"First", b'A'), (b"Second", 0), (b"Third", 0)],
    )?;
    s.api(0xa935, h, 0, 0)?;
    let mut result = serde_json::Map::new();
    result.insert("initial".into(), s.state(h, 1)?);
    let mut changes = Vec::new();
    for value in [0, 127, 128, 255, 256, 65535] {
        for op in [0xa84f, 0xa944, 0xa940, 0xa942] {
            s.api(op, h, 2, value)?;
        }
        changes.push(s.state(h, 2)?);
    }
    result.insert("set".into(), json!(changes));
    s.api(0xa945, h, 2, 1)?;
    result.insert("checked".into(), s.state(h, 2)?);
    s.api(0xa945, h, 2, 0)?;
    result.insert("unchecked".into(), s.state(h, 2)?);
    s.rename(h, 2, b"Longer label")?;
    result.insert("renamed".into(), s.state(h, 2)?);
    result.insert("invalid_zero".into(), s.state(h, 0)?);
    result.insert("invalid_high".into(), s.state(h, 99)?);
    let sub = s.menu(42, b"Sub", &[(b"Nested", b'N')])?;
    s.api(0xa84f, h, 3, 27)?;
    s.api(0xa944, h, 3, 42)?;
    s.api(0xa935, sub, -1, 0)?;
    let bar = s.api(0xa93b, 0, 0, 0)?;
    result.insert(
        "hierarchy_inserted".into(),
        json!([
            (s.api(0xa949, 0, 21000, 0)? == h) as u8,
            (s.api(0xa949, 0, 42, 0)? == sub) as u8,
            s.api(0xa93e, 0, 0, b'n' as u32)?
        ]),
    );
    s.api(0xa936, 0, 42, 0)?;
    result.insert(
        "hierarchy_deleted".into(),
        json!([
            (s.api(0xa949, 0, 42, 0)? == 0) as u8,
            s.api(0xa93e, 0, 0, b'n' as u32)?
        ]),
    );
    s.api(0xa93c, bar, 0, 0)?;
    s.heap.dispose(&mut s.m, bar).map_err(|e| e.to_string())?;
    result.insert(
        "hierarchy_restored".into(),
        json!([
            (s.api(0xa949, 0, 42, 0)? == sub) as u8,
            s.api(0xa93e, 0, 0, b'n' as u32)?
        ]),
    );
    s.api(0xa93c, base, 0, 0)?;
    Ok(json!({"schema":1,"probe":"menu-items-v1","menus":result}).to_string())
}
pub fn menusemantics_probe() -> Result<String, String> {
    use serde_json::json;
    let mut s = MenuProbeState::new()?;
    let h = s.menu(
        21000,
        b"Items",
        &[(b"First", b'A'), (b"Second", 0), (b"Third", 0)],
    )?;
    s.api(0xa935, h, 0, 0)?;
    let p = s.m.read_long(h);
    s.m.write_word(p + 2, 123);
    s.m.write_word(p + 4, 45);
    let mut geometry = vec![s.geometry(h)];
    for (op, value) in [
        (0xa84f, b'X' as u32),
        (0xa944, 18),
        (0xa940, 1),
        (0xa942, 1),
    ] {
        s.api(op, h, 2, value)?;
        geometry.push(s.geometry(h));
    }
    s.rename(h, 2, b"Longer label")?;
    geometry.push(s.geometry(h));
    let sub = s.menu(42, b"Sub", &[(b"Nested", b'N')])?;
    s.api(0xa935, sub, -1, 0)?;
    let mut result = serde_json::Map::new();
    result.insert("geometry".into(), json!(geometry));
    result.insert("unlinked".into(), json!(s.api(0xa93e, 0, 0, b'n' as u32)?));
    s.api(0xa84f, h, 3, 27)?;
    s.api(0xa944, h, 3, 42)?;
    result.insert("linked".into(), json!(s.api(0xa93e, 0, 0, b'n' as u32)?));
    for (name, target, item) in [
        ("parent_item_disabled", h, 3),
        ("parent_menu_disabled", h, 0),
        ("submenu_disabled", sub, 0),
        ("submenu_item_disabled", sub, 1),
    ] {
        s.api(0xa93a, target, item, 0)?;
        result.insert(name.into(), json!(s.api(0xa93e, 0, 0, b'n' as u32)?));
        s.api(0xa939, target, item, 0)?;
    }
    s.api(0xa944, h, 3, 0)?;
    result.insert(
        "unlinked_again".into(),
        json!(s.api(0xa93e, 0, 0, b'n' as u32)?),
    );
    Ok(json!({"schema":1,"probe":"menu-semantics-v1","menus":result}).to_string())
}

#[cfg(test)]
mod hierarchical_menu_tests {
    use super::*;
    #[test]
    fn hidden_menus_remain_separate_and_snapshot_validation_is_transactional() {
        let mut s = MenuProbeState::new().unwrap();
        let root = s.menu(21000, b"Root", &[(b"Item", 0)]).unwrap();
        let sub = s.menu(42, b"Sub", &[(b"Nested", b'N')]).unwrap();
        s.api(0xa935, root, 0, 0).unwrap();
        s.api(0xa935, sub, -1, 0).unwrap();
        assert_eq!(s.menus.installed, vec![root]);
        let bar = s.api(0xa93b, 0, 0, 0).unwrap();
        let p = s.m.read_long(bar);
        let live_bar = s.m.read_long(0xa1c);
        s.m.write_word(p + 6, 200);
        assert!(s.api(0xa93c, bar, 0, 0).is_err());
        assert_eq!(s.menus.installed, vec![root]);
        assert_eq!(s.api(0xa949, 0, 42, 0).unwrap(), sub);
        assert_eq!(s.m.read_long(0xa1c), live_bar);
        s.m.write_word(p + 6, 1);
        s.m.write_long(p + 12, root);
        assert!(s.api(0xa93c, bar, 0, 0).is_err());
        assert_eq!(s.api(0xa949, 0, 42, 0).unwrap(), sub);
    }
}

pub fn aehandlers_probe() -> Result<String, String> {
    use serde_json::json;
    struct Probe {
        m: Memory,
        ae: crate::appleevents::AppleEvents,
    }
    impl Probe {
        fn api(
            &mut self,
            selector: u16,
            handler: u32,
            refcon: u32,
            system: bool,
        ) -> Result<i16, String> {
            let sp = 0x7e0000;
            let output = 0x110000;
            self.m.write_word(sp, 0xa5a5);
            self.m.write_byte(sp, system as u8);
            let pop = if selector == 0x0720 {
                self.m.write_long(sp + 2, handler);
                self.m.write_long(sp + 6, u32::from_be_bytes(*b"Test"));
                self.m.write_long(sp + 10, u32::from_be_bytes(*b"CRt9"));
                14
            } else {
                self.m.write_long(
                    sp + 2,
                    if selector == 0x0921 {
                        output + 4
                    } else {
                        refcon
                    },
                );
                self.m
                    .write_long(sp + 6, if selector == 0x0921 { output } else { handler });
                self.m.write_long(sp + 10, u32::from_be_bytes(*b"Test"));
                self.m.write_long(sp + 14, u32::from_be_bytes(*b"CRt9"));
                18
            };
            self.m.write_word(sp + pop, 0x5a5a);
            let mut cpu = CpuCore::new();
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            cpu.dar[0] = 0x12340000 | selector as u32;
            if !self.ae.dispatch(0xa816, &mut cpu, &mut self.m)?
                || cpu.pc != 0x19002
                || cpu.dar[15] != sp + pop
            {
                return Err("Apple Event handler Pascal ABI mismatch".into());
            }
            Ok(self.m.read_word(sp + pop) as i16)
        }
        fn get(&mut self, system: bool) -> Result<serde_json::Value, String> {
            self.m.write_long(0x110000, 0x12345678);
            self.m.write_long(0x110004, 0x12345678);
            let error = self.api(0x0921, 0, 0, system)?;
            let h = self.m.read_long(0x110000);
            let r = self.m.read_long(0x110004) as i32;
            Ok(
                json!({"error":error,"handler":[(h==0x20000) as u8,(h==0x21000) as u8,(h==0x12345678) as u8,(h==0) as u8],"refcon":r}),
            )
        }
    }
    let mut s = Probe {
        m: Memory {
            bytes: vec![0; 8 * 1024 * 1024],
        },
        ae: crate::appleevents::AppleEvents::new(),
    };
    let mut result = serde_json::Map::new();
    result.insert("missing".into(), s.get(false)?);
    result.insert(
        "install".into(),
        json!(s.api(0x091f, 0x20000, 0x10203040, false)?),
    );
    result.insert("installed".into(), s.get(false)?);
    result.insert(
        "replace".into(),
        json!(s.api(0x091f, 0x21000, (-123i32) as u32, false)?),
    );
    result.insert("replaced".into(), s.get(false)?);
    result.insert(
        "remove_wrong".into(),
        json!(s.api(0x0720, 0x20000, 0, false)?),
    );
    result.insert("after_wrong".into(), s.get(false)?);
    result.insert("remove".into(), json!(s.api(0x0720, 0x21000, 0, false)?));
    result.insert("removed".into(), s.get(false)?);
    result.insert(
        "remove_again".into(),
        json!(s.api(0x0720, 0x21000, 0, false)?),
    );
    result.insert("nil_install".into(), json!(s.api(0x091f, 0, 99, false)?));
    result.insert("nil_lookup".into(), s.get(false)?);
    result.insert("nil_remove".into(), json!(s.api(0x0720, 0, 0, false)?));
    result.insert(
        "system_install".into(),
        json!(s.api(0x091f, 0x20000, 456, true)?),
    );
    result.insert("system_lookup".into(), s.get(true)?);
    result.insert("application_lookup".into(), s.get(false)?);
    result.insert(
        "system_remove".into(),
        json!(s.api(0x0720, 0x20000, 0, true)?),
    );
    result.insert("system_removed".into(), s.get(true)?);
    Ok(json!({"schema":1,"probe":"apple-event-handlers-v1","handlers":result}).to_string())
}

struct WindowResourceProbeState<'a> {
    m: Memory,
    heap: crate::managers::Heap,
    q: crate::quickdraw::QuickDraw,
    fonts: crate::fonts::Fonts,
    events: crate::events::Events,
    windows: crate::windows::Windows,
    resources: &'a [Resource<'a>],
    cache: std::collections::BTreeMap<([u8; 4], i16), u32>,
}
impl<'a> WindowResourceProbeState<'a> {
    fn new(resources: &'a [Resource<'a>]) -> Result<Self, String> {
        let mut s = Self {
            m: Memory {
                bytes: vec![0; 8 * 1024 * 1024],
            },
            heap: crate::managers::Heap::new(0x20000, 0xe0000),
            q: crate::quickdraw::QuickDraw::new(),
            fonts: crate::fonts::Fonts::new(),
            events: crate::events::Events::new(),
            windows: crate::windows::Windows::new(),
            resources,
            cache: std::collections::BTreeMap::new(),
        };
        s.q.init(&mut s.m, 0x100000)?;
        s.fonts.init()?;
        s.windows.init(&mut s.m, &mut s.heap, &s.q)?;
        let map = resources
            .first()
            .ok_or("missing window fixture resource map")?
            .map;
        let h = s
            .heap
            .new_handle(&mut s.m, map.len() as u32, false)
            .map_err(|e| e.to_string())?;
        let p = s.m.read_long(h);
        s.m.put(p as usize, map)?;
        s.m.write_long(0xa50, h);
        Ok(s)
    }
    fn api(&mut self, op: u16, w: u32, arg: u32) -> Result<u32, String> {
        let sp = 0x7e0000;
        let pop = match op {
            0xa9bd | 0xaa46 => {
                self.m.write_long(sp, u32::MAX);
                self.m.write_long(sp + 4, w);
                self.m.write_word(sp + 8, arg as u16);
                10
            }
            0xa917 => {
                self.m.write_long(sp, w);
                4
            }
            0xa918 | 0xa919 | 0xa91a => {
                self.m.write_long(sp, arg);
                self.m.write_long(sp + 4, w);
                8
            }
            _ => return Err("unknown resource-window probe operation".into()),
        };
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        let cache = &mut self.cache;
        if !self.windows.dispatch(
            op,
            &mut cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.q,
            &self.fonts,
            &mut self.events,
            self.resources,
            |r, m, heap| load_resource(r, m, heap, cache),
        )? || cpu.pc != 0x19002
            || cpu.dar[15] != sp + pop
        {
            return Err("resource-window Pascal ABI mismatch".into());
        }
        Ok(if [0xa9bd, 0xaa46, 0xa917].contains(&op) {
            self.m.read_long(sp + pop)
        } else {
            0
        })
    }
    fn title(&mut self, w: u32) -> Result<Vec<u8>, String> {
        let buffer = 0x110000;
        for i in 0..256 {
            self.m.write_byte(buffer + i, 0xa5);
        }
        self.api(0xa919, w, buffer)?;
        let n = self.m.read_byte(buffer) as u32;
        Ok((0..n).map(|i| self.m.read_byte(buffer + 1 + i)).collect())
    }
    fn basic(&mut self, id: i16, color: bool) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let before = self.q.get_port(&mut self.m)?;
        let w = self.api(if color { 0xaa46 } else { 0xa9bd }, 0, id as u16 as u32)?;
        let error = self.m.read_word(0xa60) as i16;
        let result = if w == 0 {
            json!({"nonnull":0,"resource_error":error})
        } else {
            let title = self.title(w)?;
            let cr = self.m.read_long(w + 118);
            let cp = self.m.read_long(cr);
            let sr = self.m.read_long(w + 114);
            let sp = self.m.read_long(sr);
            let rectangle = |r: crate::managers::Rect| json!([r.top, r.left, r.bottom, r.right]);
            json!({"nonnull":1,"resource_error":error,"color":((self.m.read_word(w+6)&0xc000)==0xc000) as u8,"visible":self.m.read_byte(w+110),"kind":self.m.read_word(w+108),"go_away":self.m.read_byte(w+112),"refcon":self.api(0xa917,w,0)? as i32,"current":(self.q.get_port(&mut self.m)?==w) as u8,"title":title,"port_rect":rectangle(crate::quickdraw::read_rect(&mut self.m,w+16)),"content":rectangle(crate::quickdraw::read_rect(&mut self.m,cp+2)),"structure":rectangle(crate::quickdraw::read_rect(&mut self.m,sp+2))})
        };
        if w != 0 {
            self.windows
                .dispose(&mut self.m, &mut self.heap, &self.q, &mut self.events, w)?;
        }
        self.q.set_port(&mut self.m, before)?;
        Ok(result)
    }
    fn snapshot(&mut self, w: u32) -> Result<serde_json::Value, String> {
        let mut result = serde_json::Map::new();
        result.insert(
            "visible".into(),
            serde_json::json!(self.m.read_byte(w + 110)),
        );
        result.insert(
            "refcon".into(),
            serde_json::json!(self.api(0xa917, w, 0)? as i32),
        );
        let r = crate::quickdraw::read_rect(&mut self.m, w + 16);
        result.insert(
            "port_rect".into(),
            serde_json::json!([r.top, r.left, r.bottom, r.right]),
        );
        for (name, offset) in [
            ("content", 118),
            ("structure", 114),
            ("update", 122),
            ("vis", 24),
        ] {
            let h = self.m.read_long(w + offset);
            let p = self.m.read_long(h);
            let r = crate::quickdraw::read_rect(&mut self.m, p + 2);
            result.insert(
                name.into(),
                serde_json::json!([r.top, r.left, r.bottom, r.right]),
            );
        }
        Ok(serde_json::Value::Object(result))
    }
    fn flow(&mut self, color: bool) -> Result<serde_json::Value, String> {
        let w = self.api(
            if color { 0xaa46 } else { 0xa9bd },
            0,
            if color { 21001 } else { 21000 },
        )?;
        if w == 0 {
            return Err("missing resource-window fixture".into());
        }
        let mut result = serde_json::Map::new();
        result.insert("initial".into(), self.snapshot(w)?);
        self.api(0xa918, w, (-987654i32) as u32)?;
        self.m.put(0x110400, b"\x07Changed")?;
        self.api(0xa91a, w, 0x110400)?;
        result.insert("title".into(), serde_json::json!(self.title(w)?));
        result.insert("changed".into(), self.snapshot(w)?);
        self.windows
            .move_hidden(&mut self.m, &mut self.heap, &self.q, w, 120, 100, false)?;
        result.insert("moved".into(), self.snapshot(w)?);
        self.windows.resize(
            &mut self.m,
            &mut self.heap,
            &self.q,
            &self.fonts,
            &mut self.events,
            w,
            240,
            100,
            true,
        )?;
        result.insert("resized".into(), self.snapshot(w)?);
        for (name, visible) in [("shown", true), ("hidden", false), ("reshown", true)] {
            self.windows.visible(
                &mut self.m,
                &mut self.heap,
                &self.q,
                &self.fonts,
                &mut self.events,
                w,
                visible,
            )?;
            result.insert(name.into(), self.snapshot(w)?);
        }
        self.windows
            .dispose(&mut self.m, &mut self.heap, &self.q, &mut self.events, w)?;
        Ok(serde_json::Value::Object(result))
    }
}
pub fn windowresources_probe(flow: bool) -> Result<String, String> {
    let bytes =
        std::fs::read("work/probes/windowresources/WindowResourcesProbe.APPL/..namedfork/rsrc")
            .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let mut s = WindowResourceProbeState::new(&resources)?;
    let mut result = serde_json::Map::new();
    if flow {
        result.insert("mono".into(), s.flow(false)?);
        result.insert("color".into(), s.flow(true)?);
    } else {
        for (name, id, color) in [
            ("mono", 21000, false),
            ("color", 21001, true),
            ("missing_mono", 31000, false),
            ("missing_color", 31000, true),
        ] {
            result.insert(name.into(), s.basic(id, color)?);
        }
    }
    Ok(serde_json::json!({"schema":1,"probe":if flow {"window-resource-flow-v1"} else {"window-resources-v1"},"windows":result}).to_string())
}

#[cfg(test)]
mod window_resource_tests {
    use super::*;
    #[test]
    fn malformed_cached_wind_does_not_register_a_window() {
        let mut bytes = vec![0u8; 24];
        for (i, value) in [70i16, 90, 190, 350].iter().enumerate() {
            bytes[i * 2..i * 2 + 2].copy_from_slice(&value.to_be_bytes());
        }
        bytes[12] = 1;
        bytes[18..22].copy_from_slice(b"\x03Foo");
        let map = vec![0u8; 64];
        let resources = [Resource {
            kind: *b"WIND",
            id: 21000,
            name: None,
            attributes: 0,
            data: &bytes,
            map: &map,
            reference_offset: 0,
        }];
        let mut s = WindowResourceProbeState::new(&resources).unwrap();
        s.basic(21000, false).unwrap();
        let h = s.cache[&(*b"WIND", 21000)];
        let p = s.m.read_long(h);
        let before = s.q.get_port(&mut s.m).unwrap();
        s.m.write_byte(p + 18, 255);
        assert!(s
            .api(0xa9bd, 0, 21000)
            .unwrap_err()
            .contains("truncated WIND title"));
        assert!(s.windows.windows.is_empty());
        assert_eq!(s.q.get_port(&mut s.m).unwrap(), before);
    }
}

struct StyledProbe {
    m: Memory,
    heap: crate::managers::Heap,
    q: crate::quickdraw::QuickDraw,
    fonts: crate::fonts::Fonts,
    te: crate::textedit::TextEdit,
    events: crate::events::Events,
    callbacks: crate::callbacks::Callbacks,
    windows: crate::windows::Windows,
}
impl StyledProbe {
    fn new() -> Result<Self, String> {
        let mut s = Self {
            m: Memory {
                bytes: vec![0; 8 * 1024 * 1024],
            },
            heap: crate::managers::Heap::new(0x20000, 0xe0000),
            q: crate::quickdraw::QuickDraw::new(),
            fonts: crate::fonts::Fonts::new(),
            te: crate::textedit::TextEdit::new(),
            events: crate::events::Events::new(),
            callbacks: crate::callbacks::Callbacks::new(),
            windows: crate::windows::Windows::new(),
        };
        s.q.init(&mut s.m, 0x100000)?;
        s.fonts.init()?;
        s.windows.init(&mut s.m, &mut s.heap, &s.q)?;
        s.port_style(3, 0, 12)?;
        Ok(s)
    }
    fn port_style(&mut self, font: i16, face: u8, size: i16) -> Result<(), String> {
        let p = self.q.get_port(&mut self.m)?;
        self.m.write_word(p + 68, font as u16);
        self.m.write_byte(p + 70, face);
        self.m.write_word(p + 74, size as u16);
        Ok(())
    }
    fn api(&mut self, opcode: u16, selector: u16, args: &[(u32, u32)]) -> Result<u32, String> {
        let sp = 0x7e0000;
        let mut offset = if opcode == 0xa83d {
            self.m.write_word(sp, selector);
            2
        } else {
            0
        };
        for (value, size) in args {
            match size {
                1 => {
                    self.m.write_byte(sp + offset, *value as u8);
                    offset += 2;
                }
                2 => {
                    self.m.write_word(sp + offset, *value as u16);
                    offset += 2;
                }
                4 => {
                    self.m.write_long(sp + offset, *value);
                    offset += 4;
                }
                _ => return Err("invalid styled probe ABI field".into()),
            }
        }
        self.m.write_long(sp + offset, 0xa5a5a5a5);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        if !self.te.dispatch(
            opcode,
            &mut cpu,
            &mut self.m,
            &mut self.heap,
            &self.q,
            &self.fonts,
            &self.events,
            &mut self.callbacks,
        )? || cpu.pc != 0x19002
            || cpu.dar[15] != sp + offset
        {
            return Err(format!(
                "styled TextEdit Pascal ABI mismatch {opcode:04x}/{selector:04x}"
            ));
        }
        Ok(self.m.read_long(sp + offset))
    }
    fn snapshot_scrap(&mut self, h: u32) -> Result<serde_json::Value, String> {
        if h == 0 {
            return Ok(serde_json::Value::Null);
        }
        let p = self.m.read_long(h);
        let n = self.m.read_word(p) as usize;
        if n > 64 {
            return Err("probe style-scrap count out of range".into());
        }
        let rows: Vec<serde_json::Value> = (0..n)
            .map(|i| {
                let p = p + 2 + 20 * i as u32;
                serde_json::json!([
                    self.m.read_long(p) as i32,
                    self.m.read_word(p + 4) as i16,
                    self.m.read_word(p + 6) as i16,
                    self.m.read_word(p + 8) as i16,
                    self.m.read_byte(p + 10),
                    self.m.read_word(p + 12) as i16,
                    self.m.read_word(p + 14),
                    self.m.read_word(p + 16),
                    self.m.read_word(p + 18)
                ])
            })
            .collect();
        Ok(serde_json::json!(rows))
    }
    fn state(&mut self, h: u32) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let sh = self.api(0xa83d, 4, &[(h, 4)])?;
        let p = self.m.read_long(h);
        let s = self.m.read_long(sh);
        let runs = self.m.read_word(s) as usize;
        let styles = self.m.read_word(s + 2) as usize;
        let lines = self.m.read_word(p + 94) as usize;
        let run_rows: Vec<_> = (0..=runs)
            .map(|i| {
                json!([
                    self.m.read_word(s + 20 + 4 * i as u32) as i16,
                    self.m.read_word(s + 22 + 4 * i as u32) as i16
                ])
            })
            .collect();
        let th = self.m.read_long(s + 4);
        let tp = self.m.read_long(th);
        let style_rows: Vec<_> = (0..styles)
            .map(|i| {
                let p = tp + 18 * i as u32;
                json!([
                    self.m.read_word(p) as i16,
                    self.m.read_word(p + 2) as i16,
                    self.m.read_word(p + 4) as i16,
                    self.m.read_word(p + 6) as i16,
                    self.m.read_byte(p + 8),
                    self.m.read_word(p + 10) as i16,
                    self.m.read_word(p + 12),
                    self.m.read_word(p + 14),
                    self.m.read_word(p + 16)
                ])
            })
            .collect();
        let lh = self.m.read_long(s + 8);
        let lp = self.m.read_long(lh);
        let heights: Vec<_> = (0..lines.max(1))
            .map(|i| {
                json!([
                    self.m.read_word(lp + 4 * i as u32) as i16,
                    self.m.read_word(lp + 4 * i as u32 + 2) as i16
                ])
            })
            .collect();
        let pointer_offsets: Vec<_> = (0..=94)
            .step_by(2)
            .filter(|i| self.m.read_long(p + i) == sh)
            .collect();
        let starts: Vec<_> = (0..=lines)
            .map(|i| self.m.read_word(p + 96 + 2 * i as u32) as i16)
            .collect();
        let null = self.m.read_long(s + 16);
        let np = self.m.read_long(null);
        let scrap = self.m.read_long(np + 4);
        let null_scrap = self.snapshot_scrap(scrap)?;
        let a = self.api(0xa83d, 6, &[(h, 4)])?;
        let b = self.api(0xa83d, 6, &[(h, 4)])?;
        let selection_scrap = self.snapshot_scrap(a)?;
        Ok(
            json!({"length":self.m.read_word(p+60),"selection":[self.m.read_word(p+32),self.m.read_word(p+34)],"line_height":self.m.read_word(p+24) as i16,"font_ascent":self.m.read_word(p+26) as i16,"n_lines":lines,"text":self.te.text(&mut self.m,h)?,"line_starts":starts,"style_pointer_offsets":pointer_offsets,"n_runs":runs,"n_styles":styles,"refcon":self.m.read_long(s+12) as i32,"runs":run_rows,"styles":style_rows,"heights":heights,"null_scrap":null_scrap,"scrap_identity":[(a!=0) as u8,(a==b) as u8,(a==scrap) as u8],"selection_scrap":selection_scrap}),
        )
    }
}
pub fn styledtext_probe(variant: bool) -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    let rect = crate::managers::Rect {
        top: 30,
        left: 20,
        bottom: 200,
        right: 300,
    };
    crate::quickdraw::write_rect(&mut s.m, 0x110000, rect);
    let h = s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)])?;
    let sh = s.api(0xa83d, 4, &[(h, 4)])?;
    let p = s.m.read_long(sh);
    s.m.write_long(p + 12, 0x12345678);
    let mut result = serde_json::Map::new();
    result.insert("initial".into(), s.state(h)?);
    let style = crate::styledtext::Style::new(4, 1, 9, [0; 3], &s.fonts)?;
    style.write_attributes(&mut s.m, 0x110400);
    s.api(0xa83d, 1, &[(h, 4), (0, 1), (0x110400, 4), (15, 2)])?;
    result.insert("insertion_style".into(), s.state(h)?);
    for byte in b"AB\rC" {
        s.api(0xa9dc, 0, &[(h, 4), (*byte as u32, 2)])?;
    }
    result.insert("typed".into(), s.state(h)?);
    s.api(0xa9d1, 0, &[(h, 4), (3, 4), (1, 4)])?;
    let style = crate::styledtext::Style::new(3, 1, 12, [0; 3], &s.fonts)?;
    style.write_attributes(&mut s.m, 0x110400);
    s.api(0xa83d, 1, &[(h, 4), (0, 1), (0x110400, 4), (5, 2)])?;
    result.insert("mixed".into(), s.state(h)?);
    s.api(0xa9d7, 0, &[(h, 4)])?;
    result.insert("deleted".into(), s.state(h)?);
    if variant {
        s.api(0xa9d1, 0, &[(h, 4), (2, 4), (0, 4)])?;
    }
    s.m.put(0x110500, b"xy")?;
    s.api(0xa83d, 7, &[(h, 4), (0, 4), (2, 4), (0x110500, 4)])?;
    result.insert("inserted".into(), s.state(h)?);
    if variant {
        s.port_style(4, 1, 9)?;
    }
    s.m.put(0x110500, b"one\rtwo")?;
    s.api(0xa9cf, 0, &[(h, 4), (7, 4), (0x110500, 4)])?;
    result.insert("set_text".into(), s.state(h)?);
    s.api(0xa9cd, 0, &[(h, 4)])?;
    Ok(serde_json::json!({"schema":1,"probe":"styled-text-v1","textedit":result}).to_string())
}

impl StyledProbe {
    fn queries(&mut self, h: u32) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let p = self.m.read_long(h);
        let count = self.m.read_word(p + 94);
        let length = self.m.read_word(p + 60);
        let mut heights = Vec::new();
        for (end, start) in [(1, 1), (0, 0), (0, 1), (1, 0), (2, 1), (2, 2)]
            .iter()
            .take(if count > 1 { 6 } else { 4 })
        {
            let height = self.api(0xa83d, 9, &[(h, 4), (*start, 4), (*end, 4)])? as i32;
            heights.push(json!([end, start, height]));
        }
        let mut styles = Vec::new();
        for offset in [0, length, length / 2] {
            let attributes = 0x110800;
            let height = 0x110810;
            let ascent = 0x110812;
            for i in 0..12 {
                self.m.write_byte(attributes + i, 0xa5);
            }
            self.m.write_word(height, (-1234i16) as u16);
            self.m.write_word(ascent, (-1234i16) as u16);
            self.api(
                0xa83d,
                3,
                &[
                    (h, 4),
                    (ascent, 4),
                    (height, 4),
                    (attributes, 4),
                    (offset as u32, 2),
                ],
            )?;
            let point = self.api(0xa83d, 8, &[(h, 4), (offset as u32, 2)])?;
            styles.push(json!({"offset":offset,"style":[self.m.read_word(attributes) as i16,self.m.read_byte(attributes+2),self.m.read_word(attributes+4) as i16,self.m.read_word(attributes+6),self.m.read_word(attributes+8),self.m.read_word(attributes+10)],"metrics":[self.m.read_word(height) as i16,self.m.read_word(ascent) as i16],"point":[(point>>16) as u16 as i16,point as u16 as i16]}));
        }
        Ok(json!({"heights":heights,"styles":styles}))
    }
}
pub fn stylequeries_probe() -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    crate::quickdraw::write_rect(
        &mut s.m,
        0x110000,
        crate::managers::Rect {
            top: 30,
            left: 20,
            bottom: 200,
            right: 300,
        },
    );
    let h = s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)])?;
    let mut result = serde_json::Map::new();
    result.insert("initial".into(), s.queries(h)?);
    crate::styledtext::Style::new(4, 1, 9, [0; 3], &s.fonts)?.write_attributes(&mut s.m, 0x110400);
    s.api(0xa83d, 1, &[(h, 4), (0, 1), (0x110400, 4), (15, 2)])?;
    result.insert("pending".into(), s.queries(h)?);
    for byte in b"AB\rC" {
        s.api(0xa9dc, 0, &[(h, 4), (*byte as u32, 2)])?;
    }
    result.insert("typed".into(), s.queries(h)?);
    s.api(0xa9d1, 0, &[(h, 4), (3, 4), (1, 4)])?;
    crate::styledtext::Style::new(3, 1, 12, [0; 3], &s.fonts)?.write_attributes(&mut s.m, 0x110400);
    s.api(0xa83d, 1, &[(h, 4), (0, 1), (0x110400, 4), (5, 2)])?;
    result.insert("mixed".into(), s.queries(h)?);
    s.api(0xa9d0, 0, &[(h, 4)])?;
    result.insert("recalculated".into(), s.queries(h)?);
    s.api(0xa9cd, 0, &[(h, 4)])?;
    Ok(serde_json::json!({"schema":1,"probe":"style-queries-v1","textedit":result}).to_string())
}

#[cfg(test)]
mod styled_editor_tests {
    use super::*;
    fn create(s: &mut StyledProbe) -> u32 {
        crate::quickdraw::write_rect(
            &mut s.m,
            0x110000,
            crate::managers::Rect {
                top: 30,
                left: 20,
                bottom: 200,
                right: 300,
            },
        );
        s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)]).unwrap()
    }
    fn bytes(s: &mut StyledProbe, h: u32) -> Vec<u8> {
        let n = s.heap.size(h).unwrap();
        let p = s.m.read_long(h);
        (0..n).map(|i| s.m.read_byte(p + i)).collect()
    }
    #[test]
    fn oversized_styled_insertion_preserves_text_runs_and_pending_style() {
        let mut s = StyledProbe::new().unwrap();
        let h = create(&mut s);
        let text = vec![b'a'; 32767];
        s.m.put(0x120000, &text).unwrap();
        s.api(0xa9cf, 0, &[(h, 4), (32767, 4), (0x120000, 4)])
            .unwrap();
        let p = s.m.read_long(h);
        let c = crate::styledtext::Context::read(&mut s.m, p).unwrap();
        let handles = [c.handle, c.table, c.heights, c.null, c.scrap];
        let before: Vec<_> = handles.iter().map(|h| bytes(&mut s, *h)).collect();
        assert!(s.api(0xa9dc, 0, &[(h, 4), (b'b' as u32, 2)]).is_err());
        assert_eq!(s.te.text(&mut s.m, h).unwrap(), text);
        let after: Vec<_> = handles.iter().map(|h| bytes(&mut s, *h)).collect();
        assert_eq!(before, after);
    }
    #[test]
    fn mixed_style_redraw_changes_ink_only_in_view_preserves_port_and_releases_internal_handles() {
        let mut s = StyledProbe::new().unwrap();
        let h = create(&mut s);
        s.m.put(0x110500, b"ABCD").unwrap();
        s.api(0xa9cf, 0, &[(h, 4), (4, 4), (0x110500, 4)]).unwrap();
        s.api(0xa9d3, 0, &[(h, 4), (0x110000, 4)]).unwrap();
        let p = s.m.read_long(h);
        let port = s.m.read_long(p + 82);
        let port_before: Vec<_> = (0..108).map(|i| s.m.read_byte(port + i)).collect();
        let base = s.q.screen_base;
        let row = s.q.row_bytes() as u32;
        let before: Vec<_> = (0..row * s.q.height as u32)
            .map(|i| s.m.read_byte(base + i))
            .collect();
        s.api(0xa9d1, 0, &[(h, 4), (3, 4), (1, 4)]).unwrap();
        crate::styledtext::Style::new(4, 1, 9, [0; 3], &s.fonts)
            .unwrap()
            .write_attributes(&mut s.m, 0x110400);
        s.api(0xa83d, 1, &[(h, 4), (1, 1), (0x110400, 4), (7, 2)])
            .unwrap();
        let after: Vec<_> = (0..row * s.q.height as u32)
            .map(|i| s.m.read_byte(base + i))
            .collect();
        let mut changed = 0;
        for y in 0..s.q.height as u32 {
            for x in 0..s.q.width as u32 {
                let at = (y * row + x / 8) as usize;
                let bit = 0x80 >> (x % 8);
                if (before[at] ^ after[at]) & bit != 0 {
                    assert!((20..300).contains(&x) && (30..200).contains(&y));
                    changed += 1;
                }
            }
        }
        assert!(changed > 0);
        let port_after: Vec<_> = (0..108).map(|i| s.m.read_byte(port + i)).collect();
        assert_eq!(port_before, port_after);
        let p = s.m.read_long(h);
        let c = crate::styledtext::Context::read(&mut s.m, p).unwrap();
        let text = s.m.read_long(p + 62);
        let handles = [h, text, c.handle, c.table, c.heights, c.null, c.scrap];
        s.api(0xa9cd, 0, &[(h, 4)]).unwrap();
        for h in handles {
            assert!(s.heap.size(h).is_err());
        }
    }
}

impl StyledProbe {
    fn auto_state(&mut self, h: u32) -> Result<serde_json::Value, String> {
        let flag = (self.api(0xa83d, 14, &[(h, 4), (65535, 2), (0, 2)])? >> 16) as u16 as i16;
        let p = self.m.read_long(h);
        let rectangle =
            |r: crate::managers::Rect| serde_json::json!([r.top, r.left, r.bottom, r.right]);
        Ok(
            serde_json::json!({"auto":flag,"click_flags":self.m.read_word(p+70) as i16,"cr_only":self.m.read_word(p+72) as i16,"selection":[self.m.read_word(p+32),self.m.read_word(p+34)],"n_lines":self.m.read_word(p+94),"dest":rectangle(crate::quickdraw::read_rect(&mut self.m,p)),"view":rectangle(crate::quickdraw::read_rect(&mut self.m,p+8))}),
        )
    }
    fn auto_flow(&mut self, styled: bool) -> Result<serde_json::Value, String> {
        crate::quickdraw::write_rect(
            &mut self.m,
            0x110000,
            crate::managers::Rect {
                top: 30,
                left: 20,
                bottom: 200,
                right: 120,
            },
        );
        crate::quickdraw::write_rect(
            &mut self.m,
            0x110100,
            crate::managers::Rect {
                top: 30,
                left: 20,
                bottom: 50,
                right: 120,
            },
        );
        let h = self.api(
            if styled { 0xa83e } else { 0xa9d2 },
            0,
            &[(0x110100, 4), (0x110000, 4)],
        )?;
        let mut result = serde_json::Map::new();
        result.insert("initial".into(), self.auto_state(h)?);
        self.api(0xa9d8, 0, &[(h, 4)])?;
        self.api(0xa813, 0, &[(h, 4), (1, 1)])?;
        result.insert("enabled".into(), self.auto_state(h)?);
        let mut typing = Vec::new();
        for byte in b"A\rB\rC" {
            self.api(0xa9dc, 0, &[(h, 4), (*byte as u32, 2)])?;
            typing.push(self.auto_state(h)?);
        }
        result.insert("typing".into(), serde_json::json!(typing));
        self.api(0xa813, 0, &[(h, 4), (0, 1)])?;
        result.insert("disabled".into(), self.auto_state(h)?);
        self.api(0xa9d1, 0, &[(h, 4), (0, 4), (0, 4)])?;
        let set = (self.api(0xa83d, 14, &[(h, 4), (1, 2), (0, 2)])? >> 16) as u16 as i16;
        result.insert("set_result".into(), serde_json::json!(set));
        result.insert("set".into(), self.auto_state(h)?);
        self.api(0xa811, 0, &[(h, 4)])?;
        result.insert("selection_view".into(), self.auto_state(h)?);
        let clear = (self.api(0xa83d, 14, &[(h, 4), (0, 2), (0, 2)])? >> 16) as u16 as i16;
        result.insert("clear_result".into(), serde_json::json!(clear));
        self.api(
            0xa9dd,
            0,
            &[
                (h, 4),
                ((-5i16) as u16 as u32, 2),
                ((-7i16) as u16 as u32, 2),
            ],
        )?;
        result.insert("scrolled".into(), self.auto_state(h)?);
        self.api(0xa812, 0, &[(h, 4), (1000, 2), (1000, 2)])?;
        result.insert("pinned_high".into(), self.auto_state(h)?);
        self.api(
            0xa812,
            0,
            &[
                (h, 4),
                ((-1000i16) as u16 as u32, 2),
                ((-1000i16) as u16 as u32, 2),
            ],
        )?;
        result.insert("pinned_low".into(), self.auto_state(h)?);
        self.api(0xa9cd, 0, &[(h, 4)])?;
        Ok(serde_json::Value::Object(result))
    }
}
pub fn teautoview_probe() -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    s.port_style(4, 0, 9)?;
    let mut rows = serde_json::Map::new();
    rows.insert("plain".into(), s.auto_flow(false)?);
    rows.insert("styled".into(), s.auto_flow(true)?);
    Ok(serde_json::json!({"schema":1,"probe":"te-auto-view-v1","textedit":rows}).to_string())
}

impl StyledProbe {
    fn probe_rect(&mut self, right: i16) {
        crate::quickdraw::write_rect(
            &mut self.m,
            0x110000,
            crate::managers::Rect {
                top: 30,
                left: 20,
                bottom: 200,
                right,
            },
        );
    }
    fn direct_select(&mut self, h: u32, start: u16, end: u16) {
        let p = self.m.read_long(h);
        self.m.write_word(p + 32, start);
        self.m.write_word(p + 34, end);
    }
    fn probe_text(&mut self, h: u32, text: &[u8]) -> Result<(), String> {
        for (i, b) in text.iter().enumerate() {
            self.m.write_byte(0x110200 + i as u32, *b);
        }
        self.api(0xa9cf, 0, &[(h, 4), (text.len() as u32, 4), (0x110200, 4)])?;
        Ok(())
    }
    fn continuous_queries(&mut self, h: u32) -> Result<serde_json::Value, String> {
        let mut rows = Vec::new();
        for requested in [0, 1, 2, 4, 8, 15] {
            self.m.write_word(0x110500, requested);
            for i in 0..12 {
                self.m.write_byte(0x110400 + i, 0xa5);
            }
            let result = self.api(0xa83d, 10, &[(h, 4), (0x110400, 4), (0x110500, 4)])? >> 24;
            let mode = self.m.read_word(0x110500);
            let mut attrs = vec![serde_json::Value::Null; 6];
            for (i, bit, offset) in [(0, 1, 0), (2, 4, 4), (3, 8, 6), (4, 8, 8), (5, 8, 10)] {
                if mode & bit != 0 {
                    let n = self.m.read_word(0x110400 + offset);
                    attrs[i] = serde_json::json!(if i < 3 { n as i16 as i32 } else { n as i32 });
                }
            }
            if mode & 2 != 0 {
                attrs[1] = serde_json::json!(self.m.read_byte(0x110402));
            }
            rows.push(serde_json::json!({"requested":requested,"mode":mode,"result_byte":result,"attributes":attrs}));
        }
        Ok(serde_json::json!(rows))
    }
    fn height_ranges(&mut self, h: u32) -> Result<serde_json::Value, String> {
        let mut rows = Vec::new();
        for (end, start) in [
            (0i32, 0i32),
            (32767, 0),
            (32767, 1),
            (0, 32767),
            (32767, 32767),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (2, 1),
            (1, 2),
        ] {
            let height = self.api(0xa83d, 9, &[(h, 4), (start as u32, 4), (end as u32, 4)])? as i32;
            rows.push(serde_json::json!([end, start, height]));
        }
        Ok(serde_json::json!(rows))
    }
    fn probe_set_style(
        &mut self,
        h: u32,
        mode: u16,
        font: i16,
        face: u8,
        size: i16,
    ) -> Result<(), String> {
        crate::styledtext::Style::new(font, face, size, [0; 3], &self.fonts)?
            .write_attributes(&mut self.m, 0x110400);
        self.api(
            0xa83d,
            1,
            &[(h, 4), (0, 1), (0x110400, 4), (mode as u32, 2)],
        )?;
        Ok(())
    }
}
impl StyledProbe {
    fn continuous_raw(&mut self, h: u32) -> Result<serde_json::Value, String> {
        let p = self.m.read_long(h);
        let c = crate::styledtext::Context::read(&mut self.m, p)?;
        let styles: Vec<_> = c
            .styles
            .iter()
            .map(|s| serde_json::json!([s.font, s.face, s.size]))
            .collect();
        let scrap = self.m.read_long(c.scrap);
        let mut null_row = vec![serde_json::json!(self.m.read_word(scrap))];
        if let Some(s) = c.pending(&mut self.m)? {
            null_row.extend([
                serde_json::json!(s.font),
                serde_json::json!(s.face),
                serde_json::json!(s.size),
            ]);
        }
        Ok(serde_json::json!({"runs":c.runs,"styles":styles,"null":null_row}))
    }
}
pub fn tecontinuous_probe(control: bool) -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    s.port_style(4, 0, 9)?;
    s.probe_rect(300);
    let h = s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)])?;
    let mut rows = serde_json::Map::new();
    rows.insert("initial".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("initial_raw".into(), s.continuous_raw(h)?);
    }
    rows.insert("height_empty".into(), s.height_ranges(h)?);
    s.probe_set_style(h, 15, 3, 1, 12)?;
    rows.insert("pending".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("pending_raw".into(), s.continuous_raw(h)?);
    }
    s.probe_text(h, b"abcd")?;
    s.direct_select(h, 1, 3);
    s.probe_set_style(h, 7, 3, 1, 12)?;
    s.direct_select(h, 0, 4);
    rows.insert("mixed".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("mixed_raw".into(), s.continuous_raw(h)?);
    }
    rows.insert("height_mixed".into(), s.height_ranges(h)?);
    s.probe_set_style(h, 15, 0, 1, 12)?;
    s.direct_select(h, 1, 3);
    s.probe_set_style(h, 2, 0, 3, 12)?;
    s.direct_select(h, 0, 4);
    rows.insert("shared_face".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("shared_face_raw".into(), s.continuous_raw(h)?);
    }
    s.direct_select(h, 1, 1);
    rows.insert("boundary_caret".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("boundary_caret_raw".into(), s.continuous_raw(h)?);
    }
    s.direct_select(h, 1, 3);
    s.probe_set_style(h, 2, 0, 2, 12)?;
    s.direct_select(h, 0, 4);
    rows.insert("no_common_face".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("no_common_face_raw".into(), s.continuous_raw(h)?);
    }
    s.probe_set_style(h, 2, 0, 0, 12)?;
    rows.insert("plain".into(), s.continuous_queries(h)?);
    if control {
        rows.insert("plain_raw".into(), s.continuous_raw(h)?);
    }
    s.probe_text(h, b"A\r")?;
    rows.insert("height_cr".into(), s.height_ranges(h)?);
    s.probe_text(h, b"A\rB")?;
    rows.insert("height_two".into(), s.height_ranges(h)?);
    s.api(0xa9cd, 0, &[(h, 4)])?;
    Ok(serde_json::json!({"schema":1,"probe":if control {"te-continuous-control-v1"} else {"te-continuous-v1"},"textedit":rows}).to_string())
}
pub fn tearrows_probe() -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    s.port_style(4, 0, 9)?;
    s.probe_rect(200);
    let mut rows = serde_json::Map::new();
    for styled in [false, true] {
        let h = s.api(
            if styled { 0xa83e } else { 0xa9d2 },
            0,
            &[(0x110000, 4), (0x110000, 4)],
        )?;
        let mut data = serde_json::Map::new();
        fn state(s: &mut StyledProbe, h: u32) -> Result<serde_json::Value, String> {
            let p = s.m.read_long(h);
            let end = s.m.read_word(p + 34);
            let point = s.api(0xa83d, 8, &[(h, 4), (end as u32, 2)])?;
            Ok(serde_json::json!([
                s.m.read_word(p + 32) as i16,
                end as i16,
                s.m.read_word(p + 94) as i16,
                s.m.read_word(p + 80) as i16,
                s.m.read_word(p + 70) as i16,
                (point >> 16) as u16 as i16,
                point as u16 as i16
            ]))
        }
        for (name, key) in [("empty_down", 31), ("empty_up", 30)] {
            s.api(0xa9dc, 0, &[(h, 4), (key, 2)])?;
            data.insert(name.into(), state(&mut s, h)?);
        }
        s.probe_text(h, b"abcd\rxy\rz")?;
        s.api(0xa9d1, 0, &[(h, 4), (2, 4), (2, 4)])?;
        let mut moves = Vec::new();
        for key in [31, 31, 30, 30, 30, 31] {
            s.api(0xa9dc, 0, &[(h, 4), (key, 2)])?;
            moves.push(state(&mut s, h)?);
        }
        data.insert("moves".into(), serde_json::json!(moves));
        for (name, key) in [("selection_up", 30), ("selection_down", 31)] {
            s.api(0xa9d1, 0, &[(h, 4), (6, 4), (1, 4)])?;
            s.api(0xa9dc, 0, &[(h, 4), (key, 2)])?;
            data.insert(name.into(), state(&mut s, h)?);
        }
        s.api(0xa9cd, 0, &[(h, 4)])?;
        rows.insert(
            if styled { "styled" } else { "plain" }.into(),
            serde_json::Value::Object(data),
        );
    }
    Ok(serde_json::json!({"schema":1,"probe":"te-arrows-v1","textedit":rows}).to_string())
}

impl StyledProbe {
    fn outline_window(
        &mut self,
        bounds: crate::managers::Rect,
        title: &[u8],
    ) -> Result<u32, String> {
        let w = self.windows.create(
            &mut self.m,
            &mut self.heap,
            &self.q,
            &self.fonts,
            &mut self.events,
            0,
            bounds,
            title,
            true,
            0,
            u32::MAX,
            true,
            0,
        )?;
        self.q.make_color(&mut self.m, &mut self.heap, w)?;
        Ok(w)
    }
    fn outline_pixels(&mut self) -> Result<Vec<bool>, String> {
        let port = self.q.get_port(&mut self.m)?;
        let bitmap = self.q.port_bitmap(&mut self.m, port)?;
        let bounds = crate::quickdraw::read_rect(&mut self.m, bitmap + 6);
        let base = self.m.read_long(bitmap);
        let stride = self.m.read_word(bitmap + 4) & 0x3fff;
        let mut bits = Vec::new();
        for y in 28..44 {
            for x in 20..50 {
                let xx = (x - bounds.left) as u32;
                let yy = (y - bounds.top) as u32;
                bits.push(
                    self.m.read_byte(base + yy * stride as u32 + xx / 8) & (0x80 >> (xx % 8)) != 0,
                );
            }
        }
        Ok(bits)
    }
    fn outline_capture(&mut self, baseline: &[bool]) -> Result<serde_json::Value, String> {
        let pixels = self.outline_pixels()?;
        let mut rows = Vec::new();
        let mut count = 0;
        let mut bounds = [32767, 32767, -32768, -32768];
        for y in 0..16 {
            let mut row = String::new();
            for x in 0..30 {
                let changed = pixels[y * 30 + x] != baseline[y * 30 + x];
                row.push(if changed { '1' } else { '0' });
                if changed {
                    count += 1;
                    let xx = x as i32 + 20;
                    let yy = y as i32 + 28;
                    bounds[0] = bounds[0].min(yy);
                    bounds[1] = bounds[1].min(xx);
                    bounds[2] = bounds[2].max(yy + 1);
                    bounds[3] = bounds[3].max(xx + 1);
                }
            }
            rows.push(row);
        }
        Ok(serde_json::json!({"rows":rows,"count":count,"bounds":bounds}))
    }
    fn outline_refresh(&mut self, h: u32) -> Result<(), String> {
        let rect = crate::quickdraw::read_rect(&mut self.m, 0x110000);
        self.q.paint_rect(&mut self.m, rect, [0; 8], 8)?;
        self.api(0xa9d3, 0, &[(h, 4), (0x110000, 4)])?;
        Ok(())
    }
    fn outline_flow(&mut self, styled: bool) -> Result<serde_json::Value, String> {
        use crate::managers::Rect;
        let old = self.q.get_port(&mut self.m)?;
        let w = self.outline_window(
            Rect {
                top: 60,
                left: 50,
                bottom: 260,
                right: 400,
            },
            b"Selection",
        )?;
        self.q.set_port(&mut self.m, w)?;
        self.port_style(4, 0, 9)?;
        crate::quickdraw::write_rect(
            &mut self.m,
            0x110000,
            Rect {
                top: 30,
                left: 20,
                bottom: 100,
                right: 200,
            },
        );
        let h = self.api(
            if styled { 0xa83e } else { 0xa9d2 },
            0,
            &[(0x110000, 4), (0x110000, 4)],
        )?;
        self.probe_text(h, b"A  D")?;
        self.api(0xa83d, 14, &[(h, 4), (0, 2), (2, 2)])?;
        self.api(0xa9d9, 0, &[(h, 4)])?;
        self.outline_refresh(h)?;
        let baseline = self.outline_pixels()?;
        self.api(0xa83d, 14, &[(h, 4), (1, 2), (2, 2)])?;
        self.api(0xa9d8, 0, &[(h, 4)])?;
        self.api(0xa9d1, 0, &[(h, 4), (3, 4), (1, 4)])?;
        self.outline_refresh(h)?;
        let active = self.outline_capture(&baseline)?;
        self.api(0xa9d9, 0, &[(h, 4)])?;
        self.outline_refresh(h)?;
        let inactive = self.outline_capture(&baseline)?;
        let other = self.outline_window(
            Rect {
                top: 400,
                left: 600,
                bottom: 550,
                right: 900,
            },
            b"Other",
        )?;
        self.q.set_port(&mut self.m, w)?;
        self.api(0xa9d8, 0, &[(h, 4)])?;
        self.api(0xa9d1, 0, &[(h, 4), (3, 4), (1, 4)])?;
        self.api(0xa9d9, 0, &[(h, 4)])?;
        self.outline_refresh(h)?;
        let background = self.outline_capture(&baseline)?;
        let feature =
            (self.api(0xa83d, 14, &[(h, 4), (u16::MAX as u32, 2), (2, 2)])? >> 16) as u16 as i16;
        let p = self.m.read_long(h);
        let rect = crate::quickdraw::read_rect(&mut self.m, p + 16);
        let result = serde_json::json!({"active":active,"inactive":inactive,"background":background,"feature":feature,"selection":[self.m.read_word(p+32),self.m.read_word(p+34)],"sel_rect":[rect.top,rect.left,rect.bottom,rect.right]});
        self.api(0xa9cd, 0, &[(h, 4)])?;
        for window in [other, w] {
            self.windows.dispose(
                &mut self.m,
                &mut self.heap,
                &self.q,
                &mut self.events,
                window,
            )?;
        }
        self.q.set_port(&mut self.m, old)?;
        Ok(result)
    }
}
pub fn teoutline_probe() -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    let plain = s.outline_flow(false)?;
    let styled = s.outline_flow(true)?;
    Ok(serde_json::json!({"schema":1,"probe":"te-outline-refresh-v1","textedit":{"plain":plain,"styled":styled}}).to_string())
}

pub fn resourceenum_probe() -> Result<String, String> {
    let bytes = std::fs::read("work/probes/resourceenum/ResourceEnumProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let raw = resources.first().ok_or("missing resource fixture")?.map;
    let map = heap
        .new_handle(&mut m, raw.len() as u32, false)
        .map_err(|e| e.to_string())?;
    let p = heap.data(map).map_err(|e| e.to_string())?;
    m.put(p as usize, raw)?;
    m.write_long(0xa50, map);
    m.write_word(0xa5a, 2);
    let mut cache = std::collections::BTreeMap::new();
    fn api(
        op: u16,
        args: &[(u32, u32)],
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
        resources: &[Resource<'_>],
        cache: &mut std::collections::BTreeMap<([u8; 4], i16), u32>,
    ) -> Result<u32, String> {
        let sp = 0x7e0000;
        let mut offset = 0;
        for (v, size) in args {
            match size {
                2 => m.write_word(sp + offset, *v as u16),
                4 => m.write_long(sp + offset, *v),
                _ => return Err("invalid resource probe field".into()),
            }
            offset += size;
        }
        m.write_long(sp + offset, 0xa5a5a5a5);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        if !resource_enumeration(op, &mut cpu, m, heap, resources, cache)?
            || cpu.pc != 0x19002
            || cpu.dar[15] != sp + offset
        {
            return Err("resource probe Pascal ABI mismatch".into());
        }
        Ok(m.read_long(sp + offset))
    }
    let kind = u32::from_be_bytes(*b"pTst");
    let mut data = serde_json::Map::new();
    for (name, typ) in [("count", kind), ("absent", u32::from_be_bytes(*b"pNil"))] {
        let n = api(
            0xa80d,
            &[(typ, 4)],
            &mut m,
            &mut heap,
            &resources,
            &mut cache,
        )? >> 16;
        let error = m.read_word(0xa60) as i16;
        data.insert(name.into(), serde_json::json!([n, error]));
    }
    let mut rows = Vec::new();
    for index in [0i16, 1, 2, 3, 4, -1] {
        let h = api(
            0xa80e,
            &[(index as u16 as u32, 2), (kind, 4)],
            &mut m,
            &mut heap,
            &resources,
            &mut cache,
        )?;
        let error = m.read_word(0xa60) as i16;
        let mut row = serde_json::json!({"index":index,"nonnull":(h!=0) as u8,"error":error});
        if h != 0 {
            api(
                0xa9a8,
                &[(0x110000, 4), (0x110100, 4), (0x110104, 4), (h, 4)],
                &mut m,
                &mut heap,
                &resources,
                &mut cache,
            )?;
            let info = m.read_word(0xa60) as i16;
            let id = m.read_word(0x110104) as i16;
            let typ = m.read_long(0x110100);
            let name: Vec<_> = (0..m.read_byte(0x110000) as u32)
                .map(|i| m.read_byte(0x110001 + i))
                .collect();
            let again = api(
                0xa80e,
                &[(index as u16 as u32, 2), (kind, 4)],
                &mut m,
                &mut heap,
                &resources,
                &mut cache,
            )?;
            let home = api(0xa9a4, &[(h, 4)], &mut m, &mut heap, &resources, &mut cache)? >> 16;
            let size = heap.size(h).map_err(|e| e.to_string())?;
            let p = heap.data(h).map_err(|e| e.to_string())?;
            let bytes: Vec<_> = (0..size).map(|i| m.read_byte(p + i)).collect();
            let obj = row.as_object_mut().unwrap();
            for (k, v) in [
                ("id", serde_json::json!(id)),
                ("type", serde_json::json!(typ)),
                ("name", serde_json::json!(name)),
                ("info_error", serde_json::json!(info)),
                ("same_handle", serde_json::json!((h == again) as u8)),
                ("home_is_current", serde_json::json!((home == 2) as u8)),
                ("bytes", serde_json::json!(bytes)),
            ] {
                obj.insert(k.into(), v);
            }
        }
        rows.push(row);
    }
    data.insert("items".into(), serde_json::json!(rows));
    m.write_word(0xa5a, 0);
    let n = api(
        0xa80d,
        &[(kind, 4)],
        &mut m,
        &mut heap,
        &resources,
        &mut cache,
    )? >> 16;
    let error = m.read_word(0xa60) as i16;
    data.insert("system_count".into(), serde_json::json!([n, error]));
    Ok(serde_json::json!({"schema":1,"probe":"resource-enum-v1","resources":data}).to_string())
}

pub fn stringcompare_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 0x10000],
    };
    let mut cases = Vec::new();
    for (a, b) in [
        (b"".as_slice(), b"".as_slice()),
        (b"Geneva", b"Geneva"),
        (b"Geneva", b"GENEVA"),
        (b"Geneva", b"Courier"),
        (&[0, b'a', b'B', 127], &[0, b'A', b'b', 127]),
        (b"abc", b"abcd"),
    ] {
        m.put(0x1000, a)?;
        m.put(0x2000, b)?;
        let mut row = Vec::new();
        for opcode in [0xa03c, 0xa23c, 0xa43c, 0xa63c] {
            let mut cpu = CpuCore::new();
            cpu.pc = 0x8000;
            cpu.dar[0] = ((a.len() as u32) << 16) | b.len() as u32;
            cpu.dar[8] = 0x1000;
            cpu.dar[9] = 0x2000;
            cpu.dar[15] = 0xf000;
            if !crate::stringutils::dispatch(opcode, &mut cpu, &mut m)?
                || cpu.pc != 0x8002
                || cpu.dar[15] != 0xf000
            {
                return Err("string comparison register ABI mismatch".into());
            }
            row.push(serde_json::json!([
                cpu.dar[0],
                cpu.flag_z() as u8,
                cpu.flag_n() as u8
            ]));
        }
        cases.push(row);
    }
    Ok(
        serde_json::json!({"schema":1,"probe":"string-compare-v1","strings":{"cases":cases}})
            .to_string(),
    )
}

pub fn fontavailability_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 0x10000],
    };
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut rows = Vec::new();
    for (family, size) in [
        (0i16, 12i16),
        (1, 12),
        (3, 9),
        (3, 12),
        (4, 9),
        (4, 12),
        (2002, 12),
        (32767, 12),
    ] {
        let mut cpu = CpuCore::new();
        cpu.pc = 0x8000;
        cpu.dar[15] = 0xf000;
        m.write_word(0xf000, size as u16);
        m.write_word(0xf002, family as u16);
        m.write_word(0xf004, 0xa5a5);
        if !fonts.dispatch(0xa902, &mut cpu, &mut m) || cpu.pc != 0x8002 || cpu.dar[15] != 0xf004 {
            return Err("RealFont Pascal ABI mismatch".into());
        }
        rows.push(serde_json::json!([family, size, m.read_byte(0xf004)]));
    }
    Ok(
        serde_json::json!({"schema":1,"probe":"font-availability-v1","fonts":{"queries":rows}})
            .to_string(),
    )
}

pub fn scriptcontext_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 0x10000],
    };
    let scripts = crate::scripts::Scripts::roman();
    let mut context = Vec::new();
    for selector in [18, 20, 22] {
        let mut cpu = CpuCore::new();
        cpu.pc = 0x8000;
        cpu.dar[15] = 0xf000;
        m.write_long(0xf000, 0x84020008);
        m.write_word(0xf004, selector);
        m.write_long(0xf006, 0xa5a5a5a5);
        if !scripts.dispatch(0xa8b5, &mut cpu, &mut m)? || cpu.pc != 0x8002 || cpu.dar[15] != 0xf006
        {
            return Err("script context Pascal ABI mismatch".into());
        }
        context.push(serde_json::json!([selector, m.read_long(0xf006) as i32]));
    }
    let mut properties = Vec::new();
    for script in [0i16, -1] {
        for variable in [6, 8, 90] {
            let mut cpu = CpuCore::new();
            cpu.pc = 0x8000;
            cpu.dar[15] = 0xf000;
            m.write_long(0xf000, 0x8404000c);
            m.write_word(0xf004, variable);
            m.write_word(0xf006, script as u16);
            m.write_long(0xf008, 0xa5a5a5a5);
            if !scripts.dispatch(0xa8b5, &mut cpu, &mut m)?
                || cpu.pc != 0x8002
                || cpu.dar[15] != 0xf008
            {
                return Err("script property Pascal ABI mismatch".into());
            }
            properties.push(serde_json::json!([
                script,
                variable,
                m.read_long(0xf008) as i32
            ]));
        }
    }
    Ok(serde_json::json!({"schema":1,"probe":"script-context-v1","scripts":{"context":context,"properties":properties}}).to_string())
}

pub fn fontglobals_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 0x10000],
    };
    let mut fonts = crate::fonts::Fonts::new();
    fonts.initialize(&mut m)?;
    let globals: Vec<_> = [0x984, 0xba6, 0xba8]
        .iter()
        .map(|p| m.read_word(*p) as i16)
        .collect();
    Ok(
        serde_json::json!({"schema":1,"probe":"font-globals-v1","fonts":{"globals":globals}})
            .to_string(),
    )
}

pub fn modalfilter_probe() -> Result<String, String> {
    let bytes = std::fs::read("work/probes/modalfilter/ModalFilterProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let hook =
        std::fs::read("work/probes/modalfilter/runtime-hook.bin").map_err(|e| e.to_string())?;
    let filter =
        std::fs::read("work/probes/modalfilter/runtime-filter.bin").map_err(|e| e.to_string())?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::standardfile::modal_filter_probe(&mut m, &resources, &hook, &filter)
}

pub fn handlestate_probe() -> Result<String, String> {
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut cpu = CpuCore::new();
    let mut rows = Vec::new();
    for mode in 0..2 {
        for flags in [0u8, 32, 64, 96, 128, 160, 192, 224, 255, 31] {
            let h = heap
                .new_handle(&mut m, 16, false)
                .map_err(|e| e.to_string())?;
            if mode == 1 {
                heap.resource_state(h, false, false)
                    .map_err(|e| e.to_string())?;
            }
            cpu.pc = 0x19000;
            cpu.dar[8] = h;
            cpu.dar[0] = 0x12345600 | flags as u32;
            handle_state_trap(0xa06a, &mut cpu, &mut m, &mut heap);
            if cpu.pc != 0x19002 {
                return Err("HSetState return PC mismatch".into());
            }
            let result = cpu.dar[0] as i32;
            let error = m.read_word(0x220) as i16;
            let z = cpu.flag_z() as u8;
            let n = cpu.flag_n() as u8;
            handle_state_trap(0xa069, &mut cpu, &mut m, &mut heap);
            rows.push(json!([
                mode,
                flags,
                result,
                error,
                cpu.dar[0] as u8,
                m.read_word(0x220) as i16,
                z,
                n
            ]));
            heap.set_state(h, 0).map_err(|e| e.to_string())?;
            heap.dispose(&mut m, h).map_err(|e| e.to_string())?;
        }
    }
    cpu.dar[8] = 0;
    cpu.dar[0] = 0x12345680;
    handle_state_trap(0xa06a, &mut cpu, &mut m, &mut heap);
    let nil_set = json!([
        cpu.dar[0] as i32,
        m.read_word(0x220) as i16,
        cpu.flag_z() as u8,
        cpu.flag_n() as u8
    ]);
    handle_state_trap(0xa069, &mut cpu, &mut m, &mut heap);
    let nil_get = json!([cpu.dar[0] as u8, m.read_word(0x220) as i16]);
    let mut purge = Vec::new();
    for mode in 0..2 {
        for i in 0..=8 {
            let h = if i < 8 {
                heap.new_handle(&mut m, 16, false)
                    .map_err(|e| e.to_string())?
            } else {
                0
            };
            if h != 0 {
                heap.set_state(h, (i * 32) as u8)
                    .map_err(|e| e.to_string())?;
            }
            cpu.dar[8] = h;
            cpu.dar[0] = 0x12345678;
            handle_state_trap(
                if mode == 1 { 0xa049 } else { 0xa04a },
                &mut cpu,
                &mut m,
                &mut heap,
            );
            let result = cpu.dar[0] as i32;
            let error = m.read_word(0x220) as i16;
            let z = cpu.flag_z() as u8;
            let n = cpu.flag_n() as u8;
            handle_state_trap(0xa069, &mut cpu, &mut m, &mut heap);
            purge.push(json!([
                mode,
                i * 32,
                result,
                error,
                cpu.dar[0] as u8,
                m.read_word(0x220) as i16,
                z,
                n
            ]));
            if h != 0 {
                heap.set_state(h, 0).map_err(|e| e.to_string())?;
                heap.dispose(&mut m, h).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(json!({"schema":1,"probe":"handle-state-v1","memory":{"states":rows,"nil_set":nil_set,"nil_get":nil_get,"purge":purge}}).to_string())
}

pub fn doclifecycle_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::standardfile::doc_lifecycle_probe(&mut m, &[])
}

pub fn standardfilter_probe() -> Result<String, String> {
    let bytes = std::fs::read("work/probes/modalfilter/StandardFilterProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::standardfile::standard_filter_probe(&mut m, &resources)
}

/// Independent deterministic-clock execution of the same Delay service.
pub fn delay_probe() -> Result<String, String> {
    let mut rows = Vec::new();
    let mut timing = crate::timing::Timing::default();
    let mut cpu = CpuCore::new();
    let mut now = 100u32;
    for request in 0..3u32 {
        cpu.pc = 0x18000;
        cpu.dar[8] = request;
        cpu.dar[0] = 0x5a5a5a5a;
        let before = now;
        while timing.dispatch(0xa03b, &mut cpu, now)? != Some(true) {
            now = now.wrapping_add(1);
        }
        rows.push(vec![
            request,
            cpu.dar[8],
            u32::from(now.wrapping_sub(before) >= request),
            u32::from(cpu.dar[0].wrapping_sub(before) >= request),
            u32::from(now.wrapping_sub(cpu.dar[0]) < 0x80000000),
        ]);
    }
    Ok(serde_json::json!({"schema":1,"probe":"delay-v1","timing":{"cases":rows}}).to_string())
}

/// Native user-items-v1 scenario, using real Pascal procedure CPU execution.
/// The callback fixture mutates public port fields; callback entry is observed
/// independently. Native PaintRect pixels are outside this state-only comparison.
pub fn useritems_probe() -> Result<String, String> {
    let bytes = std::fs::read("work/probes/modalfilter/UserItemsProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let code =
        std::fs::read("work/probes/modalfilter/runtime-useritem.bin").map_err(|e| e.to_string())?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let mut controls = crate::controls::Controls::new();
    let mut dialogs = crate::dialogs::Dialogs::new();
    let d = dialogs.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut windows,
        &mut controls,
        &mut events,
        &resources,
        2001,
        0,
    )?;
    q.set_port(&mut m, d)?;
    q.set_word(&mut m, 68, 3)?;
    q.set_word(&mut m, 74, 12)?;
    q.pen_size(&mut m, 1, 2)?;
    q.set_word(&mut m, 72, 2)?;
    q.set_word(&mut m, 56, 10)?;
    let caller = 0x110000;
    q.open_port(&mut m, &mut heap, caller)?;
    q.set_port(&mut m, caller)?;
    q.set_word(&mut m, 68, 4)?;
    q.set_word(&mut m, 74, 18)?;
    q.pen_size(&mut m, 5, 6)?;
    let target = 0x19100;
    m.put(target as usize, &code)?;
    let mut cpu = CpuCore::new();
    cpu.set_cpu_type(crate::environment::CPU_TYPE);
    let sp = 0x7e0000;
    let mut callbacks = crate::callbacks::Callbacks::silent();
    let mut rows = Vec::new();
    for pass in 0..2 {
        for item in 1..=2u16 {
            let rec = &dialogs.records[&d].items[item as usize - 1];
            let rect = rec.rect;
            let kind = rec.kind;
            crate::quickdraw::write_rect(&mut m, 0x110200, rect);
            m.write_long(sp, 0x110200);
            m.write_long(sp + 4, if pass == 1 && item == 2 { 0 } else { target });
            m.write_word(sp + 8, kind as u16);
            m.write_word(sp + 10, item);
            m.write_long(sp + 12, d);
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            if !dialogs.dispatch(0xa98e, &mut cpu, &mut m, &mut heap)? || cpu.dar[15] != sp + 16 {
                return Err("SetDialogItem procedure ABI mismatch".into());
            }
        }
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        m.write_long(sp, d);
        if !dialogs.dispatch_draw(0xa981, &mut cpu, &mut m, &mut heap, &q, &fonts, &controls)? {
            return Err("DrawDialog not dispatched".into());
        }
        let mut finished = false;
        for _ in 0..1000 {
            if let Some((token, _)) = callbacks.finish(&mut cpu, &mut m)? {
                if token != 4 {
                    return Err("unexpected drawing owner".into());
                }
                dialogs.finish_draw(&mut m, &q)?;
                continue;
            }
            if dialogs.service_draw(&mut cpu, &mut m, &q, &mut callbacks)? {
                continue;
            }
            if cpu.pc == 0x19002 && !dialogs.has_pending_draws() {
                finished = true;
                break;
            }
            if cpu.pc == target {
                let port = q.get_port(&mut m)?;
                let arg = m.read_long(cpu.dar[15] + 6);
                rows.push(vec![
                    m.read_word(cpu.dar[15] + 4) as i32,
                    i32::from(arg == d),
                    i32::from(port == d),
                    m.read_word(port + 68) as i16 as i32,
                    m.read_word(port + 74) as i16 as i32,
                    m.read_word(port + 54) as i16 as i32,
                    m.read_word(port + 52) as i16 as i32,
                    m.read_word(port + 56) as i16 as i32,
                    m.read_word(port + 72) as i16 as i32,
                ]);
            }
            if !matches!(cpu.step(&mut m), StepResult::Ok { .. }) {
                return Err("user-item procedure CPU stopped".into());
            }
        }
        if !finished || cpu.dar[15] != sp + 4 {
            return Err("DrawDialog procedure did not return correctly".into());
        }
    }
    let after = q.get_port(&mut m)?;
    let caller_state = vec![
        i32::from(after == caller),
        m.read_word(after + 68) as i32,
        m.read_word(after + 74) as i32,
        m.read_word(after + 54) as i32,
        m.read_word(after + 52) as i32,
    ];
    let dialog_state: Vec<_> = [68, 74, 54, 52, 56, 72]
        .iter()
        .map(|o| m.read_word(d + o) as i32)
        .collect();
    dialogs.dispose(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut windows,
        &mut controls,
        &mut events,
        d,
    )?;
    Ok(serde_json::json!({"schema":1,"probe":"user-items-v1","dialogs":{"calls":rows,"caller":caller_state,"dialog":dialog_state}}).to_string())
}

pub fn iconplot_probe() -> Result<String, String> {
    let bytes = std::fs::read("work/probes/modalfilter/IconPlotProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let port = 0x110400;
    q.open_port(&mut m, &mut heap, port)?;
    q.set_port(&mut m, port)?;
    let bounds = crate::managers::Rect {
        top: 0,
        left: 0,
        bottom: 64,
        right: 64,
    };
    m.write_long(port + 2, 0x110000);
    m.write_word(port + 6, 8);
    crate::quickdraw::write_rect(&mut m, port + 8, bounds);
    crate::quickdraw::write_rect(&mut m, port + 16, bounds);
    for offset in [24, 28] {
        let handle = m.read_long(port + offset);
        crate::regions::Region::rectangle(bounds).store(&mut m, &mut heap, handle)?;
    }
    for i in 0..512 {
        m.write_byte(0x110000 + i, 0xaa);
    }
    let mut cpu = CpuCore::new();
    let mut errors = Vec::new();
    let sp = 0x7e0000;
    for (rect, id) in [
        (
            crate::managers::Rect {
                top: 0,
                left: 0,
                bottom: 16,
                right: 16,
            },
            2001,
        ),
        (
            crate::managers::Rect {
                top: 20,
                left: 20,
                bottom: 52,
                right: 52,
            },
            2001,
        ),
        (
            crate::managers::Rect {
                top: 0,
                left: 0,
                bottom: 16,
                right: 16,
            },
            3001,
        ),
    ] {
        crate::quickdraw::write_rect(&mut m, 0x110600, rect);
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = 0x500;
        m.write_word(sp, id);
        m.write_word(sp + 2, 0);
        m.write_word(sp + 4, 0);
        m.write_long(sp + 6, 0x110600);
        m.write_word(sp + 10, 0x1234);
        if !crate::icons::dispatch(0xabc9, &mut cpu, &mut m, &q, &resources)?
            || cpu.pc != 0x19002
            || cpu.dar[15] != sp + 10
        {
            return Err("PlotIconID Pascal ABI mismatch".into());
        }
        errors.push(m.read_word(sp + 10) as i16);
    }
    let pixels: String = (0..512)
        .map(|i| format!("{:02x}", m.read_byte(0x110000 + i)))
        .collect();
    Ok(serde_json::json!({"schema":1,"probe":"icon-plot-v1","icons":{"errors":errors,"bitmap_hex":pixels}}).to_string())
}

pub fn rescreate_probe() -> Result<String, String> {
    use crate::files::{Files, Fork};
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut files = Files::new();
    m.write_word(0xa5a, 2);
    fn spec(m: &mut Memory, name: &[u8]) {
        m.write_word(0x110000, 0);
        m.write_long(0x110002, 2);
        m.write_byte(0x110006, name.len() as u8);
        m.put(0x110007, name).unwrap();
    }
    fn create(files: &mut Files, m: &mut Memory, creator: u32, kind: u32) -> Result<i16, String> {
        let sp = 0x7e0000;
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = 14;
        m.write_word(sp, 0xffff);
        m.write_long(sp + 2, kind);
        m.write_long(sp + 6, creator);
        m.write_long(sp + 10, 0x110000);
        m.write_word(sp + 14, 0xcafe);
        if !files.dispatch(0xaa52, &mut cpu, m)?
            || cpu.pc != 0x19002
            || cpu.dar[15] != sp + 14
            || m.read_word(sp + 14) != 0xcafe
        {
            return Err("FSpCreateResFile void Pascal ABI mismatch".into());
        }
        Ok(m.read_word(0xa60) as i16)
    }
    fn put(files: &mut Files, name: &[u8], fork: Fork, data: &[u8]) -> Result<(), String> {
        let r = files.open(name, fork, 3).map_err(|e| e.to_string())?;
        files.write(r, data).map_err(|e| e.to_string())?;
        files.close(r).map_err(|e| e.to_string())
    }
    fn raw(files: &mut Files, name: &[u8], fork: Fork) -> Result<(i16, Vec<u8>), String> {
        let r = match files.open(name, fork, 1) {
            Ok(r) => r,
            Err(e) => return Ok((e, Vec::new())),
        };
        let n = files.eof(r).map_err(|e| e.to_string())?;
        let (bytes, error) = files.read(r, n).map_err(|e| e.to_string())?;
        files.close(r).map_err(|e| e.to_string())?;
        Ok((error, bytes))
    }
    fn snapshot(
        files: &mut Files,
        m: &mut Memory,
        name: &[u8],
        error: i16,
    ) -> Result<serde_json::Value, String> {
        let (de, data) = raw(files, name, Fork::Data)?;
        let (re, resource) = raw(files, name, Fork::Resource)?;
        let info = files.finder(name).map_err(|e| e.to_string())?;
        let hex = |bytes: Vec<u8>| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        Ok(
            serde_json::json!({"error":error,"current_same":u8::from(m.read_word(0xa5a)==2),"fork_errors":[de,re],"finder":[u32::from_be_bytes(info[..4].try_into().unwrap()),u32::from_be_bytes(info[4..8].try_into().unwrap())],"data_hex":hex(data),"resource_hex":hex(resource)}),
        )
    }
    let creator = u32::from_be_bytes(*b"CrPr");
    let kind = u32::from_be_bytes(*b"TEXT");
    let mut rows = Vec::new();
    spec(&mut m, b"ResourceFresh");
    let error = create(&mut files, &mut m, creator, kind)?;
    rows.push(snapshot(&mut files, &mut m, b"ResourceFresh", error)?);
    put(&mut files, b"ResourceFresh", Fork::Data, b"keep")?;
    let error = create(
        &mut files,
        &mut m,
        u32::from_be_bytes(*b"New!"),
        u32::from_be_bytes(*b"BINA"),
    )?;
    rows.push(snapshot(&mut files, &mut m, b"ResourceFresh", error)?);
    let mut finder = [0u8; 16];
    finder[..8].copy_from_slice(b"BINAOld!");
    files
        .create(b"ResourcePlain", finder)
        .map_err(|e| e.to_string())?;
    put(&mut files, b"ResourcePlain", Fork::Data, b"keep")?;
    spec(&mut m, b"ResourcePlain");
    let error = create(&mut files, &mut m, creator, kind)?;
    rows.push(snapshot(&mut files, &mut m, b"ResourcePlain", error)?);
    files
        .create(b"ResourceCorrupt", finder)
        .map_err(|e| e.to_string())?;
    put(&mut files, b"ResourceCorrupt", Fork::Resource, b"bad!")?;
    spec(&mut m, b"ResourceCorrupt");
    let error = create(&mut files, &mut m, creator, kind)?;
    rows.push(snapshot(&mut files, &mut m, b"ResourceCorrupt", error)?);
    spec(&mut m, b"ResourceFresh");
    m.write_word(0x110000, 32000);
    let invalid_volume = create(&mut files, &mut m, creator, kind)?;
    m.write_word(0x110000, 0);
    m.write_long(0x110002, 0x7fffffff);
    let invalid_dir = create(&mut files, &mut m, creator, kind)?;
    Ok(serde_json::json!({"schema":1,"probe":"res-create-v1","resources":{"cases":rows,"invalid":[invalid_volume,invalid_dir]}}).to_string())
}

struct ResIOState {
    m: Memory,
    heap: crate::managers::Heap,
    files: crate::files::Files,
    manager: crate::resfiles::ResFiles,
    cache: std::collections::BTreeMap<([u8; 4], i16), u32>,
}
impl ResIOState {
    fn new() -> Result<Self, String> {
        let mut s = Self {
            m: Memory {
                bytes: vec![0; 8 * 1024 * 1024],
            },
            heap: crate::managers::Heap::new(0x20000, 0xe0000),
            files: crate::files::Files::new(),
            manager: crate::resfiles::ResFiles::default(),
            cache: Default::default(),
        };
        let args: Vec<_> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == "--volume") {
            s.files = crate::files::Files::volume(std::path::Path::new(
                args.get(i + 1).ok_or("missing probe volume")?,
            ))?;
        }
        let system = s
            .heap
            .new_handle(&mut s.m, 30, true)
            .map_err(|e| e.to_string())?;
        let app = s
            .heap
            .new_handle(&mut s.m, 30, true)
            .map_err(|e| e.to_string())?;
        for h in [system, app] {
            let p = s.heap.data(h).map_err(|e| e.to_string())?;
            s.m.put(p as usize, &crate::resource::empty_fork()[256..])?;
        }
        let p = s.heap.data(app).map_err(|e| e.to_string())?;
        s.m.write_long(p + 16, system);
        s.m.write_word(p + 20, 2);
        s.m.write_long(0xa50, app);
        s.m.write_word(0xa5a, 2);
        Ok(s)
    }
    fn api(&mut self, op: u16, selector: u16, args: &[(u32, u32)]) -> Result<u32, String> {
        let sp = 0x7e0000;
        let mut offset = 0;
        for (value, size) in args {
            match size {
                1 => {
                    self.m.write_byte(sp + offset, *value as u8);
                    offset += 2
                }
                2 => {
                    self.m.write_word(sp + offset, *value as u16);
                    offset += 2
                }
                4 => {
                    self.m.write_long(sp + offset, *value);
                    offset += 4
                }
                _ => return Err("resource probe argument width".into()),
            }
        }
        self.m.write_long(sp + offset, 0xa5a5a5a5);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = selector as u32;
        let handled = self.manager.dispatch(
            op,
            &mut cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.files,
            &[],
            &mut self.cache,
            load_resource,
        )?;
        if !handled && !self.files.dispatch(op, &mut cpu, &mut self.m)? {
            return Err(format!("resource probe trap not handled {op:#x}"));
        }
        if cpu.pc != 0x19002 || cpu.dar[15] != sp + offset {
            return Err(format!("resource probe Pascal ABI mismatch {op:#x}"));
        }
        Ok(self.m.read_long(sp + offset))
    }
    fn error(&mut self) -> i16 {
        self.m.read_word(0xa60) as i16
    }
    fn current(&mut self) -> i16 {
        self.m.read_word(0xa5a) as i16
    }
    fn spec(&mut self, name: &[u8], address: u32) {
        self.m.write_word(address, 0);
        self.m.write_long(address + 2, 2);
        self.m.write_byte(address + 6, name.len() as u8);
        for (i, b) in name.iter().enumerate() {
            self.m.write_byte(address + 7 + i as u32, *b);
        }
    }
    fn create(&mut self, spec: u32) -> Result<(), String> {
        self.api(
            0xaa52,
            14,
            &[
                (0xffff, 2),
                (u32::from_be_bytes(*b"TEXT"), 4),
                (u32::from_be_bytes(*b"IoPr"), 4),
                (spec, 4),
            ],
        )?;
        if self.error() != 0 {
            return Err("resource probe create failed".into());
        }
        Ok(())
    }
    fn open(&mut self, spec: u32, permission: u32) -> Result<i16, String> {
        Ok((self.api(0xaa52, 13, &[(permission, 1), (spec, 4)])? >> 16) as u16 as i16)
    }
    fn get(&mut self, id: u32, one: bool) -> Result<u32, String> {
        self.api(
            if one { 0xa81f } else { 0xa9a0 },
            0,
            &[(id, 2), (u32::from_be_bytes(*b"TEST"), 4)],
        )
    }
    fn word(&mut self, op: u16, h: u32) -> Result<i16, String> {
        Ok((self.api(op, 0, &[(h, 4)])? >> 16) as u16 as i16)
    }
    fn bytes(&mut self, h: u32) -> Result<Vec<u8>, String> {
        let n = self.heap.size(h).map_err(|e| e.to_string())?;
        let p = self.heap.data(h).map_err(|e| e.to_string())?;
        Ok((0..n).map(|k| self.m.read_byte(p + k)).collect())
    }
}
pub fn apprelease_probe() -> Result<String, String> {
    use serde_json::json;
    fn api(
        s: &mut ResIOState,
        app: &[Resource<'_>],
        op: u16,
        args: &[(u32, u32)],
        flags: u8,
    ) -> Result<(u32, u32, u16, i64), String> {
        let sp = 0x7e0000;
        let mut offset = 0;
        for &(value, width) in args {
            if width == 2 {
                s.m.write_word(sp + offset, value as u16);
            } else {
                s.m.write_long(sp + offset, value);
            }
            offset += width;
        }
        s.m.write_long(sp + offset, 0xa5a5a5a5);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = 0x12345678;
        cpu.set_ccr(flags);
        let handled = s.manager.dispatch(
            op,
            &mut cpu,
            &mut s.m,
            &mut s.heap,
            &mut s.files,
            app,
            &mut s.cache,
            load_resource,
        )?;
        if !handled
            && !resource_enumeration(op, &mut cpu, &mut s.m, &mut s.heap, app, &mut s.cache)?
        {
            return Err(format!(
                "application release probe trap {op:#x} unsupported"
            ));
        }
        assert_eq!(cpu.pc, 0x19002);
        assert_eq!(cpu.dar[15], sp + offset);
        Ok((
            s.m.read_long(sp + offset),
            cpu.dar[0],
            cpu.get_sr() & 31,
            (sp + offset) as i64 - cpu.dar[15] as i64,
        ))
    }
    fn payload(s: &mut ResIOState, h: u32, valid: bool) -> Result<String, String> {
        if !valid {
            return Ok(String::new());
        }
        let bytes = s.bytes(h)?;
        Ok(if bytes.len() == 6 {
            bytes.iter().map(|b| format!("{:02x}", b)).collect()
        } else {
            String::new()
        })
    }
    let fork = crate::resource::serialize(&[crate::resource::Data {
        kind: *b"TEST",
        id: 128,
        attributes: 0,
        name: Some(b"Application payload".to_vec()),
        bytes: b"ABCDEF".to_vec(),
    }])?;
    let resources = crate::resource::parse(&fork)?;
    let mut s = ResIOState::new()?;
    let app = s.current();
    let map_h = s.m.read_long(0xa50);
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    let system = s.m.read_long(map + 16);
    s.heap
        .resize(&mut s.m, map_h, resources[0].map.len() as u32)
        .map_err(|e| e.to_string())?;
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    s.m.put(map as usize, resources[0].map)?;
    s.m.write_long(map + 16, system);
    s.m.write_word(map + 20, 2);
    let entry = map + resources[0].reference_offset as u32 + 8;
    let mut records = Vec::new();
    let get_args = [(128, 2), (u32::from_be_bytes(*b"TEST"), 4)];
    for scenario in 0..4 {
        for input_ccr in [16u8, 31, 0] {
            let resource = api(&mut s, &resources, 0xa81f, &get_args, 0)?.0;
            assert_ne!(resource, 0);
            let h = match scenario {
                1 => 0,
                2 => s
                    .heap
                    .new_handle(&mut s.m, 6, false)
                    .map_err(|e| e.to_string())?,
                _ => resource,
            };
            if h != 0 {
                let p = s.heap.data(h).map_err(|e| e.to_string())?;
                if scenario == 2 {
                    s.m.put(p as usize, b"PLAIN!")?;
                } else {
                    s.m.write_byte(p, b'Z');
                }
            }
            let before_state = if h == 0 {
                -1
            } else {
                s.heap.state(h).map_err(|e| e.to_string())? as i16
            };
            let initial_error = s.error();
            let entry_before = i16::from(s.m.read_long(entry) == resource);
            let (_, d0, ccr, delta) = api(
                &mut s,
                &resources,
                if scenario == 3 { 0xa992 } else { 0xa9a3 },
                &[(h, 4)],
                input_ccr,
            )?;
            let error = s.error();
            let entry_zero = i16::from(s.m.read_long(entry) == 0);
            let after_state = if scenario == 2 || scenario == 3 {
                s.heap.state(h).map_err(|e| e.to_string())? as i16
            } else {
                -1
            };
            let (mut home, mut home_error) = (12345i16, 12345i16);
            if scenario == 3 {
                home = (api(&mut s, &resources, 0xa9a4, &[(h, 4)], 0)?.0 >> 16) as u16 as i16;
                home_error = s.error();
            }
            let again = api(&mut s, &resources, 0xa81f, &get_args, 0)?.0;
            let reload_error = s.error();
            assert_ne!(again, 0);
            let home_same = i16::from(
                (api(&mut s, &resources, 0xa9a4, &[(again, 4)], 0)?.0 >> 16) as u16 as i16 == app,
            );
            let current_same = i16::from(s.current() == app);
            let entry_after = i16::from(s.m.read_long(entry) == again);
            let resource_bit = i16::from(s.heap.state(again).map_err(|e| e.to_string())? & 32 != 0);
            let retained = payload(&mut s, h, scenario == 2 || scenario == 3)?;
            let reloaded = payload(&mut s, again, true)?;
            records.push(json!({"case":scenario,"input_ccr":input_ccr,"initial_error":initial_error,"before_state":before_state,"d0":d0,"ccr":ccr,"stack_delta":delta,"error":error,"after_state":after_state,"home":home,"home_error":home_error,"entry_before":entry_before,"entry_zero_after_call":entry_zero,"entry_matches_reload":entry_after,"reload_error":reload_error,"reload_home_same":home_same,"current_same":current_same,"reload_resource_bit":resource_bit,"retained_payload":retained,"reloaded_payload":reloaded}));
            if scenario == 2 || scenario == 3 {
                s.heap.dispose(&mut s.m, h).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(
        json!({"schema":1,"probe":"app-resource-release-v1","resources":{"records":records}})
            .to_string(),
    )
}
pub fn resio_probe() -> Result<String, String> {
    use serde_json::json;
    let mut s = ResIOState::new()?;
    let app = s.current();
    let a = 0x110000;
    let b = 0x110100;
    let empty = 0x110200;
    let corrupt = 0x110300;
    let missing = 0x110400;
    for (name, spec) in [
        (b"ResourceA".as_slice(), a),
        (b"ResourceB", b),
        (b"ResourceEmpty", empty),
        (b"ResourceCorrupt", corrupt),
        (b"ResourceMissing", missing),
    ] {
        s.spec(name, spec);
    }
    s.create(a)?;
    let ar = s.open(a, 3)?;
    let first_open = json!([i32::from(ar != -1), s.error(), i16::from(s.current() == ar)]);
    let found = s.get(128, true)?;
    let empty_before = json!([i32::from(found != 0), s.error()]);
    let h = s
        .heap
        .new_handle(&mut s.m, 4, false)
        .map_err(|e| e.to_string())?;
    let p = s.heap.data(h).map_err(|e| e.to_string())?;
    s.m.put(p as usize, b"AB\0!")?;
    s.m.put(0x110600, b"\x05Owned")?;
    s.api(
        0xa9ab,
        0,
        &[
            (0x110600, 4),
            (128, 2),
            (u32::from_be_bytes(*b"TEST"), 4),
            (h, 4),
        ],
    )?;
    let add_error = s.error();
    let attr_added = s.word(0xa9a6, h)?;
    let found = s.get(128, true)?;
    let identity = i16::from(found == h);
    let home = i16::from(s.word(0xa9a4, h)? == ar);
    let found = s.get(129, true)?;
    let missing_id = json!([i32::from(found != 0), s.error()]);
    let plain = s
        .heap
        .new_handle(&mut s.m, 1, false)
        .map_err(|e| e.to_string())?;
    s.word(0xa9a4, plain)?;
    let before = s.error();
    let found = s.get(129, true)?;
    let missing_primed = json!([before, i32::from(found != 0), s.error()]);
    s.word(0xa9a4, plain)?;
    let before = s.error();
    let found = s.get(129, false)?;
    let search_missing_primed = json!([before, i32::from(found != 0), s.error()]);
    s.heap.dispose(&mut s.m, plain).map_err(|e| e.to_string())?;
    s.heap.resize(&mut s.m, h, 6).map_err(|e| e.to_string())?;
    let p = s.heap.data(h).map_err(|e| e.to_string())?;
    s.m.put(p as usize, b"WXYZ!\0")?;
    s.api(0xa9aa, 0, &[(h, 4)])?;
    let change_error = s.error();
    s.api(0xa9b0, 0, &[(h, 4)])?;
    let write_error = s.error();
    let attr_written = s.word(0xa9a6, h)?;
    s.api(0xa999, 0, &[(ar as u16 as u32, 2)])?;
    let update_error = s.error();
    s.create(b)?;
    let create_b_same = i16::from(s.current() == ar);
    let br = s.open(b, 3)?;
    let second_open = json!([i32::from(br != -1), s.error(), i16::from(s.current() == br)]);
    let found = s.get(128, true)?;
    let second_get1 = json!([i32::from(found != 0), s.error()]);
    let found = s.get(128, false)?;
    let second_search = json!([i32::from(found == h), s.error()]);
    let search_current = s.current();
    s.api(0xa998, 0, &[(br as u16 as u32, 2)])?;
    let repeat_before = s.current();
    let again = s.open(a, 3)?;
    let repeated_error = s.error();
    let repeat_current = s.current();
    let repeated_open = json!([
        i16::from(again == ar),
        repeated_error,
        i16::from(repeat_current == br)
    ]);
    s.api(0xa998, 0, &[(app as u16 as u32, 2)])?;
    let found = s.get(128, false)?;
    let app_search = json!([i32::from(found != 0), s.error()]);
    s.api(0xa998, 0, &[(ar as u16 as u32, 2)])?;
    s.api(0xa99a, 0, &[(br as u16 as u32, 2)])?;
    let close_other = json!([s.error(), i16::from(s.current() == ar)]);
    s.api(0xa99a, 0, &[(ar as u16 as u32, 2)])?;
    let close_current = json!([s.error(), i16::from(s.current() == app)]);
    let rr = s.open(a, 1)?;
    let read_open = json!([i32::from(rr != -1), s.error(), i16::from(s.current() == rr)]);
    let found = s.get(128, true)?;
    let attr_read = s.word(0xa9a6, found)?;
    s.api(
        0xa9a8,
        0,
        &[(0x110700, 4), (0x110800, 4), (0x110804, 4), (found, 4)],
    )?;
    let read_info = json!([
        s.error() as i64,
        s.m.read_word(0x110804) as i16 as i64,
        s.m.read_long(0x110800) as i64,
        attr_read as i64
    ]);
    let n = s.m.read_byte(0x110700) as u32;
    let name_hex: String = (0..n)
        .map(|k| format!("{:02x}", s.m.read_byte(0x110701 + k)))
        .collect();
    let data_hex: String = s.bytes(found)?.iter().map(|b| format!("{b:02x}")).collect();
    s.api(0xa9a3, 0, &[(found, 4)])?;
    let release_error = s.error();
    let found = s.get(128, true)?;
    let release_reload = json!([
        release_error,
        i32::from(found != 0),
        i32::from(s.bytes(found)? == b"WXYZ!\0")
    ]);
    s.api(0xa992, 0, &[(found, 4)])?;
    let detach_error = s.error();
    let detached_home = s.word(0xa9a4, found)?;
    let detached_home_error = s.error();
    let reloaded = s.get(128, true)?;
    s.api(0xa99a, 0, &[(rr as u16 as u32, 2)])?;
    let detach = json!([
        detach_error as i32,
        detached_home as i32,
        detached_home_error as i32,
        i32::from(reloaded != 0),
        i32::from(reloaded != found),
        i32::from(s.bytes(found)? == b"WXYZ!\0")
    ]);
    s.heap.dispose(&mut s.m, found).map_err(|e| e.to_string())?;
    let info = *b"TEXTIoPr\0\0\0\0\0\0\0\0";
    s.files
        .create(b"ResourceEmpty", info)
        .map_err(|e| e.to_string())?;
    s.files
        .create(b"ResourceCorrupt", info)
        .map_err(|e| e.to_string())?;
    let f = s
        .files
        .open(b"ResourceCorrupt", crate::files::Fork::Resource, 3)
        .map_err(|e| e.to_string())?;
    s.files.write(f, b"bad!").map_err(|e| e.to_string())?;
    s.files.close(f).map_err(|e| e.to_string())?;
    let mut failed = Vec::new();
    for spec in [missing, empty, corrupt] {
        let r = s.open(spec, 1)?;
        if r != -1 {
            return Err("invalid resource fork opened".into());
        }
        failed.push(json!([s.error(), i16::from(s.current() == app)]));
    }
    Ok(json!({"schema":1,"probe":"res-io-v1","resources":{"first_open":first_open,"empty_before":empty_before,"add":[add_error,attr_added,identity,home],"write":[change_error,write_error,attr_written,update_error],"second_create_current_same":create_b_same,"second_get1":second_get1,"second_search":second_search,"repeated_open":repeated_open,"app_search":app_search,"close_other":close_other,"close_current":close_current,"read_open":read_open,"read_info":read_info,"name_hex":name_hex,"data_hex":data_hex,"second_search_current":[i16::from(search_current==ar),i16::from(search_current==br)],"repeated_before_is_second":i16::from(repeat_before==br),"missing_primed":missing_primed,"search_missing_primed":search_missing_primed,"second_open":second_open,"missing_id":missing_id,"repeated_current_is_original":i16::from(repeat_current==ar),"release_reload":release_reload,"detach":detach,"failed_open":failed}}).to_string())
}

pub fn styleuse_probe() -> Result<String, String> {
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    crate::quickdraw::write_rect(
        &mut s.m,
        0x110000,
        crate::managers::Rect {
            top: 30,
            left: 20,
            bottom: 200,
            right: 300,
        },
    );
    let source = s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)])?;
    s.probe_text(source, b"abcde")?;
    s.api(0xa9d1, 0, &[(source, 4), (3, 4), (1, 4)])?;
    let style = crate::styledtext::Style::new(4, 0, 9, [0; 3], &s.fonts)?;
    style.write_attributes(&mut s.m, 0x110400);
    s.api(0xa83d, 1, &[(source, 4), (0, 1), (0x110400, 4), (5, 2)])?;
    s.api(0xa9d1, 0, &[(source, 4), (5, 4), (0, 4)])?;
    let scrap = s.api(0xa83d, 6, &[(source, 4)])?;
    let mut rows = Vec::new();
    for (i, (start, end)) in [(0, 6), (1, 5), (2, 2), (0, 32767)].into_iter().enumerate() {
        let h = s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)])?;
        let te = s.m.read_long(h);
        let sh = s.m.read_long(te + 74);
        let p = s.m.read_long(sh);
        s.m.write_long(p + 12, 0x12345678);
        s.probe_text(h, b"uvwxyz")?;
        s.api(0xa9d1, 0, &[(h, 4), (5, 4), (4, 4)])?;
        s.heap.set_state(scrap, 64).map_err(|e| e.to_string())?;
        let before = s.heap.state(scrap).map_err(|e| e.to_string())?;
        s.api(
            0xa83d,
            11,
            &[
                (h, 4),
                ((i == 0) as u32, 1),
                (scrap, 4),
                (end, 4),
                (start, 4),
            ],
        )?;
        let after = s.heap.state(scrap).map_err(|e| e.to_string())?;
        let document = s.state(h)?;
        rows.push(json!({"range":[start,end],"handle_state":[before,after],"document":document}));
        s.api(0xa9cd, 0, &[(h, 4)])?;
    }
    s.heap.set_state(scrap, 0).map_err(|e| e.to_string())?;
    s.heap.dispose(&mut s.m, scrap).map_err(|e| e.to_string())?;
    s.api(0xa9cd, 0, &[(source, 4)])?;
    Ok(json!({"schema":1,"probe":"style-use-v1","textedit":{"cases":rows}}).to_string())
}

pub fn stylescrap_probe() -> Result<String, String> {
    let mut s = StyledProbe::new()?;
    crate::quickdraw::write_rect(
        &mut s.m,
        0x110000,
        crate::managers::Rect {
            top: 0,
            left: 0,
            bottom: 80,
            right: 150,
        },
    );
    let h = s.api(0xa83e, 0, &[(0x110000, 4), (0x110000, 4)])?;
    s.probe_text(h, b"abcde")?;
    s.api(0xa9d1, 0, &[(h, 4), (3, 4), (1, 4)])?;
    let style = crate::styledtext::Style::new(4, 0, 9, [0; 3], &s.fonts)?;
    style.write_attributes(&mut s.m, 0x110400);
    s.api(0xa83d, 1, &[(h, 4), (0, 1), (0x110400, 4), (5, 2)])?;
    let mut rows = Vec::new();
    for (start, end) in [(0u16, 5u16), (0, 32767), (3, 32767)] {
        s.direct_select(h, start, end);
        let scrap = s.api(0xa83d, 6, &[(h, 4)])?;
        let n = s.heap.size(scrap).map_err(|e| e.to_string())?;
        let p = s.heap.data(scrap).map_err(|e| e.to_string())?;
        let hex: String = (0..n)
            .map(|k| format!("{:02x}", s.m.read_byte(p + k)))
            .collect();
        let te = s.m.read_long(h);
        rows.push(serde_json::json!({"selection":[start,end],"after":[s.m.read_word(te+32),s.m.read_word(te+34)],"hex":hex}));
        s.heap.dispose(&mut s.m, scrap).map_err(|e| e.to_string())?;
    }
    s.api(0xa9cd, 0, &[(h, 4)])?;
    Ok(
        serde_json::json!({"schema":1,"probe":"style-scrap-v1","textedit":{"cases":rows}})
            .to_string(),
    )
}

/// A separate process reopens the owned resource-I/O fixture from Files storage.
pub fn resio_reload_probe() -> Result<String, String> {
    let mut s = ResIOState::new()?;
    let app = s.current();
    s.spec(b"ResourceA", 0x110000);
    let r = s.open(0x110000, 1)?;
    let opened = serde_json::json!([i32::from(r != -1), s.error(), i16::from(s.current() == r)]);
    if r == -1 {
        return Err("persisted resource map failed to reopen".into());
    }
    let h = s.get(128, true)?;
    if h == 0 {
        return Err("persisted resource missing".into());
    }
    let attrs = s.word(0xa9a6, h)?;
    let home = i16::from(s.word(0xa9a4, h)? == r);
    s.api(
        0xa9a8,
        0,
        &[(0x110700, 4), (0x110800, 4), (0x110804, 4), (h, 4)],
    )?;
    let info = serde_json::json!([
        s.error() as i64,
        s.m.read_word(0x110804) as i16 as i64,
        s.m.read_long(0x110800) as i64,
        attrs as i64
    ]);
    let n = s.m.read_byte(0x110700) as u32;
    let name: String = (0..n)
        .map(|k| format!("{:02x}", s.m.read_byte(0x110701 + k)))
        .collect();
    let data: String = s.bytes(h)?.iter().map(|b| format!("{b:02x}")).collect();
    s.api(0xa99a, 0, &[(r as u16 as u32, 2)])?;
    Ok(serde_json::json!({"schema":1,"probe":"res-io-reload-v1","resources":{"read_open":opened,"read_info":info,"name_hex":name,"data_hex":data,"home_matches":home,"close":[s.error(),i16::from(s.current()==app)]}}).to_string())
}

/// Replay the public visible-window geometry and retained-bit observations.
pub fn windowgeometry_probe() -> Result<String, String> {
    use crate::{
        managers::{Heap, Rect},
        quickdraw::{read_rect, QuickDraw},
        regions::Region,
    };
    use serde_json::json;
    fn r(r: Rect) -> serde_json::Value {
        json!([r.top, r.left, r.bottom, r.right])
    }
    fn snapshot(
        m: &mut Memory,
        q: &QuickDraw,
        windows: &crate::windows::Windows,
        w: u32,
    ) -> Result<serde_json::Value, String> {
        let mut v = serde_json::Map::new();
        v.insert("current".into(), json!((q.get_port(m)? == w) as u8));
        v.insert("front".into(), json!((windows.front() == w) as u8));
        v.insert("port".into(), r(read_rect(m, w + 16)));
        let bitmap = q.port_bitmap(m, w)?;
        let bounds = read_rect(m, bitmap + 6);
        v.insert("bitmap".into(), r(bounds));
        for (name, offset) in [
            ("structure", 114),
            ("content", 118),
            ("update", 122),
            ("visible", 24),
        ] {
            let h = m.read_long(w + offset);
            let p = m.read_long(h);
            v.insert(name.into(), r(read_rect(m, p + 2)));
        }
        let pixels: Vec<u8> = [(25i16, 15i16), (5, 5)]
            .iter()
            .map(|(x, y)| {
                let x = *x - bounds.left;
                let y = *y - bounds.top;
                let b = m.read_byte(q.screen_base + y as u32 * q.row_bytes() as u32 + x as u32 / 8);
                (b & (0x80 >> (x & 7)) != 0) as u8
            })
            .collect();
        v.insert("local_pixels".into(), json!(pixels));
        let update = m.read_long(w + 122);
        let region = Region::read(m, update)?;
        v.insert(
            "update_hits".into(),
            json!([(130, 120), (150, 345), (275, 130)].map(|(y, x)| region.contains(y, x) as u8)),
        );
        Ok(serde_json::Value::Object(v))
    }
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 180,
            right: 250,
        },
        b"Geometry",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    q.set_port(&mut m, w)?;
    let update = m.read_long(w + 122);
    Region::default().store(&mut m, &mut heap, update)?;
    q.paint_rect(
        &mut m,
        Rect {
            top: 0,
            left: 0,
            bottom: 120,
            right: 200,
        },
        [0; 8],
        8,
    )?;
    q.paint_rect(
        &mut m,
        Rect {
            top: 10,
            left: 20,
            bottom: 30,
            right: 50,
        },
        [255; 8],
        8,
    )?;
    let mut states = serde_json::Map::new();
    states.insert("initial".into(), snapshot(&mut m, &q, &windows, w)?);
    for (name, op, x, y, flag) in [
        ("moved", 0xa91b, 90, 100, false),
        ("moved_front", 0xa91b, 110, 120, true),
        ("shrunk", 0xa91d, 160, 90, true),
        ("expanded_no_update", 0xa91d, 220, 140, false),
        ("expanded_update", 0xa91d, 260, 180, true),
        ("zero_size", 0xa91d, 0, 0, true),
        ("moved_dirty", 0xa91b, 140, 150, false),
    ] {
        if name == "expanded_no_update" {
            Region::default().store(&mut m, &mut heap, update)?;
        }
        if name == "moved_dirty" {
            Region::rectangle(Rect {
                top: 130,
                left: 130,
                bottom: 150,
                right: 160,
            })
            .store(&mut m, &mut heap, update)?;
        }
        let sp = 0x7d0000;
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        m.write_byte(sp, flag as u8);
        m.write_word(sp + 2, y);
        m.write_word(sp + 4, x);
        m.write_long(sp + 6, w);
        if !windows.dispatch(
            op,
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            &fonts,
            &mut events,
            &[],
            |_, _, _| Err("unexpected resource load".into()),
        )? || cpu.pc != 0x19002
            || cpu.dar[15] != sp + 10
        {
            return Err("window geometry Pascal ABI mismatch".into());
        }
        states.insert(name.into(), snapshot(&mut m, &q, &windows, w)?);
    }
    Ok(json!({"schema":1,"probe":"window-geometry-v1","windows":states}).to_string())
}

#[cfg(test)]
mod window_geometry_tests {
    use super::*;
    use crate::{managers::Rect, regions::Region};
    #[test]
    fn moving_an_occluded_window_does_not_copy_foreground_pixels_and_marks_missing_content() {
        let mut s = StyledProbe::new().unwrap();
        let lower = s
            .windows
            .create(
                &mut s.m,
                &mut s.heap,
                &s.q,
                &s.fonts,
                &mut s.events,
                0,
                Rect {
                    top: 60,
                    left: 50,
                    bottom: 180,
                    right: 250,
                },
                b"Lower",
                true,
                0,
                u32::MAX,
                true,
                0,
            )
            .unwrap();
        s.q.set_port(&mut s.m, lower).unwrap();
        s.q.paint_rect(
            &mut s.m,
            Rect {
                top: 0,
                left: 0,
                bottom: 120,
                right: 200,
            },
            [255; 8],
            8,
        )
        .unwrap();
        let upper = s
            .windows
            .create(
                &mut s.m,
                &mut s.heap,
                &s.q,
                &s.fonts,
                &mut s.events,
                0,
                Rect {
                    top: 80,
                    left: 100,
                    bottom: 200,
                    right: 300,
                },
                b"Upper",
                true,
                0,
                u32::MAX,
                true,
                0,
            )
            .unwrap();
        s.q.set_port(&mut s.m, upper).unwrap();
        s.q.paint_rect(
            &mut s.m,
            Rect {
                top: 0,
                left: 0,
                bottom: 120,
                right: 200,
            },
            [0; 8],
            8,
        )
        .unwrap();
        let update = s.m.read_long(lower + 122);
        Region::default()
            .store(&mut s.m, &mut s.heap, update)
            .unwrap();
        s.windows
            .move_window(
                &mut s.m,
                &mut s.heap,
                &s.q,
                &s.fonts,
                &mut s.events,
                lower,
                340,
                80,
                false,
            )
            .unwrap();
        let mut pixel = |x: u32, y: u32| {
            s.m.read_byte(s.q.screen_base + y * s.q.row_bytes() as u32 + x / 8) & (0x80 >> (x & 7))
                != 0
        };
        assert!(!pixel(150, 110), "foreground content was damaged");
        assert!(pixel(345, 90), "visible lower content was not copied");
        assert!(
            !pixel(440, 130),
            "occluded source content must await redraw"
        );
        assert!(
            Region::read(&mut s.m, update).unwrap().contains(130, 440),
            "unavailable pixels must be invalidated"
        );
        assert_eq!(s.q.get_port(&mut s.m).unwrap(), upper);
        assert_eq!(s.windows.front(), upper);
    }
}

/// Fixed input gestures replayed through the production Window Manager trap ABI.
pub fn windowtracking_probe() -> Result<String, String> {
    use crate::{managers::Rect, quickdraw::read_rect};
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    let w = s.windows.create(
        &mut s.m,
        &mut s.heap,
        &s.q,
        &s.fonts,
        &mut s.events,
        0,
        Rect {
            top: 100,
            left: 100,
            bottom: 260,
            right: 380,
        },
        b"Tracking",
        true,
        0,
        u32::MAX,
        false,
        0,
    )?;
    s.q.set_port(&mut s.m, w)?;
    let mut rows = Vec::new();
    for (kind, start, end, modifiers) in [
        (4, (80i16, 251i16), (80i16, 251i16), 1u16),
        (4, (80, 251), (136, 307), 0),
        (4, (136, 307), (2, 137), 0),
        (5, (314, 437), (314, 437), 0),
        (5, (314, 437), (329, 453), 0),
        (5, (329, 453), (329, 453), 0),
    ] {
        let op = if kind == 4 { 0xa925 } else { 0xa92b };
        let sp = 0x7d0000;
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        let limits = if kind == 4 {
            Rect {
                top: 50,
                left: 50,
                bottom: 450,
                right: 600,
            }
        } else {
            Rect {
                top: 80,
                left: 100,
                bottom: 220,
                right: 300,
            }
        };
        crate::quickdraw::write_rect(&mut s.m, 0x110000, limits);
        s.m.write_long(sp, 0x110000);
        s.m.write_word(sp + 4, start.0 as u16);
        s.m.write_word(sp + 6, start.1 as u16);
        s.m.write_long(sp + 8, w);
        s.m.write_long(sp + 12, 0xa5a5a5a5);
        let before = read_rect(&mut s.m, w + 16);
        let port = s.q.get_port(&mut s.m)?;
        s.events.set_mouse(&mut s.m, start);
        s.events.modifiers = modifiers;
        s.events.mouse_down_modifiers = modifiers;
        if !s.windows.dispatch(
            op,
            &mut cpu,
            &mut s.m,
            &mut s.heap,
            &mut s.q,
            &s.fonts,
            &mut s.events,
            &[],
            |_, _, _| Err("unexpected resource load".into()),
        )? || !s.windows.is_tracking()
            || cpu.pc != 0x19000
            || cpu.dar[15] != sp
        {
            return Err("tracking prematurely returned before release".into());
        }
        s.events.set_mouse(&mut s.m, end);
        s.events.modifiers = modifiers | 0x80;
        if !s.windows.dispatch(
            op,
            &mut cpu,
            &mut s.m,
            &mut s.heap,
            &mut s.q,
            &s.fonts,
            &mut s.events,
            &[],
            |_, _, _| Err("unexpected resource load".into()),
        )? || s.windows.is_tracking()
            || cpu.pc != 0x19002
            || cpu.dar[15] != sp + 12
        {
            return Err("tracking return ABI mismatch".into());
        }
        let result = if kind == 5 { s.m.read_long(sp + 12) } else { 0 };
        let after = read_rect(&mut s.m, w + 16);
        let h = s.m.read_long(w + 118);
        let p = s.m.read_long(h);
        let content = read_rect(&mut s.m, p + 2);
        rows.push(json!({"kind":kind,"start":[start.0,start.1],"end":[end.0,end.1],"modifiers":modifiers,"result":result,"port_preserved":(s.q.get_port(&mut s.m)?==port) as u8,"before":[before.top,before.left,before.bottom,before.right],"after_tracking":[after.top,after.left,after.bottom,after.right],"content":[content.top,content.left,content.bottom,content.right]}));
        if kind == 5 && result != 0 {
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            s.m.write_byte(sp, 1);
            s.m.write_word(sp + 2, (result >> 16) as u16);
            s.m.write_word(sp + 4, result as u16);
            s.m.write_long(sp + 6, w);
            s.windows.dispatch(
                0xa91d,
                &mut cpu,
                &mut s.m,
                &mut s.heap,
                &mut s.q,
                &s.fonts,
                &mut s.events,
                &[],
                |_, _, _| Err("unexpected resource load".into()),
            )?;
        }
    }
    Ok(json!({"schema":1,"probe":"window-tracking-v1","windows":{"cases":rows}}).to_string())
}

#[cfg(test)]
mod window_tracking_tests {
    use super::*;
    use crate::managers::Rect;
    fn window(s: &mut StyledProbe, b: Rect) -> u32 {
        s.windows
            .create(
                &mut s.m,
                &mut s.heap,
                &s.q,
                &s.fonts,
                &mut s.events,
                0,
                b,
                b"Track",
                true,
                0,
                u32::MAX,
                false,
                0,
            )
            .unwrap()
    }
    fn prepare(s: &mut StyledProbe, w: u32, limits: Rect, start: (i16, i16)) -> CpuCore {
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = 0x7d0000;
        crate::quickdraw::write_rect(&mut s.m, 0x110000, limits);
        s.m.write_long(cpu.dar[15], 0x110000);
        s.m.write_word(cpu.dar[15] + 4, start.0 as u16);
        s.m.write_word(cpu.dar[15] + 6, start.1 as u16);
        s.m.write_long(cpu.dar[15] + 8, w);
        s.m.write_long(cpu.dar[15] + 12, 0xa5a5a5a5);
        cpu
    }
    fn step(s: &mut StyledProbe, cpu: &mut CpuCore, op: u16) {
        assert!(s
            .windows
            .dispatch(
                op,
                cpu,
                &mut s.m,
                &mut s.heap,
                &mut s.q,
                &s.fonts,
                &mut s.events,
                &[],
                |_, _, _| Err("unexpected load".into())
            )
            .unwrap());
    }
    fn pixels(s: &mut StyledProbe) -> Vec<u8> {
        (0..s.q.row_bytes() as u32 * s.q.height as u32)
            .map(|i| s.m.read_byte(s.q.screen_base + i))
            .collect()
    }
    #[test]
    fn grow_limits_preserve_record_and_restore_outline_until_caller_resizes() {
        for (end, expected) in [((-500, -500), 0x00500064u32), ((500, 500), 0x00dc012c)] {
            let mut s = StyledProbe::new().unwrap();
            let w = window(
                &mut s,
                Rect {
                    top: 60,
                    left: 50,
                    bottom: 180,
                    right: 250,
                },
            );
            s.q.set_port(&mut s.m, w).unwrap();
            let before = pixels(&mut s);
            let mut cpu = prepare(
                &mut s,
                w,
                Rect {
                    top: 80,
                    left: 100,
                    bottom: 220,
                    right: 300,
                },
                (170, 240),
            );
            s.events.set_mouse(&mut s.m, (170, 240));
            s.events.modifiers = 0;
            step(&mut s, &mut cpu, 0xa92b);
            s.events.set_mouse(&mut s.m, end);
            step(&mut s, &mut cpu, 0xa92b);
            assert_eq!(cpu.pc, 0x19000);
            assert_eq!(s.m.read_long(0x7d000c), 0xa5a5a5a5);
            s.events.modifiers = 0x80;
            step(&mut s, &mut cpu, 0xa92b);
            assert_eq!(s.m.read_long(0x7d000c), expected);
            assert_eq!(cpu.dar[15], 0x7d000c);
            assert_eq!(
                crate::quickdraw::read_rect(&mut s.m, w + 16),
                Rect {
                    top: 0,
                    left: 0,
                    bottom: 120,
                    right: 200
                }
            );
            assert_eq!(pixels(&mut s), before);
            assert_eq!(s.q.get_port(&mut s.m).unwrap(), w);
        }
    }
    #[test]
    fn command_drag_uses_press_modifiers_and_cancel_restores_pixels() {
        let mut s = StyledProbe::new().unwrap();
        let w = window(
            &mut s,
            Rect {
                top: 60,
                left: 50,
                bottom: 180,
                right: 250,
            },
        );
        let front = window(
            &mut s,
            Rect {
                top: 250,
                left: 300,
                bottom: 350,
                right: 500,
            },
        );
        s.q.set_port(&mut s.m, front).unwrap();
        let before = pixels(&mut s);
        let limits = Rect {
            top: 20,
            left: 0,
            bottom: 480,
            right: 640,
        };
        let mut cpu = prepare(&mut s, w, limits, (48, 100));
        s.events.set_mouse(&mut s.m, (48, 100));
        s.events.modifiers = 0x100;
        s.events.mouse_down_modifiers = 0x100;
        step(&mut s, &mut cpu, 0xa925);
        s.events.set_mouse(&mut s.m, (10, 110));
        s.events.modifiers = 0x80;
        step(&mut s, &mut cpu, 0xa925);
        assert_eq!(pixels(&mut s), before);
        assert_eq!(s.windows.front(), front);
        let mut cpu = prepare(&mut s, w, limits, (48, 100));
        s.events.set_mouse(&mut s.m, (48, 100));
        s.events.modifiers = 0x100;
        s.events.mouse_down_modifiers = 0x100;
        step(&mut s, &mut cpu, 0xa925);
        s.events.set_mouse(&mut s.m, (68, 120));
        s.events.modifiers = 0x80;
        step(&mut s, &mut cpu, 0xa925);
        assert_eq!(s.windows.front(), front);
        assert_eq!(s.q.get_port(&mut s.m).unwrap(), front);
        assert_eq!(
            s.windows
                .windows
                .iter()
                .find(|v| v.address == w)
                .unwrap()
                .bounds,
            Rect {
                top: 80,
                left: 70,
                bottom: 200,
                right: 270
            }
        );
    }
}

struct ControlHitState {
    s: StyledProbe,
    c: crate::controls::Controls,
    w: u32,
}
impl ControlHitState {
    fn api(&mut self, op: u16, args: &[(u32, u32)]) -> Result<u16, String> {
        let sp = 0x7d0000;
        let mut at = sp;
        for (value, size) in args {
            match size {
                2 => self.s.m.write_word(at, *value as u16),
                4 => self.s.m.write_long(at, *value),
                _ => return Err("control probe argument width".into()),
            };
            at += size;
        }
        self.s.m.write_word(at, 0xa5a5);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        if !self
            .c
            .dispatch(op, &mut cpu, &mut self.s.m, &mut self.s.heap, &self.s.q)?
            || cpu.pc != 0x19002
            || cpu.dar[15] != at
        {
            return Err(format!("control probe ABI mismatch {op:#06x}"));
        }
        Ok(self.s.m.read_word(at))
    }
    fn point(&mut self, h: u32, v: i16, x: i16) -> Result<serde_json::Value, String> {
        let part = self.api(
            0xa966,
            &[(((v as u16 as u32) << 16) | x as u16 as u32, 4), (h, 4)],
        )?;
        Ok(serde_json::json!([v, x, part]))
    }
    fn create(
        &mut self,
        r: crate::managers::Rect,
        title: &[u8],
        value: i16,
        min: i16,
        max: i16,
        kind: i16,
    ) -> Result<u32, String> {
        self.c.create(
            &mut self.s.m,
            &mut self.s.heap,
            self.w,
            r,
            title,
            true,
            value,
            min,
            max,
            kind,
            0,
        )
    }
}
pub fn controlhit_probe() -> Result<String, String> {
    use crate::{managers::Rect, quickdraw::read_rect};
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    let w = s.windows.create(
        &mut s.m,
        &mut s.heap,
        &s.q,
        &s.fonts,
        &mut s.events,
        0,
        Rect {
            top: 60,
            left: 50,
            bottom: 280,
            right: 450,
        },
        b"Hits",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    s.q.set_port(&mut s.m, w)?;
    let mut c = ControlHitState {
        s,
        c: crate::controls::Controls::new(),
        w,
    };
    let mut scrollbars = Vec::new();
    for mode in 0..11 {
        let mut r = Rect {
            top: 20,
            left: 10,
            bottom: 180,
            right: 26,
        };
        if mode == 7 {
            r = Rect {
                top: 20,
                left: 40,
                bottom: 36,
                right: 200,
            };
        }
        if mode == 9 {
            r.bottom = 48;
        }
        if mode == 10 {
            r.right = 22;
        }
        let h = c.create(
            r,
            b"",
            if mode == 1 || mode == 7 {
                0
            } else if mode == 2 {
                100
            } else {
                25
            },
            if mode == 7 { -100 } else { 0 },
            if mode == 6 { 0 } else { 100 },
            16,
        )?;
        if mode == 3 {
            c.api(0xa95d, &[(255, 2), (h, 4)])?;
        }
        if mode == 4 {
            c.api(0xa958, &[(h, 4)])?;
        }
        if mode == 8 {
            c.api(0xa95d, &[(20, 2), (h, 4)])?;
        }
        if mode == 5 {
            c.s.windows.visible(
                &mut c.s.m,
                &mut c.s.heap,
                &c.s.q,
                &c.s.fonts,
                &mut c.s.events,
                w,
                false,
            )?;
        }
        let port = c.s.q.get_port(&mut c.s.m)?;
        let p = c.s.m.read_long(h);
        let r = read_rect(&mut c.s.m, p + 8);
        let range = [
            c.s.m.read_word(p + 18) as i16,
            c.s.m.read_word(p + 20) as i16,
            c.s.m.read_word(p + 22) as i16,
        ];
        let mut points = Vec::new();
        if mode == 7 {
            for axis in r.left - 2..=r.right + 2 {
                points.push(c.point(h, r.top + 8, axis)?);
            }
            for y in [r.left + 8, r.left + 40, r.left + 80, r.right - 8] {
                for x in r.top - 1..=r.bottom {
                    points.push(c.point(h, x, y)?);
                }
            }
        } else {
            for axis in r.top - 2..=r.bottom + 2 {
                points.push(c.point(h, axis, r.left + 8)?);
            }
            for y in [r.top + 8, r.top + 40, r.top + 80, r.bottom - 8] {
                for x in r.left - 1..=r.right {
                    points.push(c.point(h, y, x)?);
                }
            }
        }
        scrollbars.push(json!({"mode":mode,"rect":[r.top,r.left,r.bottom,r.right],"range":range,"points":points,"port_preserved":(c.s.q.get_port(&mut c.s.m)?==port) as u8}));
        if mode == 5 {
            c.s.windows.visible(
                &mut c.s.m,
                &mut c.s.heap,
                &c.s.q,
                &c.s.fonts,
                &mut c.s.events,
                w,
                true,
            )?;
        }
        c.api(0xa955, &[(h, 4)])?;
    }
    let r = Rect {
        top: 20,
        left: 40,
        bottom: 40,
        right: 160,
    };
    let mut buttons = Vec::new();
    for kind in 0..3 {
        let h = c.create(r, b"Pick", 1, 0, 1, kind)?;
        let mut points = Vec::new();
        for y in [19, 20, 21, 25, 30, 38, 39, 40] {
            for x in [39, 40, 41, 47, 50, 55, 56, 60, 100, 159, 160] {
                points.push(c.point(h, y, x)?);
            }
        }
        buttons.push(json!({"kind":kind,"points":points}));
        c.api(0xa955, &[(h, 4)])?;
    }
    let back = c.create(r, b"Back", 0, 0, 1, 0)?;
    let front = c.create(r, b"Front", 0, 0, 1, 0)?;
    let b = c.s.m.read_long(back);
    let f = c.s.m.read_long(front);
    let chain = json!([
        (c.s.m.read_long(w + 140) == back) as u8,
        (c.s.m.read_long(b) == front) as u8,
        (c.s.m.read_long(f) == back) as u8
    ]);
    let mut find = Vec::new();
    for mode in 0..5 {
        if mode == 1 {
            c.api(0xa958, &[(front, 4)])?;
        }
        if mode == 2 {
            c.api(0xa957, &[(front, 4)])?;
            c.api(0xa95d, &[(255, 2), (front, 4)])?;
        }
        if mode == 3 {
            c.api(0xa958, &[(back, 4)])?;
        }
        if mode == 4 {
            c.api(0xa955, &[(front, 4)])?;
            c.api(0xa955, &[(back, 4)])?;
        }
        c.s.m.write_long(0x110000, 0x12345678);
        let part = c.api(0xa96c, &[(0x110000, 4), (w, 4), ((30u32 << 16) | 100, 4)])?;
        let found = c.s.m.read_long(0x110000);
        find.push(json!([
            part,
            if found == front {
                2
            } else if found == back {
                1
            } else if found == 0 {
                0
            } else {
                -1
            }
        ]));
    }
    Ok(json!({"schema":1,"probe":"control-hit-v1","controls":{"scrollbars":scrollbars,"buttons":buttons,"chain":chain,"find":find}}).to_string())
}

/// Fixed inputs shared with our owned native probe; no reference outputs are read.
pub fn tedouble_probe() -> Result<String, String> {
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    s.m.write_long(0x2f0, 32);
    s.events.ticks = 100;
    let texts: [&[u8]; 5] = [
        b"one two,three\rx",
        b"a-b can't 12.34",
        b"ab\tcd  ef",
        b"caf\x8e..fin",
        b"ab.cd a_b a/b a:b a;b a+b ab--cd ab''cd .. fin...end",
    ];
    let mut inputs = Vec::new();
    for (text, offsets) in [
        (0, vec![0, 1, 3, 4, 5, 7, 8, 10, 13, 14, 15]),
        (1, vec![0, 1, 2, 3, 4, 7, 10, 12, 15]),
        (2, vec![1, 2, 3, 5, 6, 7, 9]),
        (3, vec![2]),
    ] {
        for offset in offsets {
            inputs.push((text, offset, offset, 0, false));
        }
    }
    inputs.extend([
        (0, 4, 4, 1, false),
        (0, 4, 5, 0, false),
        (0, 5, 5, 2, false),
        (0, 5, 5, 0, true),
    ]);
    for i in 0..=texts[4].len() {
        inputs.push((4, i as i16, i as i16, 0, false));
    }
    for i in 0..=8 {
        inputs.push((0, i, i, 3, false));
    }
    inputs.extend([(0, 4, 4, 4, false), (0, 5, 5, 5, false)]);
    let mut rows = Vec::new();
    for (text, offset, second, mode, styled) in inputs {
        crate::quickdraw::write_rect(
            &mut s.m,
            0x110000,
            crate::managers::Rect {
                top: 0,
                left: 0,
                bottom: 120,
                right: 300,
            },
        );
        let h = s.api(
            if styled { 0xa83e } else { 0xa9d2 },
            0,
            &[(0x110000, 4), (0x110000, 4)],
        )?;
        s.probe_text(h, texts[text])?;
        let p = s.m.read_long(h);
        s.m.write_long(p + 46, 58);
        s.m.write_word(p + 50, 0xffff);
        s.m.write_word(p + 70, 0);
        if mode == 2 {
            s.direct_select(h, 0, 3);
        }
        if mode == 3 {
            s.direct_select(h, 1, 7);
        }
        if mode == 5 {
            s.api(0xa9d8, 0, &[(h, 4)])?;
        }
        let first = s.te.point(&mut s.m, &s.fonts, h, offset)?;
        let other = s.te.point(&mut s.m, &s.fonts, h, second)?;
        let first = (first.0 - 1, first.1 + 1);
        let other = (other.0 - 1, other.1 + 1 + if mode == 4 { 1 } else { 0 });
        let mut states = Vec::new();
        for i in 0..3 {
            if i == 1 && mode == 1 {
                s.m.write_long(p + 46, 58);
            }
            let point = if i == 0 { first } else { other };
            let extend = mode == 3 || (i > 0 && mode == 2);
            s.api(
                0xa9d4,
                0,
                &[
                    (h, 4),
                    (extend as u32, 1),
                    (point.0 as u16 as u32, 2),
                    (point.1 as u16 as u32, 2),
                ],
            )?;
            states.push(json!([
                s.m.read_word(p + 32),
                s.m.read_word(p + 34),
                s.m.read_word(p + 50) as i16,
                s.m.read_word(p + 70) as i16,
                (s.events.ticks.wrapping_sub(s.m.read_long(p + 46)) <= s.m.read_long(0x2f0)) as u8
            ]));
        }
        rows.push(json!({"text":text,"offsets":[offset,second],"mode":mode,"styled":styled as u8,"points":[[first.0,first.1],[other.0,other.1]],"states":states}));
        s.api(0xa9cd, 0, &[(h, 4)])?;
    }
    Ok(json!({"schema":1,"probe":"te-double-v1","textedit":{"double_time":s.m.read_long(0x2f0),"cases":rows}}).to_string())
}

/// Owned machine-code action procedure assembled from probes/controltrack/action.s.
/// Its nested GetPort and SetControlValue calls run on the same CPU as TrackControl.
fn run_control_action(
    s: &mut StyledProbe,
    controls: &mut crate::controls::Controls,
    cpu: &mut CpuCore,
) -> Result<(), String> {
    for _ in 0..500 {
        if let Some((token, _)) = s.callbacks.finish(cpu, &mut s.m)? {
            if token != 5 {
                return Err("wrong control callback owner".into());
            }
            controls.callback_result()?;
            return Ok(());
        }
        let op = s.m.read_word(cpu.pc);
        if s.events.dispatch_state(op, cpu, &mut s.m, &s.q)? {
            continue;
        }
        if controls.dispatch(op, cpu, &mut s.m, &mut s.heap, &s.q)? {
            continue;
        }
        if op == 0xa874 {
            let sp = cpu.dar[15];
            let out = s.m.read_long(sp);
            let port = s.q.get_port(&mut s.m)?;
            s.m.write_long(out, port);
            cpu.dar[15] += 4;
            cpu.pc += 2;
            continue;
        }
        if !matches!(cpu.step(&mut s.m), StepResult::Ok { .. }) {
            return Err("action callback CPU fault".into());
        }
    }
    Err("action callback instruction limit".into())
}
pub fn controlaction_probe() -> Result<String, String> {
    use crate::managers::Rect;
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    s.callbacks = crate::callbacks::Callbacks::silent();
    let w = s.windows.create(
        &mut s.m,
        &mut s.heap,
        &s.q,
        &s.fonts,
        &mut s.events,
        0,
        Rect {
            top: 100,
            left: 100,
            bottom: 420,
            right: 580,
        },
        b"Actions",
        true,
        0,
        u32::MAX,
        false,
        0,
    )?;
    s.q.set_port(&mut s.m, w)?;
    let mut controls = crate::controls::Controls::new();
    let mut handles = Vec::new();
    for left in [20, 80] {
        handles.push(controls.create(
            &mut s.m,
            &mut s.heap,
            w,
            Rect {
                top: 20,
                left,
                bottom: 220,
                right: left + 16,
            },
            b"",
            true,
            25,
            0,
            100,
            16,
            0,
        )?);
    }
    let code: &[u16] = &[
        0x4a79, 0x0011, 0x0600, 0x6662, 0x33fc, 0x0001, 0x0011, 0x0600, 0x33ef, 0x0004, 0x0011,
        0x0602, 0x206f, 0x0006, 0x23c8, 0x0011, 0x060a, 0x2250, 0x33e9, 0x0012, 0x0011, 0x0604,
        0x7000, 0x1029, 0x0011, 0x33c0, 0x0011, 0x0606, 0x4879, 0x0011, 0x060e, 0xa874, 0x302f,
        0x0004, 0x72ff, 0x0c40, 0x0014, 0x6712, 0x7201, 0x0c40, 0x0015, 0x670a, 0x72f6, 0x0c40,
        0x0016, 0x6702, 0x720a, 0x3029, 0x0012, 0xd041, 0x2f08, 0x3f00, 0xa963, 0x205f, 0x4fef,
        0x0006, 0x4ed0,
    ];
    for (i, word) in code.iter().enumerate() {
        s.m.write_word(0x180000 + 2 * i as u32, *word);
    }
    // Measured input coordinates are pinned here after native capture. No output
    // file is read by this replay or by production control tracking.
    let inputs: &[(usize, (i16, i16), (i16, i16))] = &[
        (0, (195, 32), (195, 32)),
        (0, (200, 32), (200, 32)),
        (0, (212, 32), (212, 32)),
        (0, (39, 32), (39, 32)),
        (0, (142, 32), (142, 32)),
        (1, (142, 82), (142, 82)),
        (1, (142, 82), (142, 233)),
        (0, (142, 34), (142, 34)),
    ];
    let mut rows = Vec::new();
    for &(index, start, end) in inputs {
        let h = handles[index];
        let p = s.m.read_long(h);
        for n in 0..20 {
            s.m.write_byte(0x110600 + n, 0);
        }
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(crate::environment::CPU_TYPE);
        cpu.pc = 0x19000;
        cpu.dar[15] = 0x7d0000;
        let sp = cpu.dar[15];
        s.m.write_long(sp, if index == 0 { 0x180000 } else { 0 });
        s.m.write_word(sp + 4, start.0 as u16);
        s.m.write_word(sp + 6, start.1 as u16);
        s.m.write_long(sp + 8, h);
        s.m.write_word(sp + 12, 0xa5a5);
        let initial = controls.test(&mut s.m, h, start.0, start.1)?;
        let port = s.q.get_port(&mut s.m)?;
        s.events.ticks = 100;
        s.events.modifiers = 0;
        s.events.set_mouse(&mut s.m, (start.0 + 100, start.1 + 100));
        controls.track(&mut cpu, &mut s.m, &s.q, &s.events, &mut s.callbacks)?;
        if cpu.pc == 0x180000 {
            run_control_action(&mut s, &mut controls, &mut cpu)?;
        }
        if !controls.is_tracking() || cpu.pc != 0x19000 || cpu.dar[15] != sp {
            return Err("TrackControl returned before release".into());
        }
        s.events.set_mouse(&mut s.m, (end.0 + 100, end.1 + 100));
        s.events.modifiers = 0x80;
        controls.track(&mut cpu, &mut s.m, &s.q, &s.events, &mut s.callbacks)?;
        if controls.is_tracking() || cpu.pc != 0x19002 || cpu.dar[15] != sp + 12 {
            return Err("TrackControl release ABI mismatch".into());
        }
        let callback = if s.m.read_word(0x110600) == 0 {
            json!([0, 0, 0, 0, 0])
        } else {
            json!([
                s.m.read_word(0x110600),
                s.m.read_word(0x110602),
                s.m.read_word(0x110604),
                s.m.read_word(0x110606),
                ((s.m.read_long(0x11060a) == h) && (s.m.read_long(0x11060e) == w)) as u8
            ])
        };
        rows.push(json!({"control":index,"start":[start.0,start.1],"end":[end.0,end.1],"initial_part":initial,"result":s.m.read_word(sp+12),"value":s.m.read_word(p+18),"hilite":s.m.read_byte(p+17),"port_preserved":(s.q.get_port(&mut s.m)?==port) as u8,"callback":callback}));
        for h in &handles {
            let p = s.m.read_long(*h);
            s.m.write_word(p + 18, 25);
        }
    }
    Ok(json!({"schema":1,"probe":"control-action-v1","controls":{"cases":rows}}).to_string())
}

/// Fixed owned gestures; this replay never reads native expected output.
pub fn controlthumb_probe() -> Result<String, String> {
    use crate::managers::Rect;
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    s.callbacks = crate::callbacks::Callbacks::silent();
    let w = s.windows.create(
        &mut s.m,
        &mut s.heap,
        &s.q,
        &s.fonts,
        &mut s.events,
        0,
        Rect {
            top: 41,
            left: 4,
            bottom: 476,
            right: 571,
        },
        b"Thumb",
        true,
        0,
        u32::MAX,
        false,
        0,
    )?;
    s.q.set_port(&mut s.m, w)?;
    let mut controls = crate::controls::Controls::new();
    let h = controls.create(
        &mut s.m,
        &mut s.heap,
        w,
        Rect {
            top: 0,
            left: 552,
            bottom: 420,
            right: 567,
        },
        b"",
        true,
        25,
        0,
        100,
        16,
        0,
    )?;
    let p = s.m.read_long(h);
    // Assembled from probes/controlthumb/action.s: real no-argument callback
    // executes GetMouse, Button and GetControlValue through shared dispatch.
    let code: &[u16] = &[
        0x4a79, 0x0011, 0x0600, 0x6630, 0x33fc, 0x0001, 0x0011, 0x0600, 0x4879, 0x0011, 0x0602,
        0xa972, 0x558f, 0xa974, 0x7000, 0x1017, 0x548f, 0x33c0, 0x0011, 0x0606, 0x558f, 0x2f39,
        0x0011, 0x0620, 0xa960, 0x33df, 0x0011, 0x0608, 0x4e75,
    ];
    for (i, word) in code.iter().enumerate() {
        s.m.write_word(0x180000 + i as u32 * 2, *word);
    }
    s.m.write_long(0x110620, h);
    let mut rows = Vec::new();
    for action in [0, 1] {
        for end_v in [122i16, 82, 102] {
            s.m.write_word(p + 18, 25);
            for i in 0..10 {
                s.m.write_byte(0x110600 + i, 0);
            }
            s.events.ticks = 100;
            s.events.modifiers = 0;
            s.events.set_mouse(&mut s.m, (143, 564));
            let mut getter = CpuCore::new();
            getter.dar[15] = 0x7c0000;
            s.m.write_long(getter.dar[15], 0x110640);
            s.events
                .dispatch_state(0xa972, &mut getter, &mut s.m, &s.q)?;
            let entry = [s.m.read_word(0x110640), s.m.read_word(0x110642)];
            s.events
                .dispatch_state(0xa974, &mut getter, &mut s.m, &s.q)?;
            let button = s.m.read_byte(getter.dar[15]);
            let mut cpu = CpuCore::new();
            cpu.set_cpu_type(crate::environment::CPU_TYPE);
            cpu.pc = 0x19000;
            cpu.dar[15] = 0x7d0000;
            let sp = cpu.dar[15];
            s.m.write_long(sp, if action == 1 { 0x180000 } else { 0 });
            s.m.write_word(sp + 4, 102);
            s.m.write_word(sp + 6, 560);
            s.m.write_long(sp + 8, h);
            s.m.write_word(sp + 12, 0xa5a5);
            let port = s.q.get_port(&mut s.m)?;
            for v in [102, end_v] {
                s.events.set_mouse(&mut s.m, (v + 41, 564));
                s.events.ticks += 1;
                controls.track(&mut cpu, &mut s.m, &s.q, &s.events, &mut s.callbacks)?;
                if cpu.pc == 0x180000 {
                    run_control_action(&mut s, &mut controls, &mut cpu)?;
                }
                if !controls.is_tracking()
                    || cpu.pc != 0x19000
                    || cpu.dar[15] != sp
                    || s.m.read_word(p + 18) != 25
                {
                    return Err("indicator changed public value/stack before release".into());
                }
            }
            s.events.modifiers = 0x80;
            controls.track(&mut cpu, &mut s.m, &s.q, &s.events, &mut s.callbacks)?;
            if controls.is_tracking()
                || cpu.pc != 0x19002
                || cpu.dar[15] != sp + 12
                || s.q.get_port(&mut s.m)? != port
            {
                return Err("indicator release changed caller ABI/port".into());
            }
            let callback = if s.m.read_word(0x110600) == 0 {
                json!([])
            } else {
                json!([
                    s.m.read_word(0x110602),
                    s.m.read_word(0x110604),
                    s.m.read_word(0x110606),
                    s.m.read_word(0x110608)
                ])
            };
            rows.push(json!({"action":action,"start":[102,560],"entry_mouse":entry,"entry_button":button,
                "end":[end_v,560],"result":s.m.read_word(sp+12),"value":s.m.read_word(p+18),"hilite":s.m.read_byte(p+17),"first_callback":callback}));
        }
    }
    Ok(
        json!({"schema":1,"probe":"control-thumb-semantics-v1","controls":{"cases":rows}})
            .to_string(),
    )
}

/// Fixed input gestures from two owned native captures; results are computed
/// solely through the production control/event managers, never read from pins.
pub fn controlthumbedges_probe() -> Result<String, String> {
    use crate::managers::Rect;
    use serde_json::json;
    let mut s = StyledProbe::new()?;
    s.callbacks = crate::callbacks::Callbacks::silent();
    let w = s.windows.create(
        &mut s.m,
        &mut s.heap,
        &s.q,
        &s.fonts,
        &mut s.events,
        0,
        Rect {
            top: 41,
            left: 4,
            bottom: 476,
            right: 571,
        },
        b"Edges",
        true,
        0,
        u32::MAX,
        false,
        0,
    )?;
    s.q.set_port(&mut s.m, w)?;
    let mut controls = crate::controls::Controls::new();
    let gestures: [(i16, i16, i16, i16); 16] = [
        (102, 560, 103, 560),
        (102, 560, 104, 560),
        (102, 560, 101, 560),
        (102, 560, 0, 560),
        (102, 560, 419, 560),
        (102, 560, 122, 580),
        (207, 118, 207, 138),
        (100, 560, -12, 560),
        (102, 560, 122, 600),
        (102, 560, 122, 620),
        (102, 560, 122, 480),
        (105, 554, 125, 434),
        (105, 554, 125, 394),
        (105, 554, 125, 452),
        (207, 118, 99, 138),
        (105, 554, 125, 451),
    ];
    let mut rows = Vec::new();
    for (i, (sv, sh, ev, eh)) in gestures.into_iter().enumerate() {
        let rect = if i % 8 == 6 {
            Rect {
                top: 200,
                left: 20,
                bottom: 215,
                right: 420,
            }
        } else {
            Rect {
                top: 0,
                left: 552,
                bottom: 420,
                right: 567,
            }
        };
        let h = controls.create(&mut s.m, &mut s.heap, w, rect, b"", true, 25, 0, 100, 16, 0)?;
        let p = s.m.read_long(h);
        s.events.ticks = 100;
        s.events.modifiers = 0;
        s.events.set_mouse(&mut s.m, (sv + 41, sh + 4));
        let mut getter = CpuCore::new();
        getter.dar[15] = 0x7c0000;
        s.m.write_long(getter.dar[15], 0x110640);
        s.events
            .dispatch_state(0xa972, &mut getter, &mut s.m, &s.q)?;
        let entry = [
            s.m.read_word(0x110640) as i16,
            s.m.read_word(0x110642) as i16,
        ];
        s.events
            .dispatch_state(0xa974, &mut getter, &mut s.m, &s.q)?;
        let button = s.m.read_byte(getter.dar[15]);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = 0x7d0000;
        let sp = cpu.dar[15];
        s.m.write_long(sp, 0);
        s.m.write_word(sp + 4, sv as u16);
        s.m.write_word(sp + 6, sh as u16);
        s.m.write_long(sp + 8, h);
        s.m.write_word(sp + 12, 0xa5a5);
        let port = s.q.get_port(&mut s.m)?;
        for (v, x) in [(sv, sh), (ev, eh)] {
            s.events.set_mouse(&mut s.m, (v + 41, x + 4));
            s.events.ticks += 1;
            controls.track(&mut cpu, &mut s.m, &s.q, &s.events, &mut s.callbacks)?;
            if !controls.is_tracking()
                || cpu.pc != 0x19000
                || cpu.dar[15] != sp
                || s.m.read_word(p + 18) != 25
            {
                return Err("edge drag altered public value/caller before release".into());
            }
        }
        s.events.modifiers = 0x80;
        controls.track(&mut cpu, &mut s.m, &s.q, &s.events, &mut s.callbacks)?;
        if controls.is_tracking()
            || cpu.pc != 0x19002
            || cpu.dar[15] != sp + 12
            || s.q.get_port(&mut s.m)? != port
        {
            return Err("edge release altered caller ABI/port".into());
        }
        rows.push(json!({"run":i/8,"case":i%8,"rect":[rect.top,rect.left,rect.bottom,rect.right],
            "initial_value":25,"start":[sv,sh],"entry_mouse":entry,"entry_button":button,
            "end":[ev,eh],"result":s.m.read_word(sp+12),"value":s.m.read_word(p+18),"hilite":s.m.read_byte(p+17)}));
    }
    Ok(
        json!({"schema":1,"probe":"control-thumb-edges-semantics-v1","controls":{"cases":rows}})
            .to_string(),
    )
}

pub fn opendf_probe() -> Result<String, String> {
    crate::files::Files::open_df_probe(&mut Memory::zeroed(0x800000))
}

pub fn pathfiles_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::files::Files::path_probe(&mut m)
}

pub fn fcbinfo_probe() -> Result<String, String> {
    let resource = std::fs::read("work/probes/fcbinfo/FCBInfoProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::files::Files::fcb_probe(&mut m, &resource)
}

pub fn cataloginfo_probe() -> Result<String, String> {
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    crate::files::Files::catalog_probe(&mut m)
}

pub fn owned68k_probe(detailed: bool) -> Result<String, String> {
    let mut memory = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let execution = classic_core::execution::owned_probe(&mut memory)?;
    if !detailed {
        return Ok(execution.observation());
    }
    Ok(serde_json::json!({"instructions":execution.instructions,"cpu":"M68020","program_bytes":classic_core::execution::OWNED_PROGRAM.len(),
        "return_pc":execution.return_pc,"return_sp":execution.return_sp,"registers_preserved":execution.registers_preserved,
        "traps":execution.traps.iter().map(|t|serde_json::json!({"pc":t.pc,"opcode":t.opcode,"d0_before":t.d0_before,"d0_after":t.d0_after})).collect::<Vec<_>>()}).to_string())
}

pub fn alerts_probe() -> Result<String, String> {
    use serde_json::json;
    let bytes = std::fs::read("work/probes/alerts/AlertProbe.APPL/..namedfork/rsrc")
        .map_err(|e| e.to_string())?;
    let resources = crate::resource::parse(&bytes)?;
    let filter =
        std::fs::read("work/probes/alerts/runtime-filter.bin").map_err(|e| e.to_string())?;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    m.put(0x19000, &filter)?;
    let mut heap = crate::managers::Heap::new(0x20000, 0xe0000);
    let mut q = crate::quickdraw::QuickDraw::new();
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    q.init(&mut m, 0x100000)?;
    let mut windows = crate::windows::Windows::new();
    let mut events = crate::events::Events::new();
    windows.init(&mut m, &mut heap, &q)?;
    let owner = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        crate::managers::Rect {
            top: 60,
            left: 90,
            bottom: 220,
            right: 460,
        },
        b"Owned alert owner",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    let mut controls = crate::controls::Controls::new();
    let mut dialogs = crate::dialogs::Dialogs::new();
    let mut alerts = crate::alerts::Alerts::new();
    alerts.trace = false;
    let mut callbacks = crate::callbacks::Callbacks::silent();
    let mut script = std::collections::VecDeque::new();
    let mut cases = Vec::new();
    for id in 128..=131u16 {
        for mode in 0..4u16 {
            q.set_port(&mut m, owner)?;
            crate::dialogs::Dialogs::parameter_text(
                &mut m,
                &mut heap,
                [
                    Some(b"first".to_vec()),
                    Some(b"second".to_vec()),
                    Some(vec![]),
                    Some(vec![]),
                ],
            )?;
            let mut cpu = CpuCore::new();
            cpu.set_cpu_type(CpuType::M68020);
            cpu.pc = 0x6000;
            cpu.dar[15] = 0x700000;
            m.write_long(0x700000, 0x19000);
            m.write_word(0x700004, id);
            m.write_word(0x700006, 0x5555);
            let mut capture = None;
            let mut calls = 0;
            let mut done = false;
            for _ in 0..1000 {
                if let Some((token, result)) = callbacks.finish(&mut cpu, &mut m)? {
                    if token != 6 {
                        return Err("Unexpected alert callback token".into());
                    }
                    alerts.callback_result(result)?;
                    continue;
                }
                if cpu.pc == 0x6000 {
                    alerts.dispatch(
                        0xa984 + mode,
                        &mut cpu,
                        &mut m,
                        &mut heap,
                        &q,
                        &fonts,
                        &mut windows,
                        &mut controls,
                        &mut events,
                        &mut dialogs,
                        &mut callbacks,
                        &resources,
                        &mut script,
                    )?;
                    if cpu.pc == 0x6002 {
                        done = true;
                        break;
                    }
                    continue;
                }
                if cpu.pc == 0x19000 {
                    calls += 1;
                    let sp = cpu.dar[15];
                    let d = m.read_long(sp + 12);
                    let event = m.read_long(sp + 8);
                    let hit = m.read_long(sp + 4);
                    let port = crate::quickdraw::read_rect(&mut m, d + 16);
                    let cont = m.read_long(d + 118);
                    let ptr = m.read_long(cont);
                    let content = crate::quickdraw::read_rect(&mut m, ptr + 2);
                    let structure = m.read_long(d + 114);
                    let ptr = m.read_long(structure);
                    let structure = crate::quickdraw::read_rect(&mut m, ptr + 2);
                    let item = &dialogs
                        .records
                        .get(&d)
                        .ok_or("Missing callback dialog")?
                        .items[2];
                    let kind = item.kind;
                    let size = heap.size(item.handle).map_err(|e| e.to_string())?;
                    let ptr = m.read_long(item.handle);
                    let text = (0..size)
                        .map(|i| format!("{:02x}", m.read_byte(ptr + i)))
                        .collect::<String>();
                    capture = Some(
                        json!({"current_dialog":(q.get_port(&mut m)?==d) as u8,"front_dialog":(windows.front()==d) as u8,
                    "kind":m.read_word(d+108) as i16,"visible":(m.read_byte(d+110)!=0) as u8,"default_item":m.read_word(d+168),"edit_field":m.read_word(d+164) as i16,
                    "port_rect":[port.top,port.left,port.bottom,port.right],"content":[content.top,content.left,content.bottom,content.right],
                    "text_kind":kind,"text_hex":text,"structure":[structure.top,structure.left,structure.bottom,structure.right],"event_nonnull":(event!=0) as u8,"hit_nonnull":(hit!=0) as u8}),
                    );
                }
                match cpu.step(&mut m) {
                    StepResult::Ok { .. } => {}
                    other => return Err(format!("Alert filter stopped: {other:?}")),
                }
            }
            if !done || cpu.dar[15] != 0x700006 || windows.windows.len() != 1 {
                return Err("Alert completion/stack/window cleanup failed".into());
            }
            cases.push(json!({"resource":id,"mode":mode,"callback":capture.ok_or("Alert filter never executed")?,"result":m.read_word(0x700006) as i16,"calls":calls,
            "old_port_restored":(q.get_port(&mut m)?==owner) as u8,"owner_front":(windows.front()==owner) as u8}));
        }
    }
    Ok(json!({"schema":1,"probe":"alerts-v1","alerts":{"cases":cases}}).to_string())
}

pub fn time_manager_probe() -> Result<String, String> {
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let procedure = 0x19000u32;
    let task = 0x2000u32;
    m.put(
        procedure as usize,
        include_bytes!("../../probes/timemanager/callback.bin"),
    )?;
    let mut records = Vec::new();
    for mode in 0..2 {
        for input_ccr in [16, 31, 0] {
            for p in task - 4..task + 64 {
                m.write_byte(p, 0);
            }
            m.write_long(task - 4, 0x11223344);
            m.write_long(task + 48, 0x55667788);
            if mode == 0 {
                m.write_long(task + 14, 0x55555555);
                m.write_long(task + 18, 0x66666666);
            }
            let mut timer = crate::timemanager::TimeManager::new(0x800000);
            let mut now = 1_000_000;
            for step in 0..3 {
                let opcode = if step == 0 {
                    if mode == 0 {
                        0xa058
                    } else {
                        0xa458
                    }
                } else if step == 1 {
                    0xa05a
                } else {
                    0xa059
                };
                let mut cpu = CpuCore::new();
                cpu.pc = 0x6000;
                cpu.dar[15] = 0x700000;
                cpu.dar[8] = task;
                cpu.dar[0] = if step == 1 { 10000 } else { 0x12345678 };
                cpu.set_ccr(input_ccr);
                if !timer.dispatch(opcode, &mut cpu, &mut m, now)
                    || cpu.pc != 0x6002
                    || cpu.dar[15] != 0x700000
                {
                    return Err("Time Manager register/PC/stack ABI failure".into());
                }
                let ccr = (u32::from(cpu.flag_x()) << 4)
                    | (u32::from(cpu.flag_n()) << 3)
                    | (u32::from(cpu.flag_z()) << 2)
                    | (u32::from(cpu.flag_v()) << 1)
                    | u32::from(cpu.flag_c());
                records.push(json!({"mode":mode,"step":step,"input_ccr":input_ccr,"d0":cpu.dar[0],"ccr":ccr,"a0_same":(cpu.dar[8]==task) as u8,
                "raw_task":task,"raw_link":m.read_long(task),"type":m.read_word(task+4),"count":m.read_long(task+10) as i32,"wake":m.read_long(task+14) as i32,"reserved":m.read_long(task+18) as i32,
                "proc_same":(m.read_long(task+6)==0) as u8,"guards":[m.read_long(task-4),m.read_long(task+48)]}));
                now += 250;
            }
        }
    }
    let mut rows = Vec::new();
    for mode in 0..2 {
        for kind in 0..3 {
            for p in task - 4..task + 64 {
                m.write_byte(p, 0);
            }
            m.write_long(task - 4, 0x11223344);
            m.write_long(task + 48, 0x55667788);
            m.write_long(task + 6, procedure);
            m.write_long(task + 40, 30);
            m.write_long(task + 44, if kind == 2 { 3 } else { 1 });
            let mut timer = crate::timemanager::TimeManager::new(0x800000);
            let mut cpu = CpuCore::new();
            cpu.set_cpu_type(CpuType::M68020);
            cpu.pc = 0x6000;
            cpu.dar[15] = 0x700000;
            cpu.dar[13] = 0x13572468;
            cpu.dar[8] = task;
            cpu.dar[0] = 0x12345678;
            cpu.set_ccr(16);
            timer.dispatch(
                if mode == 0 { 0xa058 } else { 0xa458 },
                &mut cpu,
                &mut m,
                1_000_000,
            );
            cpu.pc = 0x6000;
            cpu.dar[8] = task;
            cpu.dar[0] = if kind == 1 { -30000i32 as u32 } else { 30 };
            cpu.set_ccr(16);
            timer.dispatch(0xa05a, &mut cpu, &mut m, 1_000_000);
            cpu.set_ccr(31);
            let foreground = cpu.dar;
            let foreground_pc = cpu.pc;
            let foreground_sr = cpu.get_sr();
            let mut callbacks = crate::callbacks::Callbacks::silent();
            for now in (1_000_000..=1_500_000u64).step_by(1000) {
                m.write_long(0x16a, (now * 60 / 1_000_000) as u32);
                if timer.service(&mut cpu, &mut m, now, &mut callbacks)? {
                    let mut finished = false;
                    for _ in 0..1000 {
                        if let Some((token, _)) = callbacks.finish(&mut cpu, &mut m)? {
                            if token != 7 {
                                return Err("Wrong TimerProc callback token".into());
                            }
                            timer.callback_result()?;
                            finished = true;
                            break;
                        }
                        let opcode = m.read_word(cpu.pc);
                        if timer.dispatch(opcode, &mut cpu, &mut m, now) {
                            continue;
                        }
                        match cpu.step(&mut m) {
                            StepResult::Ok { .. } => {}
                            other => return Err(format!("TimerProc CPU stopped: {other:?}")),
                        }
                    }
                    if !finished
                        || cpu.pc != foreground_pc
                        || cpu.dar != foreground
                        || cpu.get_sr() != foreground_sr
                    {
                        return Err("TimerProc foreground register/PC/SP restoration failed".into());
                    }
                }
            }
            rows.push(json!({"mode":mode,"kind":kind,"calls":m.read_long(task+22),"self_same":(m.read_long(task+30)==task) as u8,"entry_a0_proc":(m.read_long(task+26)==procedure) as u8,
            "callback_type":m.read_word(task+34),"callback_count":m.read_long(task+36) as i32,"after_type":m.read_word(task+4),"after_count":m.read_long(task+10) as i32,
            "raw_wake":m.read_long(task+14) as i32,"raw_reserved":m.read_long(task+18) as i32,"ticks":[60,m.read_long(task+52),m.read_long(task+56),m.read_long(task+60),90],"guards":[m.read_long(task-4),m.read_long(task+48)]}));
            timer
                .state
                .remove(&mut m, task, 1_500_000)
                .map_err(|e| e.to_string())?;
        }
    }
    let mut edges = Vec::new();
    for mode in 0..2 {
        for request in [i32::MAX, 86400000, -2147483647, 2147483] {
            for p in task - 4..task + 64 {
                m.write_byte(p, 0);
            }
            m.write_long(task - 4, 0x11223344);
            m.write_long(task + 48, 0x55667788);
            let mut timer = crate::timemanager::TimeManager::new(0x800000);
            timer
                .state
                .install(&mut m, task, mode != 0)
                .map_err(|e| e.to_string())?;
            timer
                .state
                .prime(&mut m, task, request, 1_000_000)
                .map_err(|e| e.to_string())?;
            let kind = m.read_word(task + 4);
            let count = m.read_long(task + 10) as i32;
            timer
                .state
                .remove(&mut m, task, 1_000_250)
                .map_err(|e| e.to_string())?;
            edges.push(json!({"mode":mode,"request":request,"prime_type":kind,"prime_count":count,"remaining":m.read_long(task+10) as i32,"after_type":m.read_word(task+4),"guards":[m.read_long(task-4),m.read_long(task+48)]}));
        }
    }
    Ok(json!({"schema":1,"probe":"time-manager-raw-v2","records":records,"callbacks":rows,"edges":edges}).to_string())
}

pub fn move_high_probe() -> Result<String, String> {
    use serde_json::json;
    let mut cases = Vec::new();
    for mode in 0..9 {
        for flags in [16, 31, 0] {
            let mut m = Memory {
                bytes: vec![0; 0x800000],
            };
            let mut heap = crate::managers::Heap::new(0x20000, 0x500000);
            let guard = heap.new_ptr(&mut m, 16, false).map_err(|e| e.to_string())?;
            let h = if mode == 3 {
                0
            } else {
                heap.new_handle(&mut m, if mode == 2 { 0 } else { 32 }, false)
                    .map_err(|e| e.to_string())?
            };
            let mut other = heap
                .new_handle(&mut m, 48, false)
                .map_err(|e| e.to_string())?;
            let barrier = if mode == 6 || mode == 8 {
                heap.new_ptr(&mut m, 32, false).map_err(|e| e.to_string())?
            } else {
                0
            };
            for i in 0..16 {
                m.write_byte(guard + i, 0xaa);
            }
            let op = m.read_long(other);
            for i in 0..48 {
                m.write_byte(op + i, (i + 33) as u8);
            }
            if h != 0 && mode != 2 {
                let p = m.read_long(h);
                for i in 0..32 {
                    m.write_byte(p + i, (i + 1) as u8);
                }
            }
            if h != 0 {
                heap.set_state(
                    h,
                    match mode {
                        1 => 128,
                        4 => 64,
                        5 => 32,
                        _ => 0,
                    },
                )
                .map_err(|e| e.to_string())?;
            }
            if barrier != 0 {
                for i in 0..32 {
                    m.write_byte(barrier + i, 0xbb);
                }
            }
            if mode == 7 {
                heap.lock(other, true).map_err(|e| e.to_string())?;
            }
            if mode == 8 {
                heap.dispose(&mut m, other).map_err(|e| e.to_string())?;
                other = 0;
            }
            let before = if h == 0 { 0 } else { m.read_long(h) };
            let other_before = if other == 0 { 0 } else { m.read_long(other) };
            let state_before = if h == 0 {
                0
            } else {
                heap.state(h).map_err(|e| e.to_string())?
            };
            let free = heap.free_bytes();
            let mut cpu = CpuCore::new();
            cpu.dar[8] = h;
            cpu.dar[0] = 0x12345678;
            cpu.dar[15] = 0x700000;
            cpu.pc = 0x6000;
            cpu.set_ccr(flags);
            m.write_long(0x700000, 0xcafebabe);
            if !classic_core::traps::memory_dispatch(0xa064, &mut cpu, &mut m, &mut heap) {
                return Err("MoveHHi not dispatched".into());
            }
            let result = cpu.dar[0] as i32;
            let ccr = cpu.get_sr() & 31;
            let mem = m.read_word(0x220) as i16;
            let after = if h == 0 { 0 } else { m.read_long(h) };
            let other_after = if other == 0 { 0 } else { m.read_long(other) };
            let state_after = if h == 0 {
                0
            } else {
                heap.state(h).map_err(|e| e.to_string())?
            };
            let payload = if h == 0 || mode == 2 {
                Vec::new()
            } else {
                (0..32).map(|i| m.read_byte(after + i)).collect::<Vec<_>>()
            };
            let other_bytes = if other == 0 {
                Vec::new()
            } else {
                (0..48)
                    .map(|i| m.read_byte(other_after + i))
                    .collect::<Vec<_>>()
            };
            let guard_bytes = (0..16).map(|i| m.read_byte(guard + i)).collect::<Vec<_>>();
            let barrier_bytes = if barrier == 0 {
                Vec::new()
            } else {
                (0..32)
                    .map(|i| m.read_byte(barrier + i))
                    .collect::<Vec<_>>()
            };
            let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            if heap.free_bytes() != free || m.read_long(0x700000) != 0xcafebabe || cpu.pc != 0x6002
            {
                return Err("MoveHHi changed free bytes/stack guard/PC".into());
            }
            cases.push(json!({"mode":mode,"input_ccr":flags,"result":result,"ccr":ccr,"a0_same":(cpu.dar[8]==h) as u8,"stack_delta":0x700000i64-cpu.dar[15] as i64,"mem_error":mem,"before_ptr":before,"after_ptr":after,"other_before":other_before,"other_after":other_after,"state_before":state_before,"state_after":state_after,"sum":payload.iter().map(|&v|v as u32).sum::<u32>(),"other_sum":other_bytes.iter().map(|&v|v as u32).sum::<u32>(),"guard_ok":guard_bytes.iter().all(|&v|v==0xaa) as u8,"barrier_ptr":barrier,"barrier_ok":barrier_bytes.iter().all(|&v|v==0xbb) as u8,"payload_hex":hex(&payload),"other_hex":hex(&other_bytes),"guard_hex":hex(&guard_bytes),"barrier_hex":hex(&barrier_bytes)}));
        }
    }
    Ok(json!({"schema":1,"probe":"move-high-raw-v4","cases":cases}).to_string())
}

pub fn window_colors_probe() -> Result<String, String> {
    use crate::{managers::Rect, quickdraw::read_rect};
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 0x800000],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0x500000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.color_capable = true;
    q.init(&mut m, 0x100000)?;
    q.device(&mut m, &mut heap)?;
    q.set_depth(&mut m, &mut heap, 8)
        .map_err(|e| e.to_string())?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let resources =
        crate::resource::parse(include_bytes!("../../probes/windowcolors/resources.rsrc"))?;
    let mut cache = std::collections::BTreeMap::new();
    let mut cpu = CpuCore::new();
    let mut cases = Vec::new();
    for id in 22000..22003i16 {
        let old = q.get_port(&mut m)?;
        let mut source =
            if let Some(r) = resources.iter().find(|r| r.kind == *b"wctb" && r.id == id) {
                load_resource(r, &mut m, &mut heap, &mut cache)?
            } else {
                0
            };
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_long(0x700000, u32::MAX);
        m.write_long(0x700004, 0);
        m.write_word(0x700008, id as u16);
        m.write_long(0x70000e, 0x55667788);
        if !windows.dispatch(
            0xaa46,
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            &fonts,
            &mut events,
            &resources,
            |r, m, heap| load_resource(r, m, heap, &mut cache),
        )? || cpu.pc != 0x6002
            || cpu.dar[15] != 0x70000a
            || m.read_long(0x70000e) != 0x55667788
        {
            return Err("GetNewCWindow ABI failed".into());
        }
        let w = m.read_long(0x70000a);
        if w == 0 {
            return Err("Owned resource window missing".into());
        }
        for step in 0..3 {
            if step == 1 {
                let r = resources
                    .iter()
                    .find(|r| r.kind == *b"wctb" && r.id == 22003)
                    .unwrap();
                source = load_resource(r, &mut m, &mut heap, &mut cache)?;
                cpu.pc = 0x6000;
                cpu.dar[15] = 0x700000;
                m.write_long(0x700000, source);
                m.write_long(0x700004, w);
                m.write_long(0x700008, 0x55667788);
                windows.dispatch(
                    0xaa41,
                    &mut cpu,
                    &mut m,
                    &mut heap,
                    &mut q,
                    &fonts,
                    &mut events,
                    &resources,
                    |_, _, _| Err("unexpected load".into()),
                )?;
                if cpu.dar[15] != 0x700008
                    || cpu.pc != 0x6002
                    || m.read_long(0x700008) != 0x55667788
                {
                    return Err("SetWinColor ABI failed".into());
                }
            }
            if step == 2 {
                windows.visible(&mut m, &mut heap, &q, &fonts, &mut events, w, true)?;
                q.set_port(&mut m, w)?;
            }
            let prior = if step == 2 { w } else { old };
            cpu.pc = 0x6000;
            cpu.dar[15] = 0x700000;
            m.write_long(0x700000, 0x1b000);
            m.write_long(0x700004, w);
            m.write_word(0x700008, 0x7b7b);
            m.write_long(0x70000a, 0x55667788);
            windows.dispatch(
                0xaa42,
                &mut cpu,
                &mut m,
                &mut heap,
                &mut q,
                &fonts,
                &mut events,
                &resources,
                |_, _, _| Err("unexpected load".into()),
            )?;
            if cpu.pc != 0x6002 || cpu.dar[15] != 0x700008 || m.read_long(0x70000a) != 0x55667788 {
                return Err("GetAuxWin ABI failed".into());
            }
            let found = m.read_byte(0x700008);
            let aux = m.read_long(0x1b000);
            let ap = m.read_long(aux);
            let table = m.read_long(ap + 8);
            let tp = m.read_long(table);
            let n = m.read_word(tp + 6);
            cpu.pc = 0x6000;
            cpu.dar[15] = 0x700000;
            m.write_long(0x700000, w);
            windows.dispatch(
                0xa80a,
                &mut cpu,
                &mut m,
                &mut heap,
                &mut q,
                &fonts,
                &mut events,
                &resources,
                |_, _, _| Err("unexpected load".into()),
            )?;
            let variant = m.read_word(0x700004);
            if cpu.pc != 0x6002 || cpu.dar[15] != 0x700004 {
                return Err("GetWVariant ABI failed".into());
            }
            let colors = (0..=n as u32)
                .map(|i| {
                    [
                        m.read_word(tp + 8 + i * 8),
                        m.read_word(tp + 10 + i * 8),
                        m.read_word(tp + 12 + i * 8),
                        m.read_word(tp + 14 + i * 8),
                    ]
                })
                .collect::<Vec<_>>();
            let r = read_rect(&mut m, w + 16);
            let b = q.port_bitmap(&mut m, w)?;
            let b = read_rect(&mut m, b + 6);
            let h = m.read_long(w + 114);
            let p = m.read_long(h);
            let structure = read_rect(&mut m, p + 2);
            let h = m.read_long(w + 118);
            let p = m.read_long(h);
            let content = read_rect(&mut m, p + 2);
            let h = m.read_long(w + 24);
            let p = m.read_long(h);
            let visible = read_rect(&mut m, p + 2);
            let bg = q.port_rgb(&mut m, w, false);
            let fg = q.port_rgb(&mut m, w, true);
            cases.push(json!({"id":id,"step":step,"found":found,"aux_nonnull":(aux!=0) as u8,"owner_same":(m.read_long(ap+4)==w) as u8,"aux_flags":m.read_long(ap+16),"aux_refcon":m.read_long(ap+24) as i32,"aux_reserved_nonnull":(m.read_long(ap+20)!=0) as u8,"table_source_same":(table==source) as u8,"table_size":n,"table_flags":m.read_word(tp+4),"current_unchanged":(q.get_port(&mut m)?==prior) as u8,"visible":(m.read_byte(w+110)!=0) as u8,"variant":variant,"bg":bg,"fg":fg,"table":colors,"port_rect":[r.top,r.left,r.bottom,r.right],"bitmap":[b.top,b.left,b.bottom,b.right],"structure":[structure.top,structure.left,structure.bottom,structure.right],"content":[content.top,content.left,content.bottom,content.right],"visible_region":[visible.top,visible.left,visible.bottom,visible.right]}));
        }
        q.paint_rect(
            &mut m,
            Rect {
                top: 0,
                left: 0,
                bottom: 120,
                right: 260,
            },
            [0; 8],
            8,
        )?;
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_long(0x700000, 0x1b000);
        m.write_word(0x700004, 10);
        m.write_word(0x700006, 10);
        crate::colors::dispatch(0xaa17, &mut cpu, &mut m, &mut heap, &mut q)?;
        cases.push(json!({"id":id,"step":3,"erased_rgb":[m.read_word(0x1b000),m.read_word(0x1b002),m.read_word(0x1b004)]}));
        q.set_port(&mut m, old)?;
        windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
        if source != 0 {
            heap.data(source)
                .map_err(|_| "Window disposal released borrowed color resource")?;
        }
        let mut h = m.read_long(0xcd0);
        for _ in 0..64 {
            if h == 0 {
                break;
            }
            let p = m.read_long(h);
            if m.read_long(p + 4) == w {
                return Err("Disposed window retains auxiliary record".into());
            }
            h = m.read_long(p);
        }
    }
    Ok(json!({"schema":1,"probe":"window-colors-v1","windows":{"cases":cases}}).to_string())
}

pub fn window_background_probe() -> Result<String, String> {
    use crate::{callbacks::Arg, managers::Rect};
    use serde_json::json;
    let mut m = Memory {
        bytes: vec![0; 0x800000],
    };
    let mut heap = crate::managers::Heap::new(0x20000, 0x500000);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.color_capable = true;
    q.init(&mut m, 0x100000)?;
    q.device(&mut m, &mut heap)?;
    q.set_depth(&mut m, &mut heap, 8)
        .map_err(|e| e.to_string())?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let resources =
        crate::resource::parse(include_bytes!("../../probes/windowcolors/resources.rsrc"))?;
    let mut cache = std::collections::BTreeMap::new();
    let mut cpu = CpuCore::new();
    let mut cases = Vec::new();
    fn color_call(
        cpu: &mut CpuCore,
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
        q: &mut crate::quickdraw::QuickDraw,
        op: u16,
        args: &[Arg],
    ) -> Result<(), String> {
        let mut sp = 0x700000;
        for a in args {
            match a {
                Arg::Word(v) => {
                    sp -= 2;
                    m.write_word(sp, *v);
                }
                Arg::Long(v) => {
                    sp -= 4;
                    m.write_long(sp, *v);
                }
            }
        }
        cpu.pc = 0x6000;
        cpu.dar[15] = sp;
        if !crate::colors::dispatch(op, cpu, m, heap, q)?
            || cpu.pc != 0x6002
            || cpu.dar[15] != 0x700000
        {
            return Err("background fixture color ABI failure".into());
        }
        Ok(())
    }
    fn pixel(
        cpu: &mut CpuCore,
        m: &mut Memory,
        heap: &mut crate::managers::Heap,
        q: &mut crate::quickdraw::QuickDraw,
        x: u16,
        y: u16,
    ) -> Result<[u16; 3], String> {
        color_call(
            cpu,
            m,
            heap,
            q,
            0xaa17,
            &[Arg::Word(x), Arg::Word(y), Arg::Long(0x1b020)],
        )?;
        Ok([
            m.read_word(0x1b020),
            m.read_word(0x1b022),
            m.read_word(0x1b024),
        ])
    }
    for (kind, id) in [22000, 22001, 22001, 22002].into_iter().enumerate() {
        let old = q.get_port(&mut m)?;
        let seed = 0x1c000;
        q.open_port(&mut m, &mut heap, seed)?;
        q.make_color(&mut m, &mut heap, seed)?;
        m.write_word(0x1b000, 65535);
        m.write_word(0x1b002, 0);
        m.write_word(0x1b004, 0);
        color_call(
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            0xaa14,
            &[Arg::Long(0x1b000)],
        )?;
        q.paint_rect(
            &mut m,
            Rect {
                top: 70,
                left: 90,
                bottom: 190,
                right: 350,
            },
            [255; 8],
            8,
        )?;
        let seed_rgb = pixel(&mut cpu, &mut m, &mut heap, &mut q, 100, 80)?;
        q.close_port(&mut m, &mut heap, seed)?;
        q.set_port(&mut m, old)?;
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_long(0x700000, u32::MAX);
        m.write_long(0x700004, 0);
        m.write_word(0x700008, id);
        windows.dispatch(
            0xaa46,
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            &fonts,
            &mut events,
            &resources,
            |r, m, heap| load_resource(r, m, heap, &mut cache),
        )?;
        let w = m.read_long(0x70000a);
        q.set_port(&mut m, w)?;
        if kind == 0 || kind >= 2 {
            let rgb = if kind == 0 {
                [0, 0, 65535]
            } else {
                [0, 65535, 0]
            };
            for (i, v) in rgb.into_iter().enumerate() {
                m.write_word(0x1b000 + i as u32 * 2, v);
            }
            color_call(
                &mut cpu,
                &mut m,
                &mut heap,
                &mut q,
                0xaa15,
                &[Arg::Long(0x1b000)],
            )?;
        }
        windows.visible(&mut m, &mut heap, &q, &fonts, &mut events, w, true)?;
        let shown = pixel(&mut cpu, &mut m, &mut heap, &mut q, 10, 10)?;
        let bg = q.port_rgb(&mut m, w, false);
        q.paint_rect(
            &mut m,
            Rect {
                top: 0,
                left: 0,
                bottom: 120,
                right: 260,
            },
            [0; 8],
            8,
        )?;
        let erased = pixel(&mut cpu, &mut m, &mut heap, &mut q, 10, 10)?;
        let r = resources
            .iter()
            .find(|r| r.kind == *b"wctb" && r.id == 22003)
            .unwrap();
        let table = load_resource(r, &mut m, &mut heap, &mut cache)?;
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_long(0x700000, table);
        m.write_long(0x700004, w);
        windows.dispatch(
            0xaa41,
            &mut cpu,
            &mut m,
            &mut heap,
            &mut q,
            &fonts,
            &mut events,
            &resources,
            |_, _, _| Err("unexpected load".into()),
        )?;
        let after = pixel(&mut cpu, &mut m, &mut heap, &mut q, 10, 10)?;
        windows.visible(&mut m, &mut heap, &q, &fonts, &mut events, w, false)?;
        windows.visible(&mut m, &mut heap, &q, &fonts, &mut events, w, true)?;
        let reshown = pixel(&mut cpu, &mut m, &mut heap, &mut q, 10, 10)?;
        cases.push(json!({"kind":kind,"id":id,"seed_rgb":seed_rgb,"shown_rgb":shown,"port_bg":bg,"erased_rgb":erased,"after_set_rgb":after,"reshown_rgb":reshown}));
        q.set_port(&mut m, old)?;
        windows.dispose(&mut m, &mut heap, &q, &mut events, w)?;
    }
    Ok(json!({"schema":1,"probe":"window-background-v1","windows":{"cases":cases}}).to_string())
}

pub fn getctable_probe() -> Result<String, String> {
    use serde_json::json;
    fn api(
        s: &mut ResIOState,
        app: &[Resource<'_>],
        op: u16,
        args: &[(u32, u32)],
        flags: u8,
    ) -> Result<(u32, u32, u16, u32, u32), String> {
        let sp = 0x7e0000;
        let mut n = 0;
        for &(v, w) in args {
            if w == 2 {
                s.m.write_word(sp + n, v as u16)
            } else {
                s.m.write_long(sp + n, v)
            }
            n += w;
        }
        s.m.write_long(sp + n, 0x7b7b7b7b);
        s.m.write_long(sp + n + 4, 0x55667788);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = 0x12345678;
        cpu.set_ccr(flags);
        let handled = s.manager.dispatch(
            op,
            &mut cpu,
            &mut s.m,
            &mut s.heap,
            &mut s.files,
            app,
            &mut s.cache,
            load_resource,
        )?;
        if !handled
            && !resource_enumeration(op, &mut cpu, &mut s.m, &mut s.heap, app, &mut s.cache)?
        {
            let mut q = crate::quickdraw::QuickDraw::new();
            if !crate::quickdraw::device_lifecycle_dispatch(
                op,
                &mut cpu,
                &mut s.m,
                &mut s.heap,
                &mut q,
            )? {
                return Err(format!("GetCTable probe unsupported {op:#x}"));
            }
        }
        assert_eq!(cpu.pc, 0x19002);
        assert_eq!(cpu.dar[15], sp + n);
        Ok((
            s.m.read_long(sp + n),
            cpu.dar[0],
            cpu.get_sr() & 31,
            cpu.dar[15] - sp,
            s.m.read_long(sp + n + 4),
        ))
    }
    fn bytes(s: &mut ResIOState, h: u32) -> Result<String, String> {
        if h == 0 {
            Ok(String::new())
        } else {
            Ok(s.bytes(h)?.iter().map(|b| format!("{b:02x}")).collect())
        }
    }
    fn seed(s: &mut ResIOState, h: u32) -> Result<u32, String> {
        if h == 0 {
            Ok(0)
        } else {
            let p = s.heap.data(h).map_err(|e| e.to_string())?;
            Ok(s.m.read_long(p))
        }
    }
    let fork = crate::resource::serialize(&[crate::resource::Data {
        kind: *b"clut",
        id: 128,
        attributes: 0,
        name: Some(b"Owned color table".to_vec()),
        bytes: vec![
            1, 2, 3, 4, 0, 0, 0, 1, 0, 0, 255, 255, 0x12, 0x34, 0x56, 0x78, 0, 1, 0xab, 0xcd, 0x23,
            0x45, 0x67, 0x89,
        ],
    }])?;
    let resources = crate::resource::parse(&fork)?;
    let mut s = ResIOState::new()?;
    let map_h = s.m.read_long(0xa50);
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    let system = s.m.read_long(map + 16);
    s.heap
        .resize(&mut s.m, map_h, resources[0].map.len() as u32)
        .map_err(|e| e.to_string())?;
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    s.m.put(map as usize, resources[0].map)?;
    s.m.write_long(map + 16, system);
    s.m.write_word(map + 20, 2);
    let entry = map + resources[0].reference_offset as u32 + 8;
    let mut rows = Vec::new();
    for id in [
        -1i16, 0, 1, 2, 3, 4, 8, 16, 32, 33, 34, 36, 40, 64, 128, 129, 256, 1000, 5000,
    ] {
        for flags in [16u8, 31, 0] {
            let (h, d0, ccr, delta, guard) =
                api(&mut s, &resources, 0xaa18, &[(id as u16 as u32, 2)], flags)?;
            let re = s.m.read_word(0xa60) as i16;
            let me = s.m.read_word(0x220) as i16;
            let state = if h != 0 {
                s.heap.state(h).map_err(|e| e.to_string())? as i16
            } else {
                -1
            };
            let size = if h != 0 {
                s.heap.size(h).map_err(|e| e.to_string())?
            } else {
                0
            };
            let (home, he) = if h != 0 {
                let x = api(&mut s, &resources, 0xa9a4, &[(h, 4)], 0)?;
                ((x.0 >> 16) as u16 as i16, s.m.read_word(0xa60) as i16)
            } else {
                (12345, 12345)
            };
            let original_seed = seed(&mut s, h)?;
            let again = api(&mut s, &resources, 0xaa18, &[(id as u16 as u32, 2)], 0)?.0;
            let b = bytes(&mut s, h)?;
            let ab = bytes(&mut s, again)?;
            rows.push(json!({"id":id,"input_ccr":flags,"present":(h!=0) as i32,"d0":d0,"d0_is_handle":(h!=0 && h==d0) as i32,"ccr":ccr,"stack_delta":delta,"guard":guard,"resource_error":re,"memory_error":me,"state":state,"size":size,"home":home,"home_error":he,"repeat_same":(h==again) as i32,"repeat_bytes_equal":(h!=0 && again!=0 && b==ab) as i32,"repeat_seed_delta":seed(&mut s,again)?.wrapping_sub(original_seed),"bytes":b,"repeat_bytes":ab}));
            if again != 0 && again != h {
                api(&mut s, &resources, 0xaa24, &[(again, 4)], 0)?;
            }
            if h != 0 {
                api(&mut s, &resources, 0xaa24, &[(h, 4)], 0)?;
            }
        }
    }
    let mut ownership = Vec::new();
    for flags in [16u8, 31, 0] {
        let source = api(
            &mut s,
            &resources,
            0xa81f,
            &[(128, 2), (u32::from_be_bytes(*b"clut"), 4)],
            0,
        )?
        .0;
        let p = s.heap.data(source).map_err(|e| e.to_string())?;
        s.m.write_byte(p + 10, 0x55);
        let before = s.m.read_long(entry) == source;
        let h = api(&mut s, &resources, 0xaa18, &[(128, 2)], flags)?.0;
        let zero = s.m.read_long(entry) == 0;
        let source_state = s.heap.state(source).map_err(|e| e.to_string())?;
        let again = api(
            &mut s,
            &resources,
            0xa81f,
            &[(128, 2), (u32::from_be_bytes(*b"clut"), 4)],
            0,
        )?
        .0;
        ownership.push(json!({"input_ccr":flags,"entry_before":before as i32,"entry_zero_after":zero as i32,"same_as_source":(h==source) as i32,"source_state":source_state,"result_state":s.heap.state(h).map_err(|e|e.to_string())?,"reload_same":(again==source) as i32,"result_bytes":bytes(&mut s,h)?,"reload_bytes":bytes(&mut s,again)?}));
        if h != again {
            api(&mut s, &resources, 0xaa24, &[(h, 4)], 0)?;
        }
        api(&mut s, &resources, 0xa9a3, &[(again, 4)], 0)?;
    }
    Ok(serde_json::to_string_pretty(
        &json!({"schema":1,"probe":"get-color-table-raw-v1","records":rows,"ownership":ownership}),
    )
    .unwrap())
}

pub fn courier_probe() -> Result<String, String> {
    use serde_json::json;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut m = Memory::zeroed(0x10000);
    let sp = 0xf000;
    let buffer = 0x1000;
    let mut cpu = CpuCore::new();
    m.write_long(sp, buffer);
    m.write_word(sp + 4, 22);
    cpu.pc = 0x8000;
    cpu.dar[15] = sp;
    assert!(fonts.dispatch(0xa8ff, &mut cpu, &mut m));
    assert_eq!(cpu.dar[15], sp + 6);
    assert_eq!(cpu.pc, 0x8002);
    let name: Vec<_> = (0..m.read_byte(buffer) as u32)
        .map(|i| m.read_byte(buffer + 1 + i))
        .collect();
    m.write_byte(buffer, 7);
    m.put(buffer as usize + 1, b"Courier")?;
    m.write_long(sp, buffer + 256);
    m.write_long(sp + 4, buffer);
    cpu.pc = 0x8000;
    cpu.dar[15] = sp;
    assert!(fonts.dispatch(0xa900, &mut cpu, &mut m));
    assert_eq!(cpu.dar[15], sp + 8);
    assert_eq!(cpu.pc, 0x8002);
    let number = m.read_word(buffer + 256) as i16;
    let mut rows = Vec::new();
    for size in [0i16, 9, 10, 12, 14, 18, 24] {
        for face in [0u8, 1] {
            m.write_word(sp, size as u16);
            m.write_word(sp + 2, 22);
            m.write_word(sp + 4, 0xa5a5);
            cpu.pc = 0x8000;
            cpu.dar[15] = sp;
            assert!(fonts.dispatch(0xa902, &mut cpu, &mut m));
            assert_eq!(cpu.dar[15], sp + 4);
            assert_eq!(cpu.pc, 0x8002);
            let real = m.read_byte(sp + 4);
            let selected = fonts.select(22, size, face)?;
            let widths: Result<Vec<_>, _> = (0..256)
                .map(|c| fonts.text_width(22, size, face, &[c as u8]))
                .collect();
            let labels: Result<Vec<_>, _> = [
                b"Marathon".as_slice(),
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                &[65, 0, 128, 255, 32, 48, 97, 13],
            ]
            .iter()
            .map(|text| fonts.text_width(22, size, face, text))
            .collect();
            rows.push(json!({"font":22,"size":size,"face":face,"real":real,"info":selected.info,"widths":widths?,"labels":labels?}));
        }
    }
    Ok(
        json!({"schema":1,"probe":"courier-font-raw-v1","name":name,"number":number,"fonts":rows})
            .to_string(),
    )
}

pub fn ctseed_probe() -> Result<String, String> {
    use serde_json::json;
    fn api(
        s: &mut ResIOState,
        q: &mut crate::quickdraw::QuickDraw,
        app: &[Resource<'_>],
        op: u16,
        args: &[(u32, u32)],
        seed: u32,
        flags: u8,
    ) -> Result<(u32, u32, u16, i64, u32), String> {
        let sp = 0x7e0000;
        let mut n = 0;
        for &(v, w) in args {
            if w == 2 {
                s.m.write_word(sp + n, v as u16)
            } else {
                s.m.write_long(sp + n, v)
            }
            n += w;
        }
        s.m.write_long(sp + n, 0x7b7b7b7b);
        s.m.write_long(sp + n + 4, 0x55667788);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = seed;
        cpu.set_ccr(flags);
        let handled = s.manager.dispatch(
            op,
            &mut cpu,
            &mut s.m,
            &mut s.heap,
            &mut s.files,
            app,
            &mut s.cache,
            load_resource,
        )?;
        if !handled
            && !crate::quickdraw::device_lifecycle_dispatch(op, &mut cpu, &mut s.m, &mut s.heap, q)?
            && !crate::colors::dispatch(op, &mut cpu, &mut s.m, &mut s.heap, q)?
        {
            return Err(format!("color seed probe unsupported{op:#x}"));
        }
        assert_eq!(cpu.pc, 0x19002);
        assert_eq!(cpu.dar[15], sp + n);
        Ok((
            s.m.read_long(sp + n),
            cpu.dar[0],
            cpu.get_sr() & 31,
            cpu.dar[15] as i64 - (sp + n) as i64,
            s.m.read_long(sp + n + 4),
        ))
    }
    let fork = crate::resource::serialize(&[crate::resource::Data {
        kind: *b"clut",
        id: 128,
        attributes: 0,
        name: Some(b"Owned colors".to_vec()),
        bytes: vec![
            0x10, 0x20, 0x30, 0x40, 0, 0, 0, 1, 0, 0, 255, 255, 0x12, 0x34, 0x56, 0x78, 0, 1, 0xab,
            0xcd, 0x23, 0x45, 0x67, 0x89,
        ],
    }])?;
    let resources = crate::resource::parse(&fork)?;
    let mut s = ResIOState::new()?;
    let map_h = s.m.read_long(0xa50);
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    let system = s.m.read_long(map + 16);
    s.heap
        .resize(&mut s.m, map_h, resources[0].map.len() as u32)
        .map_err(|e| e.to_string())?;
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    s.m.put(map as usize, resources[0].map)?;
    s.m.write_long(map + 16, system);
    s.m.write_word(map + 20, 2);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut s.m, 0x100000)?;
    q.color_capable = true;
    q.device(&mut s.m, &mut s.heap)?;
    q.set_depth(&mut s.m, &mut s.heap, 8)
        .map_err(|e| e.to_string())?;
    s.manager = crate::resfiles::ResFiles::with_color_seeds(q.color_seeds());
    let mut rows = Vec::new();
    for (seed_index, seed) in [0x12345678u32, 0].into_iter().enumerate() {
        for (k, flags) in [16u8, 31, 0].into_iter().enumerate() {
            let base = api(&mut s, &mut q, &resources, 0xaa28, &[], 0, 0)?.0;
            let (result, d0, ccr, delta, guard) =
                api(&mut s, &mut q, &resources, 0xaa28, &[], seed, flags)?;
            let a = api(&mut s, &mut q, &resources, 0xaa18, &[(128, 2)], 0, 0)?.0;
            let ap = s.heap.data(a).map_err(|e| e.to_string())?;
            let application = s.m.read_long(ap);
            let b = api(&mut s, &mut q, &resources, 0xaa18, &[(8, 2)], 0, 0)?.0;
            let bp = s.heap.data(b).map_err(|e| e.to_string())?;
            let builtin = s.m.read_long(bp);
            let after_builtin = api(&mut s, &mut q, &resources, 0xaa28, &[], 0, 0)?.0;
            let bitmap = q.device_bitmap(&mut s.m)?;
            let dh = s.m.read_long(bitmap + 42);
            let dp = s.heap.data(dh).map_err(|e| e.to_string())?;
            let device_before = s.m.read_long(dp);
            let color = 0x110000;
            s.m.write_word(color, 16);
            s.m.write_word(color + 2, 0x1234 + k as u16);
            s.m.write_word(color + 4, 0x5678 + seed_index as u16);
            s.m.write_word(color + 6, 0x9abc);
            api(
                &mut s,
                &mut q,
                &resources,
                0xaa3f,
                &[(color, 4), (0, 2), (16, 2)],
                0,
                0,
            )?;
            let device_after = s.m.read_long(dp);
            let after_entries = api(&mut s, &mut q, &resources, 0xaa28, &[], 0, 0)?.0;
            let rgb: Vec<_> = (0..3)
                .map(|i| s.m.read_word(dp + 10 + 16 * 8 + 2 * i))
                .collect();
            rows.push(json!({"input_d0":seed,"input_ccr":flags,"base":base,"result":result,"d0":d0,"ccr":ccr,"stack_delta":delta,"guard":guard,"application_seed":application,"builtin_seed":builtin,"after_builtin":after_builtin,"device_before":device_before,"device_after":device_after,"after_entries":after_entries,"device_rgb":rgb}));
            api(&mut s, &mut q, &resources, 0xaa24, &[(a, 4)], 0, 0)?;
            api(&mut s, &mut q, &resources, 0xaa24, &[(b, 4)], 0, 0)?;
        }
    }
    Ok(serde_json::to_string_pretty(
        &json!({"schema":1,"probe":"color-seed-raw-v1","records":rows}),
    )
    .unwrap())
}

pub fn ctabchanged_probe() -> Result<String, String> {
    use serde_json::json;
    fn api(
        s: &mut ResIOState,
        q: &mut crate::quickdraw::QuickDraw,
        app: &[Resource<'_>],
        op: u16,
        args: &[(u32, u32)],
        seed: u32,
        flags: u8,
    ) -> Result<(u32, u32, u16, i64, u32), String> {
        let sp = 0x7e0000;
        let mut n = 0;
        for &(v, w) in args {
            if w == 2 {
                s.m.write_word(sp + n, v as u16)
            } else {
                s.m.write_long(sp + n, v)
            }
            n += w;
        }
        s.m.write_long(sp + n, 0x7b7b7b7b);
        s.m.write_long(sp + n + 4, 0x55667788);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        cpu.dar[0] = seed;
        cpu.set_ccr(flags);
        let handled = s.manager.dispatch(
            op,
            &mut cpu,
            &mut s.m,
            &mut s.heap,
            &mut s.files,
            app,
            &mut s.cache,
            load_resource,
        )?;
        if !handled
            && !crate::quickdraw::device_lifecycle_dispatch(op, &mut cpu, &mut s.m, &mut s.heap, q)?
            && !crate::colors::dispatch(op, &mut cpu, &mut s.m, &mut s.heap, q)?
        {
            return Err(format!("color seed probe unsupported{op:#x}"));
        }
        assert_eq!(cpu.pc, 0x19002);
        assert_eq!(cpu.dar[15], sp + n);
        Ok((
            s.m.read_long(sp + n),
            cpu.dar[0],
            cpu.get_sr() & 31,
            cpu.dar[15] as i64 - (sp + n) as i64,
            s.m.read_long(sp + n + 4),
        ))
    }
    let fork = crate::resource::serialize(&[crate::resource::Data {
        kind: *b"clut",
        id: 128,
        attributes: 0,
        name: Some(b"Owned colors".to_vec()),
        bytes: vec![
            0x10, 0x20, 0x30, 0x40, 0, 0, 0, 1, 0, 0, 255, 255, 0x12, 0x34, 0x56, 0x78, 0, 1, 0xab,
            0xcd, 0x23, 0x45, 0x67, 0x89,
        ],
    }])?;
    let resources = crate::resource::parse(&fork)?;
    let mut s = ResIOState::new()?;
    let map_h = s.m.read_long(0xa50);
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    let system = s.m.read_long(map + 16);
    s.heap
        .resize(&mut s.m, map_h, resources[0].map.len() as u32)
        .map_err(|e| e.to_string())?;
    let map = s.heap.data(map_h).map_err(|e| e.to_string())?;
    s.m.put(map as usize, resources[0].map)?;
    s.m.write_long(map + 16, system);
    s.m.write_word(map + 20, 2);
    let mut q = crate::quickdraw::QuickDraw::new();
    q.init(&mut s.m, 0x100000)?;
    q.color_capable = true;
    q.device(&mut s.m, &mut s.heap)?;
    q.set_depth(&mut s.m, &mut s.heap, 8)
        .map_err(|e| e.to_string())?;
    s.manager = crate::resfiles::ResFiles::with_color_seeds(q.color_seeds());

    fn hex(s: &mut ResIOState, h: u32) -> Result<String, String> {
        Ok(s.bytes(h)?.iter().map(|b| format!("{b:02x}")).collect())
    }
    let mut rows = Vec::new();
    for which in 0..2 {
        for flags in [16u8, 31, 0] {
            let h = api(
                &mut s,
                &mut q,
                &resources,
                0xaa18,
                &[(if which == 0 { 128 } else { 8 }, 2)],
                0,
                0,
            )?
            .0;
            let size = s.heap.size(h).map_err(|e| e.to_string())?;
            let before_state = s.heap.state(h).map_err(|e| e.to_string())?;
            let p = s.heap.data(h).map_err(|e| e.to_string())?;
            s.m.write_long(p, 0x10203040);
            let base = api(&mut s, &mut q, &resources, 0xaa28, &[], 0, 0)?.0;
            let before_hex = hex(&mut s, h)?;
            s.m.write_word(0x220, -123i16 as u16);
            s.m.write_word(0xa60, -192i16 as u16);
            let (_, d0, ccr, delta, _) = api(
                &mut s,
                &mut q,
                &resources,
                0xab1d,
                &[(h, 4)],
                0x00040007,
                flags,
            )?;
            let mem_error = s.m.read_word(0x220) as i16;
            let res_error = s.m.read_word(0xa60) as i16;
            let after = api(&mut s, &mut q, &resources, 0xaa28, &[], 0, 0)?.0;
            let after_state = s.heap.state(h).map_err(|e| e.to_string())?;
            let after_hex = hex(&mut s, h)?;
            rows.push(json!({"which":which,"input_ccr":flags,"size":size,"before_state":before_state,"base":base,"before_hex":before_hex,"d0":d0,"ccr":ccr,"stack_delta":delta,"after":after,"after_state":after_state,"memory_error":mem_error,"resource_error":res_error,"after_hex":after_hex}));
            api(&mut s, &mut q, &resources, 0xaa24, &[(h, 4)], 0, 0)?;
        }
    }
    Ok(serde_json::to_string_pretty(
        &json!({"schema":1,"probe":"ctab-changed-raw-v1","records":rows}),
    )
    .unwrap())
}
