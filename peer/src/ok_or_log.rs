use core::fmt::Display;

pub trait OkOrLog {
    type Return;
    fn ok_or_log(self) -> Self::Return;
}

pub trait OrLog {
    fn or_log(self);
}

impl<T, E: Display> OkOrLog for Result<T, E> {
    type Return = Option<T>;
    fn ok_or_log(self) -> Self::Return {
        match self {
            Ok(ok) => Some(ok),
            Err(err) => {
                log::error!("{}", err);
                None
            }
        }
    }
}

impl<E: Display> OrLog for Result<(), E> {
    fn or_log(self) {
        match self {
            Ok(()) => {}
            Err(err) => {
                log::error!("{}", err);
            }
        }
    }
}
