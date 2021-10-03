use web_sys::Blob;

#[derive(Debug)]
pub struct ObjectUrl(String);

impl ObjectUrl {
    pub fn from_blob(blob: Blob) -> Self {
        use web_sys::Url;
        Self(Url::create_object_url_with_blob(&blob).unwrap())
    }
}

impl Drop for ObjectUrl {
    fn drop(&mut self) {
        use web_sys::Url;
        Url::revoke_object_url(&self.0).unwrap();
    }
}
