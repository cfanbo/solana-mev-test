use dotenv;
use mybot::engine::Engine;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let engine = Engine::new().await;
    engine.run().await.unwrap();
}
