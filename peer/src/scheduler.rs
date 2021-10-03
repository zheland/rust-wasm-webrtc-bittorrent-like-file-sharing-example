pub async fn macrotask() {
    use js_sys::Promise;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::MessageChannel;

    let channel = MessageChannel::new().unwrap();
    let promise = Promise::new(&mut |resolve, _| {
        channel.port1().set_onmessage(Some(&resolve));
    });
    channel.port2().post_message(&JsValue::UNDEFINED).unwrap();
    let _: JsValue = JsFuture::from(promise).await.unwrap();
}
