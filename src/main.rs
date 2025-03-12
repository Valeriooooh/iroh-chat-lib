use client::ChatClient;
use dialoguer::{Input, theme::ColorfulTheme};
use futures_lite::StreamExt;
use iroh_client::Iroh;
use message::Message;
use std::sync::Arc;
use tokio::sync::mpsc;
mod client;
mod iroh_client;
mod message;

enum ChatType {
    None,
    Join,
    Create,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let i = Arc::new(Iroh::new(".iroh-dir".into()).await?);
    println!("author: {}", i.author.fmt_short());
    let mut ct = ChatType::None;
    while let ChatType::None = ct {
        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Commmand:")
            .interact_text()
            .unwrap();
        match input.as_str() {
            "create" => {
                ct = ChatType::Create;
            }
            "join" => {
                ct = ChatType::Join;
            }
            "show" => {
                let docsa = i.docs.list().await.unwrap();
                println!("{:?}", docsa.count().await)
            }
            _ => {}
        }
    }
    match ct {
        ChatType::Join => ui_join(i).await,
        ChatType::Create => ui_create(i).await,
        _ => {}
    }
    Ok(())
}

async fn ui_create(node: Arc<Iroh>) {
    let client = node.create_chat().await.unwrap();
    ui_chat(client, node).await
}

async fn ui_join(node: Arc<Iroh>) {
    let join_ticket: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("ticket:")
        .interact_text()
        .unwrap();
    // trim extra spaces
    let join_ticket = join_ticket.trim();
    let Ok(mut client) = node.join_chat(join_ticket.to_string()).await.unwrap() else {
        panic!("")
    };
    ui_chat(client, node).await
}

async fn ui_chat(mut client: ChatClient, node: Arc<Iroh>) {
    let (tx1, mut rx1) = mpsc::channel(32);
    tokio::spawn(async move {
        loop {
            let line: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Chat:")
                .interact_text()
                .unwrap();
            let _ = tx1.send(line).await;
        }
    });

    loop {
        tokio::select! {
            line = rx1.recv() => {
                let line = line.unwrap();
                if line.clone().starts_with("set name "){
                    let name = line.split(" ").last().unwrap();
                    let _ = client.set_author_name(node.clone().author, name.to_string()).await;
                }else{
                    let _ = client.send_message(node.clone().author, Message::new_text(node.author, line.clone())).await;
                }
            }
            Ok(msg) = client.message_receiver_loop(node.clone()) => {
                match msg{
                Message::TextMessage { author, content } => {
                    let name = client.get_author_name(author, node.clone()).await;
                    match name{
                        Ok(a) => println!("{}:{}", a, content.to_string()),
                        Err(_) => println!("{}:{}", author.fmt_short(), content.to_string()),
                    }
                                    }
                Message::ChatTicket{
                    author, ..
                } => {
                    println!("\n{} joined!!",author.fmt_short())
                }
                _ => {}
            }
            }
        }
    }
}

