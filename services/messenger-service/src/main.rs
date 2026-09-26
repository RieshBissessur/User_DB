#[tokio::main]
async fn main() {
    if let Err(err) = messenger_service::run().await {
        eprintln!("messenger-service failed to start: {err}");
        std::process::exit(1);
    }
}
