use core::cell::RefCell;
use std::sync::Arc;

use async_std::sync::RwLock;
use peer::JsSharedFile;
use web_sys::{Event, HtmlButtonElement, HtmlCanvasElement, HtmlDivElement, HtmlInputElement};

use crate::{ClosureCell1, Time};

#[derive(Debug)]
pub struct FileUi {
    shared_file: Arc<RwLock<JsSharedFile<Time>>>,
    file_div: HtmlDivElement,
    download_button: HtmlButtonElement,
    download_button_handler: ClosureCell1<Event>,
    canvas: Option<HtmlCanvasElement>,
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

        drop(shared_file_ref);

        let download_button: HtmlButtonElement = file_div.add_child("button").unwrap();
        download_button.add_text("Loading").unwrap();
        download_button.set_disabled(true);

        let shared_file_ref = shared_file.read().await;
        let canvas = if shared_file_ref.num_pieces() <= 1024 * 1024 {
            let canvas: HtmlCanvasElement = file_div.add_child("canvas").unwrap();
            canvas.set_width(1024);
            canvas.set_height(256);
            Some(canvas)
        } else {
            None
        };
        drop(shared_file_ref);

        let file_ui = Arc::new(Self {
            shared_file,
            file_div,
            download_button,
            download_button_handler: RefCell::new(None),
            canvas,
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
        use wasm_bindgen::{Clamped, JsCast};
        use web_sys::{CanvasRenderingContext2d, ImageData};

        let shared_file = self.shared_file.read().await;
        let state = shared_file.file().state();

        if state.is_complete() {
            if self.download_button.disabled() {
                self.download_button.replace_text("Download").unwrap();
                self.download_button.set_disabled(false);
                if let Some(canvas) = self.canvas.as_ref() {
                    canvas.remove();
                }
            }
        } else {
            self.download_button
                .replace_text(&format!(
                    "Loading: {}/{}",
                    state.num_available(),
                    state.len()
                ))
                .unwrap();
            if let Some(canvas) = self.canvas.as_ref() {
                let width = canvas.width();
                let height = canvas.height();
                let mut data = vec![0; (width * height * 4) as usize];

                let shared_file = self.shared_file.read().await;
                let num_lines = (shared_file.num_pieces() + 1023) / 256;

                for (j, bit) in shared_file.file().state().raw().iter().enumerate() {
                    let x = j % 1024;
                    let y = j / 1024;
                    for k in y * 1024 / num_lines..(y + 1) * 1024 / num_lines {
                        let offset = (k * 1024 + x) * 4;
                        if *bit {
                            data[offset] = 58;
                            data[offset + 1] = 151;
                            data[offset + 2] = 87;
                            data[offset + 3] = 255;
                        } else {
                            data[offset] = 0;
                            data[offset + 1] = 0;
                            data[offset + 2] = 0;
                            data[offset + 3] = 255;
                        }
                    }
                }

                let image =
                    ImageData::new_with_u8_clamped_array_and_sh(Clamped(&data), width, height)
                        .unwrap();
                let context: CanvasRenderingContext2d = canvas
                    .get_context("2d")
                    .unwrap()
                    .unwrap()
                    .dyn_into()
                    .unwrap();
                context.put_image_data(&image, 0.0, 0.0).unwrap();
            }
        }
    }
}

impl Drop for FileUi {
    fn drop(&mut self) {
        self.file_div.remove();
    }
}
