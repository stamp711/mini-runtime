use futures::io::copy;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, StreamExt};
use mini_runtime::executor::LocalExecutor;
use mini_runtime::net::TcpListener;

fn main() {
    let runtime = LocalExecutor::new();
    runtime.block_on(server());
}

async fn server() {
    let mut listener = TcpListener::bind("127.0.0.1:8000").expect("cannot bind to address");
    while let Some(Ok((stream, addr))) = listener.next().await {
        let name = format!("echo-{}", addr);
        LocalExecutor::spawn_with_name(echo(stream), name);
    }
}

async fn echo(stream: impl AsyncRead + AsyncWrite) {
    let (rh, mut wh) = stream.split();
    let _ = copy(rh, &mut wh).await;
}
