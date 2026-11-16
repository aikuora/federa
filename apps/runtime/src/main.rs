//! The runtime: the process that holds one person's core, apart from any window, and serves it to
//! clients. It listens only on 127.0.0.1, admits only a client that presents the token it wrote
//! where only that person can read it, and hands every command to the kernel.

mod connection;
mod data;
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
    server::serve(listener, token, Arc::new(Kernel::new(person))).await
}
