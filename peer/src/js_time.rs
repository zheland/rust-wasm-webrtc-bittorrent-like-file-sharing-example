use std::time::Duration;
use thiserror::Error;

pub fn now() -> Result<Duration, NowError> {
    let window = web_sys::window().ok_or_else(|| NowError::UndefinedWindow)?;
    let performance = window
        .performance()
        .ok_or_else(|| NowError::UndefinedPerformance)?;
    Ok(Duration::from_secs_f64(performance.now() * 0.001))
}

#[derive(Clone, Copy, Error, Debug)]
pub enum NowError {
    #[error("js window is undefined")]
    UndefinedWindow,
    #[error("js performance is undefined")]
    UndefinedPerformance,
}
