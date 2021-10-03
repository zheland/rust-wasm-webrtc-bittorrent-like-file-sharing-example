use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracker_protocol::FileSha256;

pub type FileLen = u64;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FileMetaData {
    sha256: FileSha256,
    name: String,
    len: u64,
}

impl FileMetaData {
    pub fn new(sha256: FileSha256, name: String, len: FileLen) -> Self {
        Self { sha256, name, len }
    }

    pub fn sha256(&self) -> FileSha256 {
        self.sha256
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn len(&self) -> FileLen {
        self.len
    }

    pub fn encode_base64(&self) -> Result<String, FileMetaDataEncodeBase64Error> {
        let encoded: Vec<u8> = bincode::serialize(&self)?;
        Ok(base64::encode(encoded))
    }

    pub fn decode_base64(base64: &str) -> Result<Self, FileMetaDataDecodeBase64Error> {
        let encoded = base64::decode(base64)?;
        Ok(bincode::deserialize(&encoded[..])?)
    }
}

#[derive(Error, Debug)]
pub enum FileMetaDataEncodeBase64Error {
    #[error(transparent)]
    SerializeError(#[from] bincode::Error),
}

#[derive(Error, Debug)]
pub enum FileMetaDataDecodeBase64Error {
    #[error(transparent)]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error(transparent)]
    DeserializeError(#[from] bincode::Error),
}
