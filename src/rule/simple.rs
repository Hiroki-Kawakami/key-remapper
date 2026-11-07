use std::collections::HashMap;
use std::time::Instant;

use super::{Event, Rule};

pub struct SimpleRemap(HashMap<u16, u16>);

impl SimpleRemap {
    pub fn new(pairs: &[(u16, u16)]) -> Self {
        SimpleRemap(pairs.iter().copied().collect())
    }

    fn convert(&self, key: u16) -> u16 {
        *self.0.get(&key).unwrap_or(&key)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_keys() {
        let mut rule = SimpleRemap::new(&[(1, 2), (2, 1)]);
        let now = Instant::now();
        assert_eq!(rule.remap(Event::KeyDown(1), now), [Event::KeyDown(2)]);
        assert_eq!(rule.remap(Event::KeyUp(2), now), [Event::KeyUp(1)]);
        assert_eq!(rule.remap(Event::KeyDown(3), now), [Event::KeyDown(3)]);
    }
}
