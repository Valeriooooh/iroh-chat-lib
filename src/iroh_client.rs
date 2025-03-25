use std::{path::PathBuf, pin::Pin, str::FromStr, sync::Arc};

use iroh_blobs::util::local_pool::LocalPool;
use iroh_docs::{
    AuthorId, DocTicket,
    rpc::{AddrInfoOptions, client::docs::ShareMode},
};
use iroh_gossip::RpcClient;
use quic_rpc::transport::flume::FlumeConnector;

use crate::{
    client::{ChatC, ChatClient},
    message::Message,
};

pub type BlobsClient = iroh_blobs::rpc::client::blobs::Client<
    FlumeConnector<iroh_blobs::rpc::proto::Response, iroh_blobs::rpc::proto::Request>,
>;
pub type DocsClient = iroh_docs::rpc::client::docs::Client<
    FlumeConnector<iroh_docs::rpc::proto::Response, iroh_docs::rpc::proto::Request>,
>;
pub type GossipClient =
    RpcClient<FlumeConnector<iroh_gossip::rpc::proto::Response, iroh_gossip::rpc::proto::Request>>;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Iroh {
    _local_pool: Arc<iroh_blobs::util::local_pool::LocalPool>,
    router: iroh::protocol::Router,
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
            .build(&local_pool, builder.endpoint());

        // add docs
        let docs = iroh_docs::protocol::Docs::persistent(path)
            .spawn(&blobs, &gossip)
            .await?;

        builder = builder
            .accept(iroh_gossip::ALPN, Arc::new(gossip.clone()))
            .accept(iroh_blobs::ALPN, blobs.clone())
            .accept(iroh_docs::ALPN, Arc::new(docs.clone()));
        let gc = gossip.client().clone();

        Ok(Self {
            _local_pool: Arc::new(local_pool),
            router: builder.spawn().await?,
            gossip: gc,
            blobs: blobs.client().clone(),
            docs: docs.client().clone(),
            author: docs.client().authors().default().await?,
        })
    }

    pub async fn create_chat(&self) -> anyhow::Result<ChatClient> {
        let a = self.clone();
        let temp_doc = a.docs.create().await?;

        let ticket = temp_doc
            .share(ShareMode::Write, AddrInfoOptions::RelayAndAddresses)
            .await?;

        let (chat, sub): (ChatC, _) = a.docs.import_and_subscribe(ticket.clone()).await.unwrap();
        let sub = Pin::new(Box::new(sub));
        let test: Vec<u8> =
            bincode::serialize(&Message::set_ticket(a.author, ticket.to_string())).unwrap();
        let _ = chat.set_bytes(a.author, "chat-ticket", test).await;
        println!("share this ticket to your friend: {}", ticket);
        Ok(ChatClient { chat, sub })
    }

    pub async fn join_chat(&self, ticket: String) -> Option<anyhow::Result<ChatClient>> {
        let a = self.clone();
        let Ok(doct) = DocTicket::from_str(ticket.as_str()) else {
            return None;
        };
        let Ok((chat, sub)) = a.docs.import_and_subscribe(doct).await else {
            return None;
        };
        let sub = Pin::new(Box::new(sub));
        let _ = chat
            .set_bytes(
                a.author,
                "chat-ticket",
                bincode::serialize(&Message::set_ticket(a.author, ticket.clone())).unwrap(),
            )
            .await;
        Some(Ok(ChatClient { chat, sub }))
    }
}
