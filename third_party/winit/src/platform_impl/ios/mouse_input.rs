use std::sync::{Mutex, OnceLock};
use block2::RcBlock;
use objc2_game_controller::GCMouse;

#[derive(Default)]
pub(crate) struct MouseMotionQueue {
    delta: Mutex<(f64, f64)>,
}

impl MouseMotionQueue {
    pub(crate) fn push(&self, dx: f64, dy: f64) {
        let mut delta = self.delta.lock().unwrap_or_else(|e| e.into_inner());
        delta.0 += dx;
        delta.1 += dy;
    }

    pub(crate) fn take(&self) -> Option<(f64, f64)> {
        let mut delta = self.delta.lock().unwrap_or_else(|e| e.into_inner());
        if delta.0 == 0.0 && delta.1 == 0.0 {
            return None;
        }

        let result = *delta;
        *delta = (0.0, 0.0);
        Some(result)
    }
}

static MOUSE_MOTION: OnceLock<MouseMotionQueue> = OnceLock::new();

fn queue() -> &'static MouseMotionQueue {
    MOUSE_MOTION.get_or_init(MouseMotionQueue::default)
}

pub(crate) fn push_motion(dx: f64, dy: f64) {
    if dx.is_finite() && dy.is_finite() {
        queue().push(dx, dy);

        // Wake the iOS main run loop so pending movement can be processed.
        unsafe {
            let run_loop = core_foundation::runloop::CFRunLoopGetMain();
            core_foundation::runloop::CFRunLoopWakeUp(run_loop);
        }
    }
}

pub(crate) fn take_motion() -> Option<(f64, f64)> {
    queue().take()
}

/// Register motion callbacks for mice already connected to iOS.
pub(crate) fn register_mouse_motion() {
    let mice = unsafe { GCMouse::mice() };

    for mouse in mice.iter() {
        let Some(input) = (unsafe { mouse.mouseInput() }) else {
            continue;
        };

        let callback = RcBlock::new(
            move |_input, dx: f32, dy: f32| {
                push_motion(dx as f64, dy as f64);
            },
        );

        unsafe {
            input.setMouseMovedHandler(callback.as_ptr() as *mut _);
        }
    }
}
