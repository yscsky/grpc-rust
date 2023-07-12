use std::{error::Error, time::Duration};

pub mod pb {
    tonic::include_proto!("grpc.examples.echo");
}

use futures_core::Stream;
use pb::{echo_client::EchoClient, EchoRequest};
use tokio_stream::StreamExt;
use tonic::transport::Channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = EchoClient::connect("http://[::1]:50051").await?;

    println!("Streaming echo:");
    streaming_echo(&mut client, 5).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("\nBidirectional stream echo:");
    bidirectional_streaming_echo(&mut client, 17).await;

    println!("\nBidirectional stream echo (kill client with CTLR+C):");
    bidirectional_streaming_echo_throttle(&mut client, Duration::from_secs(2)).await;

    Ok(())
}

async fn streaming_echo(client: &mut EchoClient<Channel>, num: usize) {
    let stream = client
        .server_streaming_echo(EchoRequest {
            message: "foo".into(),
        })
        .await
        .unwrap()
        .into_inner();
    let mut stream = stream.take(num);
    while let Some(item) = stream.next().await {
        println!("\treceived: {}", item.unwrap().message);
    }
}

async fn bidirectional_streaming_echo(client: &mut EchoClient<Channel>, num: usize) {
    let in_stream = echo_requesets_iter().take(num);
    let response = client
        .bidirectional_streaming_echo(in_stream)
        .await
        .unwrap();
    let mut resp_stream = response.into_inner();
    while let Some(received) = resp_stream.next().await {
        let received = received.unwrap();
        println!("\treceived message: `{}`", received.message);
    }
}

fn echo_requesets_iter() -> impl Stream<Item = EchoRequest> {
    tokio_stream::iter(1..usize::MAX).map(|i| EchoRequest {
        message: format!("msg: {:02}", i),
    })
}

async fn bidirectional_streaming_echo_throttle(client: &mut EchoClient<Channel>, dur: Duration) {
    let in_stream = echo_requesets_iter().throttle(dur);
    let response = client
        .bidirectional_streaming_echo(in_stream)
        .await
        .unwrap();
    let mut resp_stream = response.into_inner();
    while let Some(received) = resp_stream.next().await {
        let received = received.unwrap();
        println!("\treceived message: `{}`", received.message);
    }
}
