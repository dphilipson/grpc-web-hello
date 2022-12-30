use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::codegen::http::Method;
use tonic::{transport::Server, Request, Response, Status};
use tonic_web::GrpcWebLayer;
use tower_http::cors::{Any, CorsLayer};

use hello_world::subscription_counter_server::{SubscriptionCounter, SubscriptionCounterServer};
use hello_world::{
    SubscribeRequest, SubscribeUpdate, SubscriptionCountRequest, SubscriptionCountResponse,
};

pub mod hello_world {
    tonic::include_proto!("hello");
}

#[derive(Debug, Default)]
pub struct MySubscriptionCounter {
    next_id: AtomicU64,
    contexts_by_id: Arc<RwLock<HashMap<SubscriptionId, SubscriptionContext>>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct SubscriptionId(u64);

#[derive(Debug)]
struct SubscriptionContext {
    sender: mpsc::Sender<Result<SubscribeUpdate, Status>>,
}

fn broadcast_counts(contexts_by_id: &mut HashMap<SubscriptionId, SubscriptionContext>) {
    let count = contexts_by_id.len() as u32;
    for (id, context) in contexts_by_id.iter() {
        let update = Ok(SubscribeUpdate { count });
        if let Err(err) = context.sender.try_send(update) {
            match err {
                TrySendError::Full(_) => {
                    eprintln!("Buffer was full for subscription {:?}.", id)
                }
                TrySendError::Closed(_) => {
                    println!("Channel was closed for subscription {:?}.", id)
                }
            }
        }
        // TODO: Collect the closed ones for removal. Do we need this?
    }
}

#[tonic::async_trait]
impl SubscriptionCounter for MySubscriptionCounter {
    async fn get_subscription_count(
        &self,
        _: Request<SubscriptionCountRequest>,
    ) -> Result<Response<SubscriptionCountResponse>, Status> {
        let count = self.contexts_by_id.read().await.len() as u32;
        Ok(Response::new(SubscriptionCountResponse { count }))
    }

    type SubscribeStream = DropStream<SubscribeUpdate>;

    async fn subscribe(
        &self,
        _: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let id = SubscriptionId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let (update_sender, update_receiver) = mpsc::channel(4);
        {
            let mut contexts_by_id = self.contexts_by_id.write().await;
            contexts_by_id.insert(
                id,
                SubscriptionContext {
                    sender: update_sender,
                },
            );
            broadcast_counts(&mut contexts_by_id)
        }
        let (drop_sender, drop_receiver) = oneshot::channel();
        let contexts_by_id = self.contexts_by_id.clone();
        tokio::spawn(async move {
            if drop_receiver.await.is_err() {
                eprintln!("The receiver dropped, but the sender dropped first somehow.");
                return;
            }
            let mut contexts_by_id = contexts_by_id.write().await;
            contexts_by_id.remove(&id);
            broadcast_counts(&mut contexts_by_id);
        });
        Ok(Response::new(DropStream::new(update_receiver, drop_sender)))
    }
}

/// The way to detect that the client has disconnected from a server-side stream
/// in Tonic is that the receiver stream is dropped. This type helps us work
/// with this interface, by delegating to a plain ReceiverStream, but also
/// using a oneshot channel to notify when this is dropped.
pub struct DropStream<T> {
    delegate: ReceiverStream<Result<T, Status>>,
    sender: Option<oneshot::Sender<()>>,
}

impl<T> DropStream<T> {
    pub fn new(
        update_receiver: mpsc::Receiver<Result<T, Status>>,
        drop_sender: oneshot::Sender<()>,
    ) -> Self {
        DropStream {
            delegate: ReceiverStream::new(update_receiver),
            sender: Some(drop_sender),
        }
    }
}

impl<T> Stream for DropStream<T> {
    type Item = Result<T, Status>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.delegate).poll_next(cx)
    }
}

impl<T> Drop for DropStream<T> {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::]:50051".parse()?;
    println!("Starting server at {}.", addr);
    let subscription_counter = MySubscriptionCounter::default();
    let cors_layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any);
    Server::builder()
        .accept_http1(true)
        .layer(cors_layer)
        .layer(GrpcWebLayer::new())
        .add_service(SubscriptionCounterServer::new(subscription_counter))
        .serve(addr)
        .await?;
    Ok(())
}
