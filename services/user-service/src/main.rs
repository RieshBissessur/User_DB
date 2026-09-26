#[tokio::main]
async fn main() {
    if let Err(err) = user_service::run().await {
        eprintln!("user-service failed to start: {err}");
        std::process::exit(1);
    }
}
