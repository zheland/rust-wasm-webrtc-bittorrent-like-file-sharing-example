use thiserror::Error;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlDivElement, HtmlElement, HtmlInputElement, Node, Window};

pub fn window() -> Result<Window, WindowError> {
    web_sys::window().ok_or(WindowError::WindowDoesNotExist)
}

fn document() -> Result<Document, DocumentError> {
    window()?
        .document()
        .ok_or(DocumentError::DocumentDoesNotExist)
}

pub fn body() -> Result<HtmlElement, DocumentBodyError> {
    document()?
        .body()
        .ok_or(DocumentBodyError::BodyDoesNotExist)
}

pub trait ElementExt {
    fn add_child<T: JsCast>(&self, name: &str) -> Result<T, ElementAddChildError>;
    fn add_text(&self, text: &str) -> Result<(), ElementAddTextError>;
    fn add_input(&self, text: &str, value: &str) -> Result<HtmlInputElement, ElementAddInputError>;
    fn remove(&self) -> Result<(), ElementRemoveError>;

    fn add_div(&self) -> Result<HtmlDivElement, ElementAddChildError> {
        self.add_child("div")
    }
}

impl ElementExt for Element {
    fn add_child<T: JsCast>(&self, name: &str) -> Result<T, ElementAddChildError> {
        let node = document()?
            .create_element(name)
            .map_err(ElementAddChildError::CreateElementError)?;
        let _: Node = self
            .append_child(&node)
            .map_err(ElementAddChildError::AppendChildError)?;
        node.dyn_into().map_err(ElementAddChildError::DynIntoError)
    }

    fn add_text(&self, text: &str) -> Result<(), ElementAddTextError> {
        let node = document()?.create_text_node(text);
        let _: Node = self
            .append_child(&node)
            .map_err(ElementAddTextError::AppendChildError)?;
        Ok(())
    }

    fn add_input(&self, text: &str, value: &str) -> Result<HtmlInputElement, ElementAddInputError> {
        use web_sys::{HtmlLabelElement, HtmlSpanElement};

        let label: HtmlLabelElement = self.add_child("label")?;
        let span: HtmlSpanElement = label.add_child("span")?;
        span.add_text(text)?;
        let input: HtmlInputElement = label.add_child("input")?;
        input.set_value(value);
        Ok(input)
    }

    fn remove(&self) -> Result<(), ElementRemoveError> {
        let _: Node = self
            .parent_element()
            .ok_or(ElementRemoveError::ParentElementNotFound)?
            .remove_child(self)
            .map_err(ElementRemoveError::RemoveChildError)?;
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum WindowError {
    #[error("window object does not exist")]
    WindowDoesNotExist,
}

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error(transparent)]
    WindowError(#[from] WindowError),
    #[error("window.document object does not exist")]
    DocumentDoesNotExist,
}

#[derive(Error, Debug)]
pub enum DocumentBodyError {
    #[error(transparent)]
    WindowError(#[from] WindowError),
    #[error(transparent)]
    DocumentError(#[from] DocumentError),
    #[error("window.document.body object does not exist")]
    BodyDoesNotExist,
}

#[derive(Error, Debug)]
pub enum ElementAddChildError {
    #[error(transparent)]
    DocumentError(#[from] DocumentError),
    #[error("create element failed: {0:?}")]
    CreateElementError(JsValue),
    #[error("append child failed: {0:?}")]
    AppendChildError(JsValue),
    #[error("Dynamic cast failed: {0:?}")]
    DynIntoError(Element),
}

#[derive(Error, Debug)]
pub enum ElementAddTextError {
    #[error(transparent)]
    DocumentError(#[from] DocumentError),
    #[error("append child failed: {0:?}")]
    AppendChildError(JsValue),
}

#[derive(Error, Debug)]
pub enum ElementAddInputError {
    #[error(transparent)]
    AddChildError(#[from] ElementAddChildError),
    #[error(transparent)]
    AddTextError(#[from] ElementAddTextError),
}

#[derive(Error, Debug)]
pub enum ElementRemoveError {
    #[error("parent element not found")]
    ParentElementNotFound,
    #[error("remove child failed: {0:?}")]
    RemoveChildError(JsValue),
}
