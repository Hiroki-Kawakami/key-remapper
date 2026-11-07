use std::time::{Duration, Instant};

use super::{Event, Rule};

enum State {
    Idle,
    Pending(Instant),
    Interrupted,
}

pub struct Tap {
    key: u16,
    alone: u16,
    timeout: Duration,
    state: State,
}

impl Tap {
    pub fn new(key: u16, alone: u16, timeout: Duration) -> Self {
        Tap {
            key,
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
            }
            Event::KeyUp(key) if key == self.key => {
                let state = std::mem::replace(&mut self.state, State::Idle);
                if let State::Pending(pressed_at) = state
                    && now.duration_since(pressed_at) < self.timeout
                {
                    return vec![event, Event::KeyDown(self.alone), Event::KeyUp(self.alone)];
                }
            }
            Event::KeyDown(_) | Event::Pointer => {
                if let State::Pending(_) = self.state {
                    self.state = State::Interrupted;
                }
            }
            Event::KeyUp(_) => {}
        }
        vec![event]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: u16 = 1;
    const ALONE: u16 = 9;
    const OTHER: u16 = 5;

    fn rule() -> Tap {
        Tap::new(KEY, ALONE, Duration::from_millis(1000))
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
