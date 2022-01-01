use js_sys::Uint8Array;

pub trait FileChunk {
    fn with_len(len: usize) -> Self;
    fn get(&self, offset: usize, len: usize) -> Box<[u8]>;
    fn set(&mut self, offset: usize, data: &[u8]);
}

impl FileChunk for Box<[u8]> {
    fn with_len(len: usize) -> Self {
        vec![0; len].into_boxed_slice()
    }

    fn get(&self, offset: usize, len: usize) -> Box<[u8]> {
        self[offset..self.len().min(offset + len)]
            .to_vec()
            .into_boxed_slice()
    }

    fn set(&mut self, offset: usize, data: &[u8]) {
        self[offset..offset + data.len()].copy_from_slice(data);
    }
}

impl FileChunk for Uint8Array {
    fn with_len(len: usize) -> Self {
        Uint8Array::new_with_length(len.try_into().unwrap())
    }

    fn get(&self, offset: usize, len: usize) -> Box<[u8]> {
        self.slice(
            offset.try_into().unwrap(),
            (offset + len).try_into().unwrap(),
        )
        .to_vec()
        .into_boxed_slice()
    }

    fn set(&mut self, offset: usize, data: &[u8]) {
        let slice = self.subarray(
            offset.try_into().unwrap(),
            (offset + data.len()).try_into().unwrap(),
        );
        slice.copy_from(data);
    }
}
