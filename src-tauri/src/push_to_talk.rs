use rdev::{EventType, Key};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
struct PushToTalkEvent {
    state: &'static str,
    key: &'static str,
}

pub fn start_fn_listener(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("auralflow-fn-listener".into())
        .spawn(move || {
            let mut fn_down = false;
            let listener = rdev::listen(move |event| match event.event_type {
                EventType::KeyPress(Key::Function) if !fn_down => {
                    fn_down = true;
                    let _ = app.emit(
                        "push-to-talk",
                        PushToTalkEvent {
                            state: "pressed",
                            key: "Fn",
                        },
                    );
                }
                EventType::KeyRelease(Key::Function) if fn_down => {
                    fn_down = false;
                    let _ = app.emit(
                        "push-to-talk",
                        PushToTalkEvent {
                            state: "released",
                            key: "Fn",
                        },
                    );
                }
                _ => {}
            });
            if let Err(error) = listener {
                eprintln!("AuralFlow Fn listener error: {error:?}");
            }
        });
}
