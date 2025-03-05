use iroh_docs::AuthorId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    TextMessage { author: AuthorId, content: String },
    BlobMessage { author: AuthorId, content: Vec<u8> },
    AuthorMessage { author: AuthorId, content: String },
    ChatTicket { author: AuthorId, content: String },
}

impl Message {
    pub fn new_text(author: AuthorId, content: String) -> Self {
        Self::TextMessage { author, content }
    }
    pub fn new_blob(author: AuthorId, content: &[u8]) -> Self {
        Self::BlobMessage {
            author,
            content: content.to_vec(),
        }
    }
    pub fn set_username(author: AuthorId, content: String) -> Self {
        Self::AuthorMessage { author, content }
    }
    pub fn set_ticket(author: AuthorId, content: String) -> Self {
        Self::ChatTicket { author, content }
    }
}
