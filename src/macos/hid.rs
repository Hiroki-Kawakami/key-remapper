use std::ffi::{CStr, c_void};
use std::ptr::NonNull;

use objc2_core_foundation::{
    CFDictionary, CFNumber, CFRetained, CFRunLoop, CFString, kCFRunLoopDefaultMode,
};
use objc2_io_kit::{
    IOHIDDevice, IOHIDManager, IOReturn, kIOHIDDeviceUsageKey, kIOHIDDeviceUsagePageKey,
    kIOHIDLocationIDKey, kIOHIDOptionsTypeNone, kIOHIDProductIDKey, kIOHIDVendorIDKey,
};

const GENERIC_DESKTOP_PAGE: i32 = 0x01;
const KEYBOARD_USAGE: i32 = 0x06;

pub struct Keyboard {
    pub vendor_id: Option<u32>,
    pub product_id: Option<u32>,
    pub location_id: u32,
}

type Handler = Box<dyn FnMut(Keyboard)>;

fn cf_string(key: &CStr) -> CFRetained<CFString> {
    CFString::from_str(key.to_str().unwrap())
}

fn int_property(device: &IOHIDDevice, key: &CStr) -> Option<u32> {
    let value = device.property(&cf_string(key))?;
    value.downcast_ref::<CFNumber>()?.as_i64().map(|v| v as u32)
}

unsafe extern "C-unwind" fn matched(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    device: NonNull<IOHIDDevice>,
) {
    let handler = unsafe { &mut *(context as *mut Handler) };
    let device = unsafe { device.as_ref() };
    let Some(location_id) = int_property(device, kIOHIDLocationIDKey) else {
        super::report_error("keyboard without LocationID ignored");
        return;
    };
    handler(Keyboard {
        vendor_id: int_property(device, kIOHIDVendorIDKey),
        product_id: int_property(device, kIOHIDProductIDKey),
        location_id,
    });
}

pub fn watch_keyboards(handler: Handler) {
    let manager = IOHIDManager::new(None, kIOHIDOptionsTypeNone);
    let keys = [
        cf_string(kIOHIDDeviceUsagePageKey),
        cf_string(kIOHIDDeviceUsageKey),
    ];
    let values = [
        CFNumber::new_i32(GENERIC_DESKTOP_PAGE),
        CFNumber::new_i32(KEYBOARD_USAGE),
    ];
    let matching = CFDictionary::from_slices(&[&*keys[0], &*keys[1]], &[&*values[0], &*values[1]]);
    let context = Box::into_raw(Box::new(handler)) as *mut c_void;
    unsafe {
        manager.set_device_matching(Some(matching.as_opaque()));
        manager.register_device_matching_callback(Some(matched), context);
        manager.schedule_with_run_loop(
            &CFRunLoop::current().unwrap(),
            kCFRunLoopDefaultMode.unwrap(),
        );
    }
    std::mem::forget(manager);
}
