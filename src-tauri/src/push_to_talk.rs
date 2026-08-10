use std::{ffi::c_void, ptr::NonNull};

use objc2_core_foundation::{kCFRunLoopCommonModes, CFMachPort, CFRunLoop};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventTapCallBack, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventTapProxy, CGEventType,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const FN_KEY_CODE: i64 = 63;
const FLAGS_CHANGED_MASK: u64 = 1 << 12;

#[derive(Clone, Serialize)]
struct PushToTalkEvent {
    state: &'static str,
    key: &'static str,
}

unsafe extern "C-unwind" fn fn_event_callback(
    _proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    if event_type == CGEventType::FlagsChanged {
        let event_ref = unsafe { event.as_ref() };
        let key_code =
            CGEvent::integer_value_field(Some(event_ref), CGEventField::KeyboardEventKeycode);

        // Fn/Globe is virtual key 63. Arrow and navigation keys can carry the
        // same Function flag, but they have their own key codes and never pass
        // this check.
        if key_code == FN_KEY_CODE && !user_info.is_null() {
            let is_down = CGEvent::flags(Some(event_ref)).contains(CGEventFlags::MaskSecondaryFn);
            let app = unsafe { &*user_info.cast::<AppHandle>() };
            let _ = app.emit(
                "push-to-talk",
                PushToTalkEvent {
                    state: if is_down { "pressed" } else { "released" },
                    key: "Fn",
                },
            );
        }
    }

    event.as_ptr()
}

pub fn start_fn_listener(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("auralflow-fn-listener".into())
        .spawn(move || unsafe {
            let user_info = Box::into_raw(Box::new(app)).cast::<c_void>();
            let callback: CGEventTapCallBack = Some(fn_event_callback);
            let Some(tap) = CGEvent::tap_create(
                CGEventTapLocation::HIDEventTap,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::ListenOnly,
                FLAGS_CHANGED_MASK,
                callback,
                user_info,
            ) else {
                drop(Box::from_raw(user_info.cast::<AppHandle>()));
                return;
            };
            let Some(source) = CFMachPort::new_run_loop_source(None, Some(&tap), 0) else {
                drop(Box::from_raw(user_info.cast::<AppHandle>()));
                return;
            };
            let Some(run_loop) = CFRunLoop::current() else {
                drop(Box::from_raw(user_info.cast::<AppHandle>()));
                return;
            };

            run_loop.add_source(Some(&source), kCFRunLoopCommonModes);
            CGEvent::tap_enable(&tap, true);
            CFRunLoop::run();
        });
}
