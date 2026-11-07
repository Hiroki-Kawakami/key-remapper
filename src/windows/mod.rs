mod keys;

use std::cell::RefCell;
use std::mem;
use std::path::Path;
use std::time::Instant;

use serde::Deserialize;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CallNextHookEx, CreatePopupMenu,
    CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW,
    HHOOK, IDI_APPLICATION, KBDLLHOOKSTRUCT, LoadIconW, MB_ICONERROR, MB_OK, MF_STRING, MSG,
    MessageBoxW, PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetWindowsHookExW,
    TPM_LEFTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY, WM_KEYUP, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYUP, WM_XBUTTONDOWN,
    WNDCLASSEXW, WS_POPUP,
};
use windows::core::{HSTRING, w};

use crate::config::{self, RemapEntry, TapEntry};
use crate::rule::{Event, Pipeline, Rule, SimpleRemap};

const INJECTED: usize = 0xA3F7C2D1;
const WM_TRAY_ICON: u32 = 0x8001;
const IDM_EXIT: usize = 1001;

thread_local! {
    static PIPELINE: RefCell<Option<Pipeline>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    remap: Vec<RemapEntry>,
    #[serde(default)]
    tap: Vec<TapEntry>,
}

pub fn report_error(message: &str) {
    eprintln!("key-remapper: {message}");
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            w!("key-remapper"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn process(event: Event) -> Option<Vec<Event>> {
    PIPELINE.with_borrow_mut(|p| p.as_mut().map(|p| p.process(event, Instant::now())))
}

fn send(events: &[Event]) {
    let inputs: Vec<INPUT> = events
        .iter()
        .filter_map(|event| {
            let (vk, mut flags) = match *event {
                Event::KeyDown(vk) => (vk, KEYBD_EVENT_FLAGS(0)),
                Event::KeyUp(vk) => (vk, KEYEVENTF_KEYUP),
                Event::Pointer => return None,
            };
            let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC_EX) };
            if scan & 0xFF00 == 0xE000 {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            Some(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vk),
                        wScan: scan as u16,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: INJECTED,
                    },
                },
            })
        })
        .collect();
    let sent = unsafe { SendInput(&inputs, mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        eprintln!("key-remapper: SendInput sent {sent}/{}", inputs.len());
    }
}

unsafe extern "system" fn keyboard_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode >= 0 {
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if kb.dwExtraInfo != INJECTED {
            let vk = kb.vkCode as u16;
            let input = if matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP) {
                Event::KeyUp(vk)
            } else {
                Event::KeyDown(vk)
            };
            if let Some(output) = process(input)
                && output != [input]
            {
                send(&output);
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode >= 0
        && matches!(
            wparam.0 as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
        )
    {
        process(Event::Pointer);
    }
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TRAY_ICON => {
                let event = (lparam.0 as u32) & 0xFFFF;
                if (event == WM_RBUTTONUP || event == WM_LBUTTONUP)
                    && let Ok(menu) = CreatePopupMenu()
                {
                    let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, w!("Exit"));
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let _ = SetForegroundWindow(hwnd);
                    let _ = TrackPopupMenu(
                        menu,
                        TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                        pt.x,
                        pt.y,
                        Some(0),
                        hwnd,
                        None,
                    );
                    let _ = DestroyMenu(menu);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                if wparam.0 & 0xFFFF == IDM_EXIT {
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn build_pipeline(config: &Config) -> Result<Pipeline, String> {
    let mut rules: Vec<Box<dyn Rule>> = Vec::new();
    let pairs = config::remap_pairs(&config.remap, keys::VIRTUAL_KEYS)?;
    if !pairs.is_empty() {
        rules.push(Box::new(SimpleRemap::new(&pairs)));
    }
    rules.extend(config::tap_rules(&config.tap, keys::VIRTUAL_KEYS)?);
    Ok(Pipeline::new(rules))
}

fn create_tray() -> Result<NOTIFYICONDATAW, String> {
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {e}"))?;
        let class_name = w!("KeyRemapperTray");
        let wc = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: HINSTANCE(hinstance.0),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("key-remapper"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(HINSTANCE(hinstance.0)),
            None,
        )
        .map_err(|e| format!("CreateWindowExW: {e}"))?;

        let mut tip = [0u16; 128];
        for (i, c) in "key-remapper".encode_utf16().enumerate() {
            tip[i] = c;
        }
        let nid = NOTIFYICONDATAW {
            cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            szTip: tip,
            ..Default::default()
        };
        if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            eprintln!("key-remapper: Shell_NotifyIconW failed");
        }
        Ok(nid)
    }
}

pub fn run(path: &Path) -> Result<(), String> {
    let config: Config = config::load(path)?;
    let pipeline = build_pipeline(&config)?;
    let has_tap = !config.tap.is_empty();
    PIPELINE.with_borrow_mut(|p| *p = Some(pipeline));

    let nid = create_tray()?;
    unsafe {
        let mut hooks: Vec<HHOOK> = vec![
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
                .map_err(|e| format!("SetWindowsHookExW(WH_KEYBOARD_LL): {e}"))?,
        ];
        if has_tap {
            hooks.push(
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
                    .map_err(|e| format!("SetWindowsHookExW(WH_MOUSE_LL): {e}"))?,
            );
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        for hook in hooks {
            let _ = UnhookWindowsHookEx(hook);
        }
    }
    Ok(())
}
