mod event_tap;
mod hid;
mod hidutil;
mod keys;
mod signal;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use objc2_core_foundation::CFRunLoop;
use serde::Deserialize;

use crate::config::{self, RemapEntry, TapEntry};
use crate::rule::Pipeline;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    remap: Vec<RemapEntry>,
    #[serde(default)]
    device: Vec<DeviceConfig>,
    #[serde(default)]
    tap: Vec<TapEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceConfig {
    vendor_id: u32,
    product_id: u32,
    #[serde(default)]
    remap: Vec<RemapEntry>,
}

struct DeviceMapping {
    vendor_id: u32,
    product_id: u32,
    mapping: Vec<(u16, u16)>,
}

fn merge(global: &[(u16, u16)], device: &[(u16, u16)]) -> Vec<(u16, u16)> {
    let mut merged: Vec<(u16, u16)> = global
        .iter()
        .copied()
        .filter(|(src, _)| !device.iter().any(|(s, _)| s == src))
        .collect();
    merged.extend_from_slice(device);
    merged
}

pub fn report_error(message: &str) {
    eprintln!("key-remapper: {message}");
}

pub fn run(path: &Path) -> Result<(), String> {
    let config: Config = config::load(path)?;
    let global = config::remap_pairs(&config.remap, keys::HID_USAGES)?;
    let devices = config
        .device
        .iter()
        .map(|d| {
            let device = config::remap_pairs(&d.remap, keys::HID_USAGES)?;
            Ok(DeviceMapping {
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                mapping: merge(&global, &device),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let taps = config::tap_rules(&config.tap, keys::KEYCODES)?;

    let applied = Arc::new(Mutex::new(BTreeSet::new()));
    signal::clear_mappings_on_exit(applied.clone());

    if !taps.is_empty() {
        event_tap::install(Pipeline::new(taps))?;
    }

    hid::watch_keyboards(Box::new(move |keyboard| {
        eprintln!(
            "key-remapper: keyboard attached: location_id={:#010x} vendor_id={:?} product_id={:?}",
            keyboard.location_id, keyboard.vendor_id, keyboard.product_id
        );
        let mapping = devices
            .iter()
            .find(|d| {
                keyboard.vendor_id == Some(d.vendor_id) && keyboard.product_id == Some(d.product_id)
            })
            .map_or(global.as_slice(), |d| d.mapping.as_slice());
        if mapping.is_empty() {
            return;
        }
        match hidutil::set_mapping(keyboard.location_id, mapping) {
            Ok(()) => {
                applied.lock().unwrap().insert(keyboard.location_id);
            }
            Err(e) => report_error(&e),
        }
    }));

    CFRunLoop::run();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_mapping_overrides_global() {
        assert_eq!(merge(&[(1, 2), (3, 4)], &[(1, 5)]), [(3, 4), (1, 5)]);
    }
}
