use mini_runtime::executor::LocalExecutor;

fn main() {
    LocalExecutor::new().block_on(async { println!("hello") });
}
