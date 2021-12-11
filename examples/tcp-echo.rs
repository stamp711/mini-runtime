use futures::{io::copy, AsyncRead, AsyncReadExt, AsyncWrite, StreamExt};
use mini_runtime::{executor::LocalExecutor, net::TcpListener};

fn main() {
    let runtime = LocalExecutor::new();
    runtime.block_on(server());
}

async fn server() {
    let mut listener = TcpListener::bind("127.0.0.1:8000").expect("cannot bind to address");
    while let Some(stream) = listener.next().await {
        LocalExecutor::spawn(echo(stream));
    }
}

async fn echo(stream: impl AsyncRead + AsyncWrite) {
    println!("new echo stream");
    let (rh, mut wh) = stream.split();
    let _ = copy(rh, &mut wh).await;
}
