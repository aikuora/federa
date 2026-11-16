//! The runtime: the process that holds one person's core, apart from any window. It listens only
//! on 127.0.0.1 and serves the protocol to clients and the kernel's tools to agents, each behind a
//! token of its own: a client presents the one the runtime wrote where only that person can read
//! it, and an agent the one the kernel admitted it with.

mod connection;
mod data;
mod mcp;
mod person;
mod server;
mod token;

use std::{io, net::Ipv4Addr, process::ExitCode, sync::Arc};

use kernel::Kernel;
use tokio::net::TcpListener;

use crate::{data::DataFolder, token::Token};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("runtime: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> io::Result<()> {
    let data = DataFolder::locate()?;
    let person = person::identify(data.root())?;
    // Port 0: the system picks a free one, and clients learn it from `runtime.toml`.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let token = Token::generate();
    data.publish(listener.local_addr()?.port(), &token)
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "could not publish runtime.toml in {}: {e}",
                    data.root().display()
                ),
            )
        })?;
    // Without Claude the runtime still serves organizations and assets, and clients show why.
    let engine = claude::Claude::new("claude".into(), data.claude_config())
        .map(|claude| Arc::new(claude) as Arc<dyn engine::Engine>)
        .map_err(|e| e.to_string());
    server::serve(listener, token, Arc::new(Kernel::new(person, engine))).await
}
