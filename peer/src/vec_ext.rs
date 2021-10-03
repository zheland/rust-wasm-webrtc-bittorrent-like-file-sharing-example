pub trait SetWithResizeDefault {
    type Key;
    type Value;
    fn get_mut_or_resize_default(&mut self, key: Self::Key) -> &mut Self::Value;
}

pub trait PushAndReturnOffset {
    type Key;
    type Value;
    fn push_and_get_offset(&mut self, value: Self::Value) -> Self::Key;
}

impl<T> PushAndReturnOffset for Vec<T> {
    type Key = usize;
    type Value = T;

    fn push_and_get_offset(&mut self, value: Self::Value) -> Self::Key {
        let offset = self.len();
        self.push(value);
        offset
    }
}

impl<T: Clone + Default> SetWithResizeDefault for Vec<T> {
    type Key = usize;
    type Value = T;

    fn get_mut_or_resize_default(&mut self, key: Self::Key) -> &mut Self::Value {
        if self.len() <= key {
            self.resize(key + 1, T::default())
        }
        &mut self[key]
    }
}
