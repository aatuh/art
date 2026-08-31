//! Ownership and teardown for one browser view at a time.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, EventTarget, Window};

thread_local! {
    static ACTIVE_VIEW: RefCell<Option<ViewLifecycle>> = const { RefCell::new(None) };
    static TRANSITION_PENDING: Cell<bool> = const { Cell::new(false) };
}

pub(super) struct EventListeners {
    entries: Vec<EventListener>,
}

impl EventListeners {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn listen<T>(
        &mut self,
        target: &T,
        event_name: &'static str,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<(), JsValue>
    where
        T: AsRef<EventTarget>,
    {
        let callback = Closure::<dyn FnMut(Event)>::new(callback);
        target
            .as_ref()
            .add_event_listener_with_callback(event_name, callback.as_ref().unchecked_ref())?;
        self.entries.push(EventListener {
            target: target.as_ref().clone(),
            event_name,
            callback,
        });
        Ok(())
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

struct EventListener {
    target: EventTarget,
    event_name: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

impl Drop for EventListener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback(
            self.event_name,
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

pub(super) struct ViewLifecycle {
    pub(super) listeners: EventListeners,
    animation: Option<AnimationFrameLoop>,
}

impl ViewLifecycle {
    pub(super) const fn new() -> Self {
        Self {
            listeners: EventListeners::new(),
            animation: None,
        }
    }

    pub(super) fn set_animation(&mut self, animation: AnimationFrameLoop) {
        self.animation = Some(animation);
    }
}

impl Drop for ViewLifecycle {
    fn drop(&mut self) {
        drop(self.animation.take());
        self.listeners.clear();
    }
}

pub(super) fn activate(view: ViewLifecycle) -> Result<(), JsValue> {
    ACTIVE_VIEW.with(|active| {
        let mut active = active.borrow_mut();
        if active.is_some() {
            return Err(JsValue::from_str(
                "A gallery view is already active during installation.",
            ));
        }
        *active = Some(view);
        Ok(())
    })
}

/// Defers teardown until after the currently executing event callback has returned.
pub(super) fn schedule_transition(transition: impl FnOnce() -> Result<(), JsValue> + 'static) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let already_pending = TRANSITION_PENDING.with(|pending| pending.replace(true));
    if already_pending {
        return;
    }

    let callback = Closure::once_into_js(move || {
        TRANSITION_PENDING.with(|pending| pending.set(false));
        ACTIVE_VIEW.with(|active| drop(active.borrow_mut().take()));
        if let Err(error) = transition() {
            web_sys::console::error_1(&error);
        }
    });
    window.queue_microtask(callback.unchecked_ref());
}

struct AnimationState {
    window: Window,
    callback: RefCell<Option<AnimationCallback>>,
    request_id: Cell<Option<i32>>,
    running: Cell<bool>,
}

type AnimationCallback = Closure<dyn FnMut(f64)>;

pub(super) struct AnimationFrameLoop {
    state: Rc<AnimationState>,
}

impl AnimationFrameLoop {
    pub(super) fn start(
        mut render_frame: impl FnMut(f64) -> bool + 'static,
    ) -> Result<Self, JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("The browser window is unavailable."))?;
        let state = Rc::new(AnimationState {
            window,
            callback: RefCell::new(None),
            request_id: Cell::new(None),
            running: Cell::new(true),
        });
        let weak_state: Weak<AnimationState> = Rc::downgrade(&state);
        let callback = Closure::<dyn FnMut(f64)>::new(move |now| {
            let Some(state) = weak_state.upgrade() else {
                return;
            };
            state.request_id.set(None);
            if !state.running.get() || !render_frame(now) {
                state.running.set(false);
                return;
            }
            let request = state.callback.borrow().as_ref().and_then(|callback| {
                state
                    .window
                    .request_animation_frame(callback.as_ref().unchecked_ref())
                    .ok()
            });
            state.request_id.set(request);
            if request.is_none() {
                state.running.set(false);
            }
        });
        *state.callback.borrow_mut() = Some(callback);
        let request_id = state.window.request_animation_frame(
            state
                .callback
                .borrow()
                .as_ref()
                .ok_or_else(|| JsValue::from_str("Animation callback is unavailable."))?
                .as_ref()
                .unchecked_ref(),
        )?;
        state.request_id.set(Some(request_id));
        Ok(Self { state })
    }
}

impl Drop for AnimationFrameLoop {
    fn drop(&mut self) {
        self.state.running.set(false);
        if let Some(request_id) = self.state.request_id.take() {
            let _ = self.state.window.cancel_animation_frame(request_id);
        }
        self.state.callback.borrow_mut().take();
    }
}
