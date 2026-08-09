use std::env;
use std::io::{self, Write as _};

use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind_address = bind_address()?;
    let listener = TcpListener::bind(&bind_address).await?;
    let address = listener.local_addr()?;
    println!("STORYOS_SERVER_URL=http://{address}");
    io::stdout().flush()?;
    axum::serve(listener, storyos_server::router()).await?;
    Ok(())
}

fn bind_address() -> Result<String, String> {
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => Ok("127.0.0.1:3000".to_owned()),
        [flag, address] if flag == "--bind" => Ok(address.clone()),
        _ => Err("usage: storyos-server [--bind <address>]".to_owned()),
    }
}
