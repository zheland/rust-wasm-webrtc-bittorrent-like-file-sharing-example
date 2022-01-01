use core::cell::RefCell;
use std::sync::Arc;

use async_std::sync::RwLock;
use peer::JsSharedFile;
use web_sys::{Event, HtmlButtonElement, HtmlDivElement, HtmlInputElement};

use crate::{ClosureCell1, Time};

#[derive(Debug)]
pub struct FileUi {
    shared_file: Arc<RwLock<JsSharedFile<Time>>>,
    file_div: HtmlDivElement,
    download_button: HtmlButtonElement,
    download_button_handler: ClosureCell1<Event>,
}

impl FileUi {
    pub async fn new(shared_file: Arc<RwLock<JsSharedFile<Time>>>) -> Arc<Self> {
        use crate::{body, ElementExt};

        let shared_file_ref = shared_file.read().await;
        let metadata = shared_file_ref.file().metadata();

        let file_div: HtmlDivElement = body().unwrap().add_div().unwrap();

        file_div.add_div().unwrap().add_text("File:").unwrap();
        file_div
            .add_div()
            .unwrap()
            .add_text(&format!(
                "name = {}, len = {}, sha256 = {}",
                metadata.name(),
                metadata.len().0,
                metadata.sha256()
            ))
            .unwrap();

        let magnet_input: HtmlInputElement = file_div
            .add_input("magnet", &metadata.encode_base64().unwrap())
            .unwrap();
        magnet_input.class_list().add_1("magnet").unwrap();
        magnet_input.set_read_only(true);

        let download_button: HtmlButtonElement = file_div.add_child("button").unwrap();
        download_button.add_text("Loading").unwrap();
        download_button.set_disabled(true);

        drop(shared_file_ref);

        let file_ui = Arc::new(Self {
            shared_file,
            file_div,
            download_button,
            download_button_handler: RefCell::new(None),
        });

        file_ui.init();

        file_ui
    }

    fn init(self: &Arc<Self>) {
        use crate::init_weak_callback;
        use web_sys::HtmlElement;

        init_weak_callback(
            &self,
            Self::on_download_click,
            &self.download_button_handler,
            HtmlElement::set_onclick,
            &self.download_button,
        );
    }

    fn on_download_click(self: &Arc<Self>, _: Event) {
        use crate::{body, ElementExt};
        use wasm_bindgen_futures::spawn_local;
        use web_sys::{HtmlAnchorElement, Url};

        let file_ui = Arc::clone(&self);
        spawn_local(async move {
            let shared_file = file_ui.shared_file.read().await;
            let blob = shared_file.file().to_blob().await;
            let blob = match blob {
                Ok(blob) => blob,
                Err(err) => {
                    log::error!("Can not download file: {:?}", err);
                    return;
                }
            };
            let url = Url::create_object_url_with_blob(&blob).unwrap();

            let link: HtmlAnchorElement = body().unwrap().add_child("a").unwrap();
            let name = shared_file.file().metadata().name();
            link.set_href(&url);
            link.set_target("_blank");
            link.set_download(name);
            link.click();
            Url::revoke_object_url(&url).unwrap();
        })
    }

    pub async fn update(self: &Arc<Self>) {
        use crate::ElementExt;

        let shared_file = self.shared_file.read().await;
        let state = shared_file.file().state();

        if state.is_complete() {
            if self.download_button.disabled() {
                self.download_button.replace_text("Download").unwrap();
                self.download_button.set_disabled(false);
            }
        } else {
            self.download_button
                .replace_text(&format!(
                    "Loading: {}/{}",
                    state.num_available(),
                    state.len()
                ))
                .unwrap();
        }
    }
}

impl Drop for FileUi {
    fn drop(&mut self) {
        self.file_div.remove();
    }
}
