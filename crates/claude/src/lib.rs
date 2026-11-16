//! The engine that runs agents on the official `claude` binary, speaking stream-json over stdio
//! with its sandbox always on. It reads only the fields it uses, so newer versions of the binary
//! work unchanged.

mod config;
mod launch;
mod session;
mod sign_in;
mod stream;
mod version;

use std::io;
use std::path::PathBuf;

use engine::{Agent, AgentSpec, Engine, Error};
use tokio::sync::mpsc;

pub struct Claude {
    binary: PathBuf,
    config: PathBuf,
}

impl Claude {
    /// Checks that `binary` runs and is at least the minimum version. Its agents run on `config`,
    /// a configuration folder of their own, where the person signs in once; this writes its
    /// settings.
    pub fn new(binary: PathBuf, config: PathBuf) -> Result<Self, Error> {
        let output = std::process::Command::new(&binary)
            .arg("--version")
            .output()
            .map_err(|_| Error::NotInstalled(binary.clone()))?;
        let printed = String::from_utf8_lossy(&output.stdout);
        let found = version::parse(&printed).ok_or_else(|| {
            let message = format!(
                "{} --version printed no version: {printed:?}",
                binary.display()
            );
            io::Error::new(io::ErrorKind::InvalidData, message)
        })?;
        if found < version::MINIMUM {
            return Err(Error::TooOld {
                found: found.to_string(),
                minimum: version::MINIMUM.to_string(),
            });
        }
        // The binary runs from each agent's workdir, so a relative folder would name another one
        // there. `binary` stays as given, so a bare name is looked up on `PATH`.
        let config = std::path::absolute(config)?;
        config::prepare(&config)?;
        Ok(Self { binary, config })
    }
}

impl Engine for Claude {
    /// Must be called within a Tokio runtime, which runs the agent.
    fn open(&self, spec: AgentSpec) -> Result<Agent, Error> {
        let child = launch::command(&self.binary, &self.config, &spec).spawn()?;
        let (input, messages) = mpsc::channel(8);
        let (events, receiver) = mpsc::channel(64);
        let task = tokio::spawn(session::run(child, messages, events));
        Ok(Agent::new(input, receiver, task))
    }
}
