//! Vertical retrace queues driven by injected clock/pulse events, not a host OS.
use crate::MemoryBus;
use alloc::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    vec::Vec,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Queue {
    System,
    Slot(u16),
}
struct Pulse {
    nodes: VecDeque<(u32, Queue, u64)>,
}
#[derive(Clone, Copy)]
struct Entry {
    queue: Queue,
    generation: u64,
}
pub struct Retrace {
    tasks: BTreeMap<u32, Entry>,
    queues: BTreeMap<Queue, Vec<u32>>,
    bound: BTreeSet<u16>,
    registered: BTreeSet<u16>,
    tick: u64,
    pulse: Option<Pulse>,
    current: Option<(u32, u64)>,
    generation: u64,
    last_borrow: bool,
    limit: u32,
}
impl Retrace {
    pub fn new(limit: u32) -> Self {
        Self {
            tasks: BTreeMap::new(),
            queues: BTreeMap::new(),
            bound: BTreeSet::new(),
            registered: BTreeSet::new(),
            tick: 0,
            pulse: None,
            current: None,
            generation: 0,
            last_borrow: false,
            limit,
        }
    }
    pub fn valid_queue(&self, queue: Queue) -> bool {
        match queue {
            Queue::System => true,
            Queue::Slot(slot) => slot < 15 || self.registered.contains(&slot),
        }
    }
    pub fn bind_slot(&mut self, slot: u16) -> Result<(), i16> {
        if !self.valid_queue(Queue::Slot(slot)) {
            return Err(-337);
        }
        self.bound.insert(slot);
        Ok(())
    }
    /// Hardware/driver configuration, separate from a guest queue request.
    pub fn register_slot(&mut self, slot: u16) -> Result<(), i16> {
        if !(32..=255).contains(&slot) {
            return Err(-337);
        }
        self.registered.insert(slot);
        Ok(())
    }
    pub fn last_pulse_borrow(&self) -> bool {
        self.last_borrow
    }
    fn linked<B: MemoryBus>(&self, m: &mut B, q: Queue) {
        let list = self.queues.get(&q).map(Vec::as_slice).unwrap_or(&[]);
        for (i, &p) in list.iter().enumerate() {
            m.write_long(p, list.get(i + 1).copied().unwrap_or(0));
        }
        if q == Queue::System {
            m.write_word(0x160, 0);
            m.write_long(0x162, list.first().copied().unwrap_or(0));
            m.write_long(0x166, list.last().copied().unwrap_or(0));
        }
    }
    pub fn install<B: MemoryBus>(&mut self, m: &mut B, p: u32, q: Queue) -> Result<(), i16> {
        if !self.valid_queue(q) {
            return Err(-337);
        }
        if p < 0x100 || p & 1 != 0 || p.checked_add(14).is_none_or(|end| end > self.limit) {
            return Err(-50);
        }
        if m.read_word(p + 4) != 1 {
            return Err(-2);
        }
        if self.tasks.contains_key(&p) {
            return Err(-1);
        }
        if m.read_long(p + 6) == 0 {
            return Err(-50);
        }
        self.generation = self.generation.checked_add(1).ok_or(-108i16)?;
        let count = m.read_word(p + 10).wrapping_add(m.read_word(p + 12));
        m.write_word(p + 10, count);
        self.tasks.insert(
            p,
            Entry {
                queue: q,
                generation: self.generation,
            },
        );
        self.queues.entry(q).or_default().push(p);
        self.linked(m, q);
        Ok(())
    }
    pub fn remove<B: MemoryBus>(&mut self, m: &mut B, p: u32, q: Queue) -> Result<(), i16> {
        if !self.valid_queue(q) {
            return Err(-337);
        }
        if !self.tasks.get(&p).is_some_and(|e| e.queue == q) {
            return Err(-1);
        }
        self.tasks.remove(&p);
        self.queues.get_mut(&q).unwrap().retain(|&node| node != p);
        m.write_long(p, 0);
        self.linked(m, q);
        Ok(())
    }
    pub fn begin_pulse(&mut self, q: Queue) -> Result<(), i16> {
        if !self.valid_queue(q) {
            return Err(-337);
        }
        if self.pulse.is_some() || self.current.is_some() {
            return Err(-1);
        }
        let nodes = self
            .queues
            .get(&q)
            .into_iter()
            .flatten()
            .map(|&p| (p, q, self.tasks[&p].generation))
            .collect();
        self.pulse = Some(Pulse { nodes });
        self.last_borrow = false;
        Ok(())
    }
    pub fn pulse_pending(&self) -> bool {
        self.pulse.is_some() || self.current.is_some()
    }
    /// Continue one pulse until a real callback is needed. The measured OS9
    /// implementation decrements zero through negative, including DoVBLTask.
    pub fn next_pulse_callback<B: MemoryBus>(&mut self, m: &mut B) -> Option<(u32, u32)> {
        if self.current.is_some() {
            return None;
        }
        loop {
            let next = self.pulse.as_mut()?.nodes.pop_front();
            let Some((p, q, generation)) = next else {
                self.pulse = None;
                return None;
            };
            if !self
                .tasks
                .get(&p)
                .is_some_and(|e| e.queue == q && e.generation == generation)
            {
                continue;
            }
            let count = m.read_word(p + 10);
            self.last_borrow = count == 0;
            let count = count.wrapping_sub(1);
            m.write_word(p + 10, count);
            if count == 0 {
                self.current = Some((p, generation));
                return Some((p, m.read_long(p + 6)));
            }
        }
    }
    pub fn finish_callback<B: MemoryBus>(&mut self, m: &mut B) -> Result<(), i16> {
        let (p, generation) = self.current.take().ok_or(-1i16)?;
        if let Some(&entry) = self.tasks.get(&p) {
            if entry.generation == generation && m.read_word(p + 10) == 0 {
                self.remove(m, p, entry.queue)?;
            }
        }
        Ok(())
    }
    fn active(&self) -> Vec<(u32, Queue, u64)> {
        self.queues
            .iter()
            .filter(|(q, _)| match q {
                Queue::System => true,
                Queue::Slot(slot) => self.bound.contains(slot),
            })
            .flat_map(|(&q, nodes)| {
                nodes
                    .iter()
                    .map(move |&p| (p, q, self.tasks[&p].generation))
            })
            .collect()
    }
    /// Deliver virtual system ticks and explicitly bound display clocks. Long
    /// idle gaps use arithmetic skipping up to the next counter expiration.
    pub fn next_time_callback<B: MemoryBus>(
        &mut self,
        m: &mut B,
        target: u64,
    ) -> Option<(u32, u32)> {
        if self.current.is_some() {
            return None;
        }
        if self.pulse.is_some() {
            if let Some(task) = self.next_pulse_callback(m) {
                return Some(task);
            }
        }
        while self.tick < target {
            let active = self.active();
            let distance = active
                .iter()
                .map(|&(p, _, _)| {
                    let n = m.read_word(p + 10);
                    if n == 0 {
                        65536u64
                    } else {
                        n as u64
                    }
                })
                .min();
            let Some(distance) = distance else {
                self.tick = target;
                return None;
            };
            let skip = (target - self.tick).min(distance - 1);
            if skip != 0 {
                for &(p, _, _) in &active {
                    let n = m.read_word(p + 10);
                    self.last_borrow = n.wrapping_sub((skip - 1) as u16) == 0;
                    m.write_word(p + 10, n.wrapping_sub(skip as u16));
                }
                self.tick += skip;
            }
            if self.tick == target {
                return None;
            }
            self.tick += 1;
            self.pulse = Some(Pulse {
                nodes: active.into_iter().collect(),
            });
            self.last_borrow = false;
            if let Some(task) = self.next_pulse_callback(m) {
                return Some(task);
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct M(Vec<u8>);
    impl MemoryBus for M {
        fn read_byte(&mut self, p: u32) -> u8 {
            self.0[p as usize]
        }
        fn write_byte(&mut self, p: u32, v: u8) {
            self.0[p as usize] = v;
        }
    }
    fn task(m: &mut M, p: u32, count: i16, phase: i16) {
        m.write_word(p + 4, 1);
        m.write_long(p + 6, 0x19000);
        m.write_word(p + 10, count as u16);
        m.write_word(p + 12, phase as u16);
        m.write_long(p + 14, 0x55667788);
    }
    #[test]
    fn phase_is_added_once_and_rearm_uses_written_counter() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        task(&mut m, 0x2000, 3, 2);
        r.install(&mut m, 0x2000, Queue::System).unwrap();
        assert_eq!(m.read_word(0x200a), 5);
        assert_eq!(r.next_time_callback(&mut m, 4), None);
        assert_eq!(r.next_time_callback(&mut m, 5), Some((0x2000, 0x19000)));
        m.write_word(0x200a, 3);
        r.finish_callback(&mut m).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 8), Some((0x2000, 0x19000)));
        r.finish_callback(&mut m).unwrap();
        assert_eq!(r.remove(&mut m, 0x2000, Queue::System), Err(-1));
        assert_eq!(m.read_long(0x200e), 0x55667788);
    }
    #[test]
    fn unbound_slots_wait_for_explicit_pulses_and_zero_decrements_when_pulsed() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        task(&mut m, 0x2000, 0, 0);
        r.install(&mut m, 0x2000, Queue::Slot(0)).unwrap();
        r.next_time_callback(&mut m, 1000);
        assert_eq!(m.read_word(0x200a), 0);
        r.begin_pulse(Queue::Slot(0)).unwrap();
        assert_eq!(r.next_pulse_callback(&mut m), None);
        assert_eq!(m.read_word(0x200a) as i16, -1);
        assert!(r.last_pulse_borrow());
        m.write_word(0x200a, 1);
        r.begin_pulse(Queue::Slot(0)).unwrap();
        assert_eq!(r.next_pulse_callback(&mut m), Some((0x2000, 0x19000)));
        r.finish_callback(&mut m).unwrap();
        assert_eq!(m.read_long(0x2000), 0);
    }
    #[test]
    fn system_zero_decrements_and_long_idle_gaps_do_not_loop_per_tick() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        task(&mut m, 0x2000, 0, 0);
        r.install(&mut m, 0x2000, Queue::System).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 30), None);
        assert_eq!(m.read_word(0x200a) as i16, -30);
        assert_eq!(
            r.next_time_callback(&mut m, 1_000_000),
            Some((0x2000, 0x19000))
        );
        r.finish_callback(&mut m).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 1_000_000), None);
    }
    #[test]
    fn insertion_order_and_removal_during_callback_keep_queue_consistent() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        for p in [0x2100, 0x2000] {
            task(&mut m, p, 1, 0);
            r.install(&mut m, p, Queue::System).unwrap();
        }
        assert_eq!(m.read_long(0x162), 0x2100);
        assert_eq!(m.read_long(0x2100), 0x2000);
        assert_eq!(r.next_time_callback(&mut m, 1), Some((0x2100, 0x19000)));
        r.remove(&mut m, 0x2000, Queue::System).unwrap();
        r.finish_callback(&mut m).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 1), None);
        assert_eq!(m.read_long(0x162), 0);
        assert_eq!(m.read_long(0x166), 0);
    }
    #[test]
    fn binding_starts_at_current_clock_without_retroactive_slot_ticks() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        task(&mut m, 0x2000, 3, 0);
        r.install(&mut m, 0x2000, Queue::Slot(0)).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 100), None);
        r.bind_slot(0).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 102), None);
        assert_eq!(m.read_word(0x200a), 1);
        assert!(r.next_time_callback(&mut m, 103).is_some());
    }
    #[test]
    fn reinstallation_does_not_reuse_an_old_pulse_or_callback_continuation() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        for p in [0x2000, 0x2100] {
            task(&mut m, p, 1, 0);
            r.install(&mut m, p, Queue::System).unwrap();
        }
        assert_eq!(r.next_time_callback(&mut m, 1), Some((0x2000, 0x19000)));
        r.remove(&mut m, 0x2100, Queue::System).unwrap();
        task(&mut m, 0x2100, 2, 0);
        r.install(&mut m, 0x2100, Queue::System).unwrap();
        r.remove(&mut m, 0x2000, Queue::System).unwrap();
        task(&mut m, 0x2000, 0, 0);
        r.install(&mut m, 0x2000, Queue::System).unwrap();
        r.finish_callback(&mut m).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 1), None);
        assert_eq!(m.read_word(0x210a), 2);
        assert_eq!(r.next_time_callback(&mut m, 2), None);
        assert_eq!(m.read_word(0x200a) as i16, -1);
        assert_eq!(r.next_time_callback(&mut m, 3), Some((0x2100, 0x19000)));
    }
    #[test]
    fn modern_slots_require_a_driver_registration_and_an_explicit_clock_binding() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        task(&mut m, 0x2000, 3, 0);
        assert_eq!(r.install(&mut m, 0x2000, Queue::Slot(32)), Err(-337));
        r.register_slot(32).unwrap();
        r.install(&mut m, 0x2000, Queue::Slot(32)).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 30), None);
        assert_eq!(m.read_word(0x200a), 3);
        r.bind_slot(32).unwrap();
        assert_eq!(r.next_time_callback(&mut m, 32), None);
        assert_eq!(r.next_time_callback(&mut m, 33), Some((0x2000, 0x19000)));
    }
    #[test]
    fn invalid_records_do_not_mutate_guards() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut r = Retrace::new(0x10000);
        task(&mut m, 0x2000, 3, 0);
        assert_eq!(r.install(&mut m, 0x2000, Queue::Slot(15)), Err(-337));
        assert_eq!(r.install(&mut m, 1, Queue::System), Err(-50));
        assert_eq!(r.install(&mut m, 0xfff4, Queue::System), Err(-50));
        m.write_word(0x2004, 0);
        assert_eq!(r.install(&mut m, 0x2000, Queue::System), Err(-2));
        assert_eq!(m.read_word(0x200a), 3);
        assert_eq!(m.read_long(0x200e), 0x55667788);
    }
}
