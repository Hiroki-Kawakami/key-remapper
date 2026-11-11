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

    /// `true` なら、`event` を `remap` した結果を後段のルールで変換しない。
    fn finalizes(&self, _event: Event) -> bool {
        false
    }

    /// 前段で確定したイベントを、変換せずに観測する。
    fn observe(&mut self, _event: Event, _now: Instant) {}
}

pub struct Pipeline(Vec<Box<dyn Rule>>);

impl Pipeline {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Pipeline(rules)
    }

    pub fn process(&mut self, event: Event, now: Instant) -> Vec<Event> {
        let mut events = vec![(event, false)];
        for rule in &mut self.0 {
            events = events
                .into_iter()
                .flat_map(|(e, done)| {
                    if done {
                        rule.observe(e, now);
                        return vec![(e, true)];
                    }
                    let finalized = rule.finalizes(e);
                    rule.remap(e, now)
                        .into_iter()
                        .map(|o| (o, finalized))
                        .collect()
                })
                .collect();
        }
        events.into_iter().map(|(e, _)| e).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn remapped_key_feeds_next_rule() {
        let mut pipeline = Pipeline::new(vec![
            Box::new(SimpleRemap::new(&[(1, 2)], &[])),
            Box::new(Tap::new(2, None, 9, Duration::from_secs(1))),
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

    fn final_pipeline() -> Pipeline {
        Pipeline::new(vec![
            Box::new(SimpleRemap::new(&[(1, 2)], &[1])),
            Box::new(Tap::new(2, Some(3), 9, Duration::from_secs(1))),
        ])
    }

    #[test]
    fn final_remap_skips_next_rule() {
        let mut pipeline = final_pipeline();
        let now = Instant::now();
        assert_eq!(
            pipeline.process(Event::KeyDown(1), now),
            [Event::KeyDown(2)]
        );
        assert_eq!(pipeline.process(Event::KeyUp(1), now), [Event::KeyUp(2)]);
        assert_eq!(
            pipeline.process(Event::KeyDown(2), now),
            [Event::KeyDown(3)]
        );
    }

    #[test]
    fn final_remap_still_interrupts_tap() {
        let mut pipeline = final_pipeline();
        let now = Instant::now();
        pipeline.process(Event::KeyDown(2), now);
        pipeline.process(Event::KeyDown(1), now);
        pipeline.process(Event::KeyUp(1), now);
        assert_eq!(pipeline.process(Event::KeyUp(2), now), [Event::KeyUp(3)]);
    }
}
