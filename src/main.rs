use std::{path::PathBuf, str::FromStr, sync::Arc, thread, time::Duration};

use dialoguer::{Input, theme::ColorfulTheme};
use futures_lite::{Stream, StreamExt};
use iroh::protocol::Router;
use iroh_blobs::util::local_pool::LocalPool;
use iroh_docs::{
    AuthorId, DocTicket,
    engine::LiveEvent,
    rpc::{
        client::docs::{Doc, ShareMode},
        proto::{Request, Response},
    },
};
use message::Message;
use quic_rpc::transport::flume::FlumeConnector;
use tokio::pin;
mod message;

pub(crate) type BlobsClient = iroh_blobs::rpc::client::blobs::Client<
    FlumeConnector<iroh_blobs::rpc::proto::Response, iroh_blobs::rpc::proto::Request>,
>;
pub(crate) type DocsClient = iroh_docs::rpc::client::docs::Client<
    FlumeConnector<iroh_docs::rpc::proto::Response, iroh_docs::rpc::proto::Request>,
>;
pub(crate) type GossipClient = iroh_gossip::rpc::client::Client<
    FlumeConnector<iroh_gossip::rpc::proto::Response, iroh_gossip::rpc::proto::Request>,
>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let i = Arc::new(Iroh::new(".iroh-dir".into()).await?);
    println!("author: {}", i.author.fmt_short());
    let _ = tokio::spawn(menu_loop(i)).await;
    Ok(())
}

async fn menu_loop(node: Arc<Iroh>) {
    loop {
        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Commmand:")
            .interact_text()
            .unwrap();
        match input.as_str() {
            "create" => {
                let (chat, sub) = node.create_chat().await.unwrap();
                let nodec = node.clone();
                tokio::spawn(chat_loop((chat.clone(), Box::pin(sub)), nodec.clone()));
                loop {
                    let line: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Chat:")
                        .interact_text()
                        .unwrap();
                    let timestamp = std::time::UNIX_EPOCH.elapsed().unwrap().as_micros();
                    let _ = chat
                        .set_bytes(
                            node.author,
                            timestamp.to_string(),
                            bincode::serialize(&Message::new_text(node.author, line)).unwrap(),
                        )
                        .await
                        .unwrap();
                }
            }
            "join" => {
                let join_ticket: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("ticket:")
                    .interact_text()
                    .unwrap();
                // trim extra spaces
                let join_ticket = join_ticket.trim();
                let (chat, sub) = node.join_chat(join_ticket.to_string()).await.unwrap();
                tokio::spawn(chat_loop((chat.clone(), sub), node.clone()));
                loop {
                    let line: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Chat:")
                        .interact_text()
                        .unwrap();

                    let timestamp = std::time::UNIX_EPOCH.elapsed().unwrap().as_micros();
                    let _ = chat
                        .set_bytes(
                            node.author,
                            timestamp.to_string(),
                            bincode::serialize(&Message::new_text(node.author, line)).unwrap(),
                        )
                        .await
                        .unwrap();
                }
            }
            _ => {}
        }
    }
}

async fn chat_loop(
    (_, sub): (
        Doc<FlumeConnector<Response, Request>>,
        impl Stream<Item = Result<LiveEvent, anyhow::Error>> + 'static,
    ),
    iroh: Arc<Iroh>,
) {
    pin! {sub};
    let blobs = iroh.blobs.clone();
    while let Ok(e) = sub.try_next().await {
        if let Some(LiveEvent::InsertRemote { entry, .. }) = e {
            let message_content = blobs.read_to_bytes(entry.content_hash()).await;
            match message_content {
                Ok(a) => {
                    let msg: Message = bincode::deserialize(&a).unwrap();
                    match msg {
                        Message::TextMessage { author, content } => {
                            println!("{}: {}", author.fmt_short(), content);
                        }
                        Message::ChatTicket { .. } => {}
                        _ => {}
                    }
                }
                Err(_) => {
                    // may still be syncing so
                    for _ in 0..3 {
                        thread::sleep(Duration::from_secs(1));
                        let message_content = blobs.read_to_bytes(entry.content_hash()).await;
                        if let Ok(a) = message_content {
                            let msg: Message = bincode::deserialize(&a).unwrap();
                            match msg {
                                Message::TextMessage { author, content } => {
                                    println!("{}: {}", author.fmt_short(), content);
                                }
                                Message::ChatTicket { author, content } => {}
                                _ => {}
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct Iroh {
    _local_pool: Arc<LocalPool>,
    router: Router,
    pub(crate) gossip: GossipClient,
    pub(crate) blobs: BlobsClient,
    pub(crate) docs: DocsClient,
    pub(crate) author: AuthorId,
}

impl Iroh {
    pub async fn new(path: PathBuf) -> Result<Self, anyhow::Error> {
        // create dir if it doesn't already exist
        tokio::fs::create_dir_all(&path).await?;

        let key = iroh_blobs::util::fs::load_secret_key(path.clone().join("keypair")).await?;

        // local thread pool manager for blobs
        let local_pool = LocalPool::default();

        // create endpoint
        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .secret_key(key)
            .bind()
            .await?;

        // build the protocol router
        let mut builder = iroh::protocol::Router::builder(endpoint);

        // add iroh gossip
        let gossip = iroh_gossip::net::Gossip::builder()
            .spawn(builder.endpoint().clone())
            .await?;

        // add iroh blobs
        let blobs = iroh_blobs::net_protocol::Blobs::persistent(&path)
            .await?
            .build(local_pool.handle(), builder.endpoint());

        // add docs
        let docs = iroh_docs::protocol::Docs::persistent(path)
            .spawn(&blobs, &gossip)
            .await?;

        builder = builder
            .accept(iroh_gossip::ALPN, Arc::new(gossip.clone()))
            .accept(iroh_blobs::ALPN, blobs.clone())
            .accept(iroh_docs::ALPN, Arc::new(docs.clone()));

        Ok(Self {
            _local_pool: Arc::new(local_pool),
            router: builder.spawn().await?,
            gossip: gossip.client().clone(),
            blobs: blobs.client().clone(),
            docs: docs.client().clone(),
            author: docs.client().authors().default().await?,
        })
    }

    async fn create_chat(
        &self,
    ) -> anyhow::Result<(
        Doc<FlumeConnector<Response, Request>>,
        impl Stream<Item = Result<LiveEvent, anyhow::Error>> + 'static,
    )> {
        let a = self.clone();
        let temp_doc = a.docs.create().await?;

        let ticket = temp_doc
            .share(
                ShareMode::Write,
                iroh_docs::rpc::AddrInfoOptions::RelayAndAddresses,
            )
            .await?;

        let (chat, sub) = a.docs.import_and_subscribe(ticket.clone()).await.unwrap();
        let test: Vec<u8> =
            bincode::serialize(&Message::set_ticket(a.author, ticket.to_string())).unwrap();
        let _ = chat.set_bytes(a.author, "chat-ticket", test).await;
        println!("share this ticket to your friend: {}", ticket);
        Ok((chat, sub))
    }

    async fn join_chat(
        &self,
        ticket: String,
    ) -> anyhow::Result<(
        Doc<FlumeConnector<Response, Request>>,
        impl Stream<Item = Result<LiveEvent, anyhow::Error>> + 'static,
    )> {
        let a = self.clone();
        let doct = DocTicket::from_str(ticket.as_str())?;
        let (chat, sub) = a.docs.import_and_subscribe(doct).await?;
        let _ = chat
            .set_bytes(
                a.author,
                "chat-ticket",
                bincode::serialize(&Message::set_ticket(a.author, ticket.clone())).unwrap(),
            )
            .await;
        Ok((chat, sub))
    }
}
