use std::future::pending;

use mini_runtime::executor::LocalExecutor;

fn main() {
    LocalExecutor::new().block_on(f());
}

async fn f() {
    println!("f.begin");
    LocalExecutor::spawn_with_name(g(), "g");
    pending::<()>().await;
    println!("f.end");
}

async fn g() {
    println!("g.");
}
