#[cfg(any(windows, test))]
mod simple;
mod tap;

#[cfg(any(windows, test))]
pub use simple::SimpleRemap;
pub use tap::Tap;

use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    KeyDown(u16),
    KeyUp(u16),
    Pointer,
}

pub trait Rule {
    fn remap(&mut self, event: Event, now: Instant) -> Vec<Event>;
}

pub struct Pipeline(Vec<Box<dyn Rule>>);

impl Pipeline {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Pipeline(rules)
    }

    pub fn process(&mut self, event: Event, now: Instant) -> Vec<Event> {
        let mut events = vec![event];
        for rule in &mut self.0 {
            events = events
                .into_iter()
                .flat_map(|e| rule.remap(e, now))
                .collect();
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn remapped_key_feeds_next_rule() {
        let mut pipeline = Pipeline::new(vec![
            Box::new(SimpleRemap::new(&[(1, 2)])),
            Box::new(Tap::new(2, 9, Duration::from_secs(1))),
        ]);
        let now = Instant::now();
        assert_eq!(
            pipeline.process(Event::KeyDown(1), now),
            [Event::KeyDown(2)]
        );
        assert_eq!(
            pipeline.process(Event::KeyUp(1), now),
            [Event::KeyUp(2), Event::KeyDown(9), Event::KeyUp(9)]
        );
    }
}
