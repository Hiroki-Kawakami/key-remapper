use std::time::{Duration, Instant};

use super::{Event, Rule};

enum State {
    Idle,
    Pending(Instant),
    Interrupted,
}

pub struct Tap {
    key: u16,
    hold: u16,
    alone: u16,
    timeout: Duration,
    state: State,
}

impl Tap {
    /// `hold` を指定すると、`key` を押している間は `key` の代わりに `hold` を送る。
    pub fn new(key: u16, hold: Option<u16>, alone: u16, timeout: Duration) -> Self {
        Tap {
            key,
            hold: hold.unwrap_or(key),
            alone,
            timeout,
            state: State::Idle,
        }
    }
}

impl Rule for Tap {
    fn remap(&mut self, event: Event, now: Instant) -> Vec<Event> {
        match event {
            Event::KeyDown(key) if key == self.key => {
                if let State::Idle = self.state {
                    self.state = State::Pending(now);
                }
                return vec![Event::KeyDown(self.hold)];
            }
            Event::KeyUp(key) if key == self.key => {
                let up = Event::KeyUp(self.hold);
                let state = std::mem::replace(&mut self.state, State::Idle);
                if let State::Pending(pressed_at) = state
                    && now.duration_since(pressed_at) < self.timeout
                {
                    return vec![up, Event::KeyDown(self.alone), Event::KeyUp(self.alone)];
                }
                return vec![up];
            }
            _ => self.observe(event, now),
        }
        vec![event]
    }

    fn observe(&mut self, event: Event, _now: Instant) {
        if matches!(event, Event::KeyDown(_) | Event::Pointer)
            && let State::Pending(_) = self.state
        {
            self.state = State::Interrupted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: u16 = 1;
    const ALONE: u16 = 9;
    const OTHER: u16 = 5;

    fn rule() -> Tap {
        Tap::new(KEY, None, ALONE, Duration::from_millis(1000))
    }

    const HOLD: u16 = 7;

    fn hold_rule() -> Tap {
        Tap::new(KEY, Some(HOLD), ALONE, Duration::from_millis(1000))
    }

    #[test]
    fn hold_replaces_key_and_emits_alone_key() {
        let mut rule = hold_rule();
        let now = Instant::now();
        assert_eq!(rule.remap(Event::KeyDown(KEY), now), [Event::KeyDown(HOLD)]);
        assert_eq!(
            rule.remap(Event::KeyUp(KEY), now),
            [
                Event::KeyUp(HOLD),
                Event::KeyDown(ALONE),
                Event::KeyUp(ALONE)
            ]
        );
    }

    #[test]
    fn hold_replaces_key_when_interrupted() {
        let mut rule = hold_rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        rule.remap(Event::KeyDown(OTHER), now);
        assert_eq!(rule.remap(Event::KeyUp(KEY), now), [Event::KeyUp(HOLD)]);
    }

    #[test]
    fn hold_key_itself_is_untouched() {
        let mut rule = hold_rule();
        let now = Instant::now();
        assert_eq!(rule.remap(Event::KeyDown(HOLD), now), [Event::KeyDown(HOLD)]);
        assert_eq!(rule.remap(Event::KeyUp(HOLD), now), [Event::KeyUp(HOLD)]);
    }

    #[test]
    fn emits_alone_key_after_release() {
        let mut rule = rule();
        let now = Instant::now();
        assert_eq!(rule.remap(Event::KeyDown(KEY), now), [Event::KeyDown(KEY)]);
        assert_eq!(
            rule.remap(Event::KeyUp(KEY), now + Duration::from_millis(100)),
            [
                Event::KeyUp(KEY),
                Event::KeyDown(ALONE),
                Event::KeyUp(ALONE)
            ]
        );
    }

    #[test]
    fn other_key_interrupts() {
        let mut rule = rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        rule.remap(Event::KeyDown(OTHER), now);
        rule.remap(Event::KeyUp(OTHER), now);
        assert_eq!(rule.remap(Event::KeyUp(KEY), now), [Event::KeyUp(KEY)]);
    }

    #[test]
    fn pointer_interrupts() {
        let mut rule = rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        rule.remap(Event::Pointer, now);
        assert_eq!(rule.remap(Event::KeyUp(KEY), now), [Event::KeyUp(KEY)]);
    }

    #[test]
    fn timeout_suppresses_alone_key() {
        let mut rule = rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        assert_eq!(
            rule.remap(Event::KeyUp(KEY), now + Duration::from_millis(1000)),
            [Event::KeyUp(KEY)]
        );
    }

    #[test]
    fn autorepeat_keeps_first_press_time() {
        let mut rule = rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        rule.remap(Event::KeyDown(KEY), now + Duration::from_millis(900));
        assert_eq!(
            rule.remap(Event::KeyUp(KEY), now + Duration::from_millis(1100)),
            [Event::KeyUp(KEY)]
        );
    }

    #[test]
    fn autorepeat_after_interrupt_does_not_rearm() {
        let mut rule = rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        rule.remap(Event::KeyDown(OTHER), now);
        rule.remap(Event::KeyDown(KEY), now);
        assert_eq!(rule.remap(Event::KeyUp(KEY), now), [Event::KeyUp(KEY)]);
    }

    #[test]
    fn release_of_key_held_before_press_does_not_interrupt() {
        let mut rule = rule();
        let now = Instant::now();
        rule.remap(Event::KeyDown(KEY), now);
        rule.remap(Event::KeyUp(OTHER), now);
        assert_eq!(rule.remap(Event::KeyUp(KEY), now).len(), 3);
    }
}
