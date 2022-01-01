use core::ops::Add;
use std::time::Duration;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Time(pub Duration);

pub fn monotonic_time() -> Result<Time, NowError> {
    let window = web_sys::window().ok_or_else(|| NowError::UndefinedWindow)?;
    let performance = window
        .performance()
        .ok_or_else(|| NowError::UndefinedPerformance)?;
    Ok(Time(Duration::from_secs_f64(performance.now() * 0.001)))
}

impl Time {
    pub fn saturating_sub(self, rhs: Duration) -> Self {
        Time(self.0.saturating_sub(rhs))
    }
}

impl Add<Duration> for Time {
    type Output = Time;
    fn add(self, rhs: Duration) -> Self::Output {
        Time(self.0 + rhs)
    }
}

#[derive(Clone, Copy, Error, Debug, Eq, PartialEq)]
pub enum NowError {
    #[error("js window is undefined")]
    UndefinedWindow,
    #[error("js performance is undefined")]
    UndefinedPerformance,
}
