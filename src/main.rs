use clap::Parser;

use github_fetcher_mcp::{cli::Args, run};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    run(args).await
}
