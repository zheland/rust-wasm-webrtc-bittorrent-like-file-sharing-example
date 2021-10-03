#[macro_export]
macro_rules! unwrap_or_return {
    ( $expr:expr ) => {
        match $expr {
            Some(value) => value,
            None => return,
        }
    };
    ( $expr:expr, $value:expr ) => {
        match $expr {
            Some(value) => value,
            None => return $value,
        }
    };
}

#[macro_export]
macro_rules! unwrap_or_continue {
    ( $expr:expr ) => {
        match $expr {
            Some(value) => value,
            None => continue,
        }
    };
}
