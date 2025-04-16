use crate::paths::SOCKET;
use anyhow::{Result, anyhow};
use tokio::{io::AsyncWriteExt, net::UnixStream, runtime::Runtime};

pub struct Transmitter {
    runtime: Runtime,
}

impl Transmitter {
    pub fn new() -> Result<Self> {
        let runtime = Runtime::new().map_err(|e| anyhow!("Failed to create runtime: {}", e))?;
        Ok(Transmitter { runtime })
    }

    pub fn send(&mut self, message: &str) -> Result<()> {
        if !std::path::Path::new(SOCKET).exists() {
            return Err(anyhow!("Socket does not exist"));
        }

        self.runtime.block_on(async {
            let stream = UnixStream::connect(SOCKET)
                .await
                .map_err(|e| anyhow!("Failed to connect to socket: {}", e))?;
            let mut stream = stream;
            // Send the message
            stream
                .write_all(message.as_bytes())
                .await
                .map_err(|e| anyhow!("Failed to send message: {}", e))?;
            Ok(())
        })
    }
}
