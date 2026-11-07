use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use super::hidutil;

const SIG_BLOCK: i32 = 1;
const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

unsafe extern "C" {
    fn pthread_sigmask(how: i32, set: *const u32, old: *mut u32) -> i32;
    fn sigwait(set: *const u32, sig: *mut i32) -> i32;
}

fn sigset(signals: &[i32]) -> u32 {
    signals.iter().fold(0, |set, sig| set | 1 << (sig - 1))
}

pub fn clear_mappings_on_exit(applied: Arc<Mutex<BTreeSet<u32>>>) {
    let set = sigset(&[SIGHUP, SIGINT, SIGTERM]);
    unsafe { pthread_sigmask(SIG_BLOCK, &set, std::ptr::null_mut()) };
    std::thread::spawn(move || {
        let mut sig = 0;
        unsafe { sigwait(&set, &mut sig) };
        for location_id in applied.lock().unwrap().iter() {
            if let Err(e) = hidutil::set_mapping(*location_id, &[]) {
                super::report_error(&e);
            }
        }
        std::process::exit(0);
    });
}
