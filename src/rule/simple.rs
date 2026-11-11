use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::{Event, Rule};

pub struct SimpleRemap {
    map: HashMap<u16, u16>,
    finals: HashSet<u16>,
}

impl SimpleRemap {
    /// `finals` に含まれる変換元キーは、変換後に後段のルールで変換しない。
    pub fn new(pairs: &[(u16, u16)], finals: &[u16]) -> Self {
        SimpleRemap {
            map: pairs.iter().copied().collect(),
            finals: finals.iter().copied().collect(),
        }
    }

    fn convert(&self, key: u16) -> u16 {
        *self.map.get(&key).unwrap_or(&key)
    }
}

impl Rule for SimpleRemap {
    fn remap(&mut self, event: Event, _now: Instant) -> Vec<Event> {
        vec![match event {
            Event::KeyDown(key) => Event::KeyDown(self.convert(key)),
            Event::KeyUp(key) => Event::KeyUp(self.convert(key)),
            Event::Pointer => Event::Pointer,
        }]
    }

    fn finalizes(&self, event: Event) -> bool {
        match event {
            Event::KeyDown(key) | Event::KeyUp(key) => self.finals.contains(&key),
            Event::Pointer => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_keys() {
        let mut rule = SimpleRemap::new(&[(1, 2), (2, 1)], &[]);
        let now = Instant::now();
        assert_eq!(rule.remap(Event::KeyDown(1), now), [Event::KeyDown(2)]);
        assert_eq!(rule.remap(Event::KeyUp(2), now), [Event::KeyUp(1)]);
        assert_eq!(rule.remap(Event::KeyDown(3), now), [Event::KeyDown(3)]);
    }

    #[test]
    fn finalizes_only_listed_keys() {
        let rule = SimpleRemap::new(&[(1, 2), (3, 4)], &[1]);
        assert!(rule.finalizes(Event::KeyDown(1)));
        assert!(rule.finalizes(Event::KeyUp(1)));
        assert!(!rule.finalizes(Event::KeyDown(3)));
        assert!(!rule.finalizes(Event::Pointer));
    }
}
