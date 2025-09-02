mod crypto;
mod models;
mod vault;
mod cli;
mod debug;
mod tui;
mod ssh;

use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    sodiumoxide::init().expect("Failed to initialize sodiumoxide");

    // Check for debug flag
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "debug" {
        debug::debug_vault();
        return Ok(());
    }

    let mut handler = cli::CliHandler::new()?;
    handler.run().await
}
