use crate::paths::SOCKET;
use anyhow::{Result, anyhow};
use tokio::{io::AsyncReadExt, net::UnixListener};

pub struct Listener {
    pub listener: UnixListener,
}

impl Listener {
    pub fn new() -> Result<Self> {
        let listener =
            UnixListener::bind(SOCKET).map_err(|e| anyhow!("Failed to bind to socket: {}", e))?;
        Ok(Listener { listener })
    }

    pub async fn accept(&self) -> Result<String> {
        match self.listener.accept().await {
            Ok((mut stream, _)) => {
                let mut data = String::new();
                AsyncReadExt::read_to_string(&mut stream, &mut data)
                    .await
                    .map_err(|e| anyhow!("Failed to read from stream: {}", e))?;
                data = data.trim().to_string();
                if data.is_empty() {
                    return Err(anyhow!("Received empty data"));
                }
                Ok(data)
            }
            Err(e) => Err(anyhow!("Failed to accept connection: {}", e)),
        }
    }

    pub fn socket(&self) -> String {
        SOCKET.to_string()
    }
}
