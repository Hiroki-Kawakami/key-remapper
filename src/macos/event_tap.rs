use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::Instant;

use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventMask, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType, CGPreflightPostEventAccess,
    CGRequestPostEventAccess,
};

use super::keys::modifier_mask;
use crate::rule::{Event, Pipeline};

const INJECTED: i64 = 0x6B72_6D70;

struct State {
    pipeline: Pipeline,
    port: Option<CFRetained<CFMachPort>>,
}

fn mask(types: &[CGEventType]) -> CGEventMask {
    types.iter().fold(0, |mask, t| mask | 1 << t.0)
}

fn classify(event_type: CGEventType, event: &CGEvent) -> Option<Event> {
    let keycode =
        || CGEvent::integer_value_field(Some(event), CGEventField::KeyboardEventKeycode) as u16;
    match event_type {
        CGEventType::KeyDown => Some(Event::KeyDown(keycode())),
        CGEventType::KeyUp => Some(Event::KeyUp(keycode())),
        CGEventType::FlagsChanged => {
            let key = keycode();
            let mask = modifier_mask(key)?;
            if CGEvent::flags(Some(event)).bits() & mask != 0 {
                Some(Event::KeyDown(key))
            } else {
                Some(Event::KeyUp(key))
            }
        }
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown | CGEventType::OtherMouseDown => {
            Some(Event::Pointer)
        }
        _ => None,
    }
}

fn post(events: &[Event], flags: CGEventFlags) {
    for event in events {
        let (keycode, down) = match *event {
            Event::KeyDown(key) => (key, true),
            Event::KeyUp(key) => (key, false),
            Event::Pointer => continue,
        };
        let Some(synthesized) = CGEvent::new_keyboard_event(None, keycode, down) else {
            super::report_error(&format!(
                "CGEventCreateKeyboardEvent failed: keycode {keycode}"
            ));
            continue;
        };
        CGEvent::set_flags(Some(&synthesized), flags);
        CGEvent::set_integer_value_field(
            Some(&synthesized),
            CGEventField::EventSourceUserData,
            INJECTED,
        );
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&synthesized));
    }
}

unsafe extern "C-unwind" fn callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    context: *mut c_void,
) -> *mut CGEvent {
    let state = unsafe { &mut *(context as *mut State) };
    let cg_event = unsafe { event.as_ref() };

    if event_type == CGEventType::TapDisabledByTimeout
        || event_type == CGEventType::TapDisabledByUserInput
    {
        if let Some(port) = &state.port {
            CGEvent::tap_enable(port, true);
        }
        return event.as_ptr();
    }
    if CGEvent::integer_value_field(Some(cg_event), CGEventField::EventSourceUserData) == INJECTED {
        return event.as_ptr();
    }
    let Some(input) = classify(event_type, cg_event) else {
        return event.as_ptr();
    };

    let output = state.pipeline.process(input, Instant::now());
    let flags = CGEvent::flags(Some(cg_event));
    match output.split_first() {
        Some((first, rest)) if *first == input => {
            post(rest, flags);
            event.as_ptr()
        }
        _ => {
            post(&output, flags);
            std::ptr::null_mut()
        }
    }
}

pub fn install(pipeline: Pipeline) -> Result<(), String> {
    if !CGPreflightPostEventAccess() {
        CGRequestPostEventAccess();
    }
    let state = Box::leak(Box::new(State {
        pipeline,
        port: None,
    }));
    let events = mask(&[
        CGEventType::KeyDown,
        CGEventType::KeyUp,
        CGEventType::FlagsChanged,
        CGEventType::LeftMouseDown,
        CGEventType::RightMouseDown,
        CGEventType::OtherMouseDown,
    ]);
    let port = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            events,
            Some(callback),
            state as *mut State as *mut c_void,
        )
    }
    .ok_or("CGEventTapCreate failed (Accessibility permission required)")?;
    let source = CFMachPort::new_run_loop_source(None, Some(&port), 0)
        .ok_or("CFMachPortCreateRunLoopSource failed")?;
    CFRunLoop::current()
        .ok_or("CFRunLoopGetCurrent failed")?
        .add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    CGEvent::tap_enable(&port, true);
    state.port = Some(port);
    Ok(())
}
