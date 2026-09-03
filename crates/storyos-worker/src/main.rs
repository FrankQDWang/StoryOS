use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--check") {
        return Ok(());
    }
    storyos_worker::run().await;
    Ok(())
}
