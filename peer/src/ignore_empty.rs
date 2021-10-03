pub trait IgnoreEmpty {
    fn ignore_empty(self);
}

impl IgnoreEmpty for Option<()> {
    fn ignore_empty(self) {}
}
