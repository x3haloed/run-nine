//! Clock-injected Time Manager state. No host OS, wall clock or CPU execution here.
use crate::MemoryBus;
use alloc::{collections::BTreeMap, vec::Vec};
struct Task {
    extended: bool,
    installed: bool,
    deadline: Option<u64>,
    last_expiry: Option<u64>,
    paused: Option<u64>,
    order: u64,
}
pub struct Timers {
    tasks: BTreeMap<u32, Task>,
    next: u64,
    limit: u32,
}
impl Timers {
    pub fn new(limit: u32) -> Self {
        Self {
            tasks: BTreeMap::new(),
            next: 0,
            limit,
        }
    }
    fn valid(&self, p: u32, extended: bool) -> Result<(), i16> {
        if p < 0x100
            || p & 1 != 0
            || p.checked_add(if extended { 22 } else { 14 })
                .is_none_or(|end| end > self.limit)
        {
            Err(-50)
        } else {
            Ok(())
        }
    }
    fn kind(task: &Task) -> u16 {
        0x2000 | if task.extended { 0x4000 } else { 0 }
    }
    fn links<B: MemoryBus>(&self, m: &mut B) {
        let mut active = self
            .tasks
            .iter()
            .filter_map(|(&p, t)| {
                if t.installed {
                    t.deadline.map(|d| (d, t.order, p))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        active.sort_unstable();
        for (&p, t) in &self.tasks {
            if t.installed {
                m.write_long(p, 0);
            }
        }
        for pair in active.windows(2) {
            m.write_long(pair[0].2, pair[1].2);
        }
    }
    pub fn install<B: MemoryBus>(&mut self, m: &mut B, p: u32, extended: bool) -> Result<(), i16> {
        self.valid(p, extended)?;
        if self.tasks.get(&p).is_some_and(|t| t.installed) {
            return Err(-50);
        }
        let paused = if extended && (m.read_long(p + 14) != 0 || m.read_long(p + 10) != 0) {
            self.tasks.get(&p).and_then(|t| t.paused)
        } else {
            None
        };
        self.next = self.next.checked_add(1).ok_or(-108i16)?;
        self.tasks.insert(
            p,
            Task {
                extended,
                installed: true,
                deadline: None,
                last_expiry: None,
                paused,
                order: self.next,
            },
        );
        m.write_long(p, 0);
        m.write_word(p + 4, if extended { 0x6000 } else { 0x2000 });
        Ok(())
    }
    pub fn prime<B: MemoryBus>(
        &mut self,
        m: &mut B,
        p: u32,
        request: i32,
        now: u64,
    ) -> Result<(), i16> {
        let task = self
            .tasks
            .get_mut(&p)
            .filter(|t| t.installed)
            .ok_or(-50i16)?;
        let duration = if request < 0 {
            -(request as i64) as u64
        } else {
            request as u64 * 1000
        };
        let deadline = if request == 0 && task.paused.is_some() {
            now.checked_add(task.paused.take().unwrap())
        } else {
            let base = if task.extended && task.deadline.is_none() {
                task.last_expiry.unwrap_or(now)
            } else {
                now
            };
            task.paused = None;
            base.checked_add(duration)
        }
        .ok_or(-50i16)?;
        task.deadline = Some(deadline);
        m.write_word(p + 4, Self::kind(task) | 0x8000);
        m.write_long(p + 10, request as u32);
        if task.extended {
            m.write_long(p + 14, deadline as u32);
        }
        self.links(m);
        Ok(())
    }
    pub fn remove<B: MemoryBus>(&mut self, m: &mut B, p: u32, now: u64) -> Result<(), i16> {
        let task = self
            .tasks
            .get_mut(&p)
            .filter(|t| t.installed)
            .ok_or(-50i16)?;
        let remaining = task.deadline.map(|d| d.saturating_sub(now)).unwrap_or(0);
        let count = if remaining <= i32::MAX as u64 + 1 {
            -(remaining as i64) as i32
        } else {
            (remaining / 1000).min(i32::MAX as u64) as i32
        };
        task.paused = Some(remaining);
        task.deadline = None;
        task.installed = false;
        task.last_expiry = None;
        m.write_long(p, 0);
        m.write_word(p + 4, 0);
        m.write_long(p + 10, count as u32);
        self.links(m);
        Ok(())
    }
    /// Expire one due task before its code executes. A0 receives the routine,
    /// A1 the task in the CPU adapter. Inactive tasks keep their request count.
    pub fn due<B: MemoryBus>(&mut self, m: &mut B, now: u64) -> Option<(u32, u32)> {
        let (_, _, p) = self
            .tasks
            .iter()
            .filter_map(|(&p, t)| {
                if t.installed {
                    t.deadline.filter(|d| *d <= now).map(|d| (d, t.order, p))
                } else {
                    None
                }
            })
            .min()?;
        let task = self.tasks.get_mut(&p).unwrap();
        task.last_expiry = task.deadline.take();
        m.write_word(p + 4, Self::kind(task));
        m.write_long(p, 0);
        let procedure = m.read_long(p + 6);
        self.links(m);
        Some((p, procedure))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Memory(Vec<u8>);
    impl MemoryBus for Memory {
        fn read_byte(&mut self, p: u32) -> u8 {
            self.0[p as usize]
        }
        fn write_byte(&mut self, p: u32, v: u8) {
            self.0[p as usize] = v;
        }
    }
    fn scene() -> (Timers, Memory) {
        (Timers::new(0x10000), Memory(alloc::vec![0;0x10000]))
    }
    #[test]
    fn install_is_not_activation_and_negative_counts_are_microseconds() {
        let (mut t, mut m) = scene();
        t.install(&mut m, 0x1000, false).unwrap();
        assert_eq!(t.due(&mut m, u64::MAX), None);
        t.prime(&mut m, 0x1000, -30000, 100).unwrap();
        assert_eq!(t.due(&mut m, 30099), None);
        assert_eq!(t.due(&mut m, 30100), Some((0x1000, 0)));
        assert_eq!(m.read_long(0x100a) as i32, -30000);
    }
    #[test]
    fn cancellation_reports_unused_time_and_keeps_original_record_guards() {
        let (mut t, mut m) = scene();
        m.write_long(0x100e, 0x55555555);
        m.write_long(0x1012, 0x66666666);
        t.install(&mut m, 0x1000, false).unwrap();
        t.prime(&mut m, 0x1000, 10000, 100).unwrap();
        t.remove(&mut m, 0x1000, 350).unwrap();
        assert_eq!(m.read_long(0x100a) as i32, -9999750);
        assert_eq!(m.read_word(0x1004), 0);
        assert_eq!(m.read_long(0x100e), 0x55555555);
        assert_eq!(m.read_long(0x1012), 0x66666666);
        assert_eq!(t.due(&mut m, u64::MAX), None);
    }
    #[test]
    fn extended_rearming_anchors_to_expiry_and_pause_resume_freezes_remaining() {
        let (mut t, mut m) = scene();
        t.install(&mut m, 0x1000, true).unwrap();
        t.prime(&mut m, 0x1000, 30, 0).unwrap();
        assert!(t.due(&mut m, 90000).is_some());
        t.prime(&mut m, 0x1000, 30, 90000).unwrap();
        assert!(t.due(&mut m, 90000).is_some());
        t.prime(&mut m, 0x1000, 30, 90000).unwrap();
        assert!(t.due(&mut m, 90000).is_some());
        t.prime(&mut m, 0x1000, 30, 90000).unwrap();
        t.remove(&mut m, 0x1000, 100000).unwrap();
        assert_eq!(m.read_long(0x100a) as i32, -20000);
        t.install(&mut m, 0x1000, true).unwrap();
        t.prime(&mut m, 0x1000, 0, 500000).unwrap();
        assert_eq!(t.due(&mut m, 519999), None);
        assert!(t.due(&mut m, 520000).is_some());
    }
    #[test]
    fn pause_resume_survives_virtual_wake_word_wrap() {
        let (mut t, mut m) = scene();
        let end = 1u64 << 32;
        t.install(&mut m, 0x1000, true).unwrap();
        t.prime(&mut m, 0x1000, -30000, end - 30000).unwrap();
        assert_eq!(m.read_long(0x100e), 0);
        t.remove(&mut m, 0x1000, end - 20000).unwrap();
        t.install(&mut m, 0x1000, true).unwrap();
        t.prime(&mut m, 0x1000, 0, end).unwrap();
        assert_eq!(t.due(&mut m, end + 19999), None);
        assert!(t.due(&mut m, end + 20000).is_some());
    }
    #[test]
    fn equal_deadlines_follow_install_order_not_address_order() {
        let (mut t, mut m) = scene();
        for p in [0x2000, 0x1000] {
            t.install(&mut m, p, false).unwrap();
            t.prime(&mut m, p, 20, 0).unwrap();
        }
        assert_eq!(m.read_long(0x2000), 0x1000);
        assert_eq!(t.due(&mut m, 20000), Some((0x2000, 0)));
        assert_eq!(t.due(&mut m, 20000), Some((0x1000, 0)));
    }
    #[test]
    fn re_priming_replaces_deadline_and_bad_records_do_not_succeed() {
        let (mut t, mut m) = scene();
        for p in [0, 1, 0xffff, 0xfff4] {
            assert_eq!(t.install(&mut m, p, true), Err(-50));
        }
        t.install(&mut m, 0x1000, false).unwrap();
        assert_eq!(t.install(&mut m, 0x1000, false), Err(-50));
        t.prime(&mut m, 0x1000, 10, 0).unwrap();
        t.prime(&mut m, 0x1000, 20, 5000).unwrap();
        assert_eq!(t.due(&mut m, 24999), None);
        assert!(t.due(&mut m, 25000).is_some());
    }
}
