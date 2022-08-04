use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloRequest, HelloResponse};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

pub mod hello_world {
    tonic::include_proto!("hello");
}

#[derive(Debug, Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloResponse>, Status> {
        println!("Got a request: {:?}", request);
        let reply = HelloResponse {
            message: format!("Hello {}!", request.into_inner().name), // We must use .into_inner() as the fields of gRPC requests and responses are private
        };
        Ok(Response::new(reply)) // Send back our formatted greeting
    }

    type GetPeriodicHellosStream = ReceiverStream<Result<HelloResponse, Status>>;

    async fn get_periodic_hellos(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<Self::GetPeriodicHellosStream>, Status> {
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            let message = format!("Hello {}", request.get_ref().name);
            loop {
                let res = HelloResponse {
                    message: message.clone(),
                };
                if tx.send(Ok(res)).await.is_err() {
                    break;
                }
                time::sleep(Duration::from_secs(1)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::]:50051".parse()?;
    let greeter = MyGreeter::default();
    Server::builder()
        .add_service(GreeterServer::new(greeter))
        .serve(addr)
        .await?;
    Ok(())
}
