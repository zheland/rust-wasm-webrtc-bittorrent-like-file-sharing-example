use async_std::sync::Arc;
use web_sys::HtmlDivElement;

// TODO
#[derive(Debug)]
pub struct App {
    placeholder: HtmlDivElement,
}

impl App {
    pub fn new() -> Arc<Self> {
        use crate::{body, ElementExt};

        let placeholder: HtmlDivElement = body().add_child("div");
        placeholder.add_text("Not implemented yet");

        let app = Arc::new(App { placeholder });

        app
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.placeholder.remove();
    }
}
