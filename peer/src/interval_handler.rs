use std::time::Duration;
use thiserror::Error;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsValue;
use web_sys::Window;

#[derive(Debug)]
pub struct IntervalHandler {
    window: Window,
    callback: Closure<IntervalHandlerFn>,
    interval_id: i32,
}

pub type IntervalHandlerFn = dyn FnMut() -> ();

impl IntervalHandler {
    pub fn new<F>(mut callback: F, interval: Duration) -> Result<Self, NewIntervalHandlerError>
    where
        F: 'static + FnMut() -> (),
    {
        use wasm_bindgen::JsCast;

        let window = web_sys::window().ok_or(NewIntervalHandlerError::WindowDoesNotExist)?;

        let callback: Box<dyn FnMut()> = Box::new(move || callback());
        let callback = Closure::wrap(callback);
        let interval_ms = (interval.as_secs_f64() * 1000.0).floor() as i32;
        let interval_id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                interval_ms,
            )
            .map_err(NewIntervalHandlerError::SetIntervalError)?;
        Ok(Self {
            window,
            callback,
            interval_id,
        })
    }
}

impl Drop for IntervalHandler {
    fn drop(&mut self) {
        self.window.clear_interval_with_handle(self.interval_id);
    }
}

#[derive(Error, Debug)]
pub enum NewIntervalHandlerError {
    #[error("window object does not exist")]
    WindowDoesNotExist,
    #[error("set interval error: {0:?}")]
    SetIntervalError(JsValue),
}
