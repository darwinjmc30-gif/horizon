use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use block2::RcBlock;
use objc2_game_controller::{GCMouse, GCControllerButtonInput};

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


#[derive(Clone, Copy, Debug)]
pub(crate) enum PhysicalMouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MouseButtonEvent {
    pub button: PhysicalMouseButton,
    pub pressed: bool,
}

static MOUSE_BUTTONS: OnceLock<Mutex<VecDeque<MouseButtonEvent>>> = OnceLock::new();

pub(crate) fn push_button(button: PhysicalMouseButton, pressed: bool) {
    let queue = MOUSE_BUTTONS.get_or_init(|| Mutex::new(VecDeque::new()));
    queue.lock().unwrap_or_else(|e| e.into_inner())
        .push_back(MouseButtonEvent { button, pressed });

    unsafe {
        let run_loop = core_foundation::runloop::CFRunLoopGetMain();
        core_foundation::runloop::CFRunLoopWakeUp(run_loop);
    }
}

pub(crate) fn take_buttons() -> Vec<MouseButtonEvent> {
    let queue = MOUSE_BUTTONS.get_or_init(|| Mutex::new(VecDeque::new()));
    queue.lock().unwrap_or_else(|e| e.into_inner())
        .drain(..).collect()
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


fn register_button(
    input: &GCControllerButtonInput,
    button: PhysicalMouseButton,
) {
    let callback = RcBlock::new(
        move |
            _input: std::ptr::NonNull<GCControllerButtonInput>,
            _value: f32,
            pressed: objc2::runtime::Bool,
        | {
            push_button(button, pressed.as_bool());
        },
    );

    unsafe {
        input.setPressedChangedHandler(
            (&*callback as *const _) as *mut _
        );
    }

    // Keep callback storage alive while the device is registered.
    std::mem::forget(callback);
}

/// Register motion callbacks for mice already connected to iOS.
pub(crate) fn register_mouse_motion() {
    let mice = unsafe { GCMouse::mice() };

    for mouse in mice.iter() {
        let Some(input) = (unsafe { mouse.mouseInput() }) else {
            continue;
        };

        let left_button = unsafe { input.leftButton() };
        register_button(
            &*left_button,
            PhysicalMouseButton::Left,
        );

        if let Some(button) = unsafe { input.rightButton() } {
            register_button(&button, PhysicalMouseButton::Right);
        }

        if let Some(button) = unsafe { input.middleButton() } {
            register_button(&button, PhysicalMouseButton::Middle);
        }

        let callback = RcBlock::new(
            move |_input: std::ptr::NonNull<objc2_game_controller::GCMouseInput>, dx: f32, dy: f32| {
                push_motion(dx as f64, dy as f64);
            },
        );

        unsafe {
            input.setMouseMovedHandler((&*callback as *const _) as *mut _);
        }

        // Keep the block alive for the lifetime of the process.
        std::mem::forget(callback);
    }
}
