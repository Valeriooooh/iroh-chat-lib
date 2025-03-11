use std::{pin::Pin, sync::Arc, thread, time::Duration};

use quic_rpc::transport::flume::{FlumeConnector};

use futures_lite::{Stream, StreamExt};
use iroh_docs::{
    engine::LiveEvent, rpc::{
        client::docs::Doc,
        proto::{Request, Response},
    }, store::{Query, QueryBuilder}, AuthorId
};

use crate::{iroh_client::Iroh, message::Message};

pub(crate) type ChatC =  Doc<FlumeConnector<Response, Request>>;
pub(crate) type SubC = Pin<Box<dyn Stream<Item = Result<LiveEvent, anyhow::Error> > + Send  + 'static>>;


pub(crate) struct ChatClient {
    pub chat: ChatC,
    pub sub: SubC

}

impl ChatClient{
    pub async fn send_message(&mut self, author: AuthorId, msg:Message) -> Result<(), ChatError>{
        match self.chat.set_bytes(
            author,
            std::time::UNIX_EPOCH.elapsed().unwrap().as_micros().to_string(),
            bincode::serialize(&msg).unwrap()
        ).await {
            Ok(_) => Ok(()),
            Err(_) =>  Err(ChatError::SendError)
        }
    }

    pub async fn set_author_name(&mut self, author: AuthorId, name: String) -> Result<(), ChatError>{
        let _a = self.chat.set_bytes(author, author.to_string(), name.clone()).await;
        //let _ = self.send_message(author, Message::AuthorMessage { author , content: name }).await;
        Ok(())
    }

    pub async fn get_author_name(&mut self, author: AuthorId, iroh : Arc<Iroh>) -> Result<String, AuthorId>{
        let blobs = iroh.blobs.clone();
            if let Some(b)  = self.chat.get_one(Query::key_exact(author.to_string())).await.unwrap(){
                let name = blobs.read_to_bytes(b.content_hash()).await.unwrap();
                let name = String::from_utf8(name.to_vec()).unwrap();
                return Ok(name);
        };

        Err(author)
    }
}

#[derive(Debug)]
pub enum ChatError{
    SendError,
}



impl ChatClient{
    pub async fn message_receiver_loop(
        &mut self,
        iroh: Arc<Iroh>,
    ) -> Result<Message , ChatError>{
        let blobs = iroh.blobs.clone();
        while let Ok(e) = self.sub.try_next().await {
            if let Some(LiveEvent::InsertRemote { entry, .. }) = e {
                let message_content = blobs.read_to_bytes(entry.content_hash()).await;
                match message_content {
                    Ok(a) => {
                                if let Ok(a) = bincode::deserialize(&a){
                                    return Ok(a);
                                }
                    }
                    Err(_) => {
                        // may still be syncing so
                        for _ in 0..3 {
                            thread::sleep(Duration::from_secs(1));
                            let message_content = blobs.read_to_bytes(entry.content_hash()).await;
                            if let Ok(a) = message_content {
                                if let Ok(a) = bincode::deserialize(&a){
                                    return Ok(a);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(ChatError::SendError)
    }

}
