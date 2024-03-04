use futures::{AsyncBufReadExt, AsyncWriteExt, AsyncReadExt};
use anyhow::{Result, anyhow};
use interprocess::local_socket::tokio::LocalSocketStream;

use std::process::exit;

use crate::messager::TerminalClientMessage;

pub struct TerminalClient {
    pub socket: LocalSocketStream,
    pub pid: u32,
}

impl TerminalClient {
    pub async fn recv_message(&mut self) -> Result<TerminalClientMessage> {
        let mut reader = futures::io::BufReader::new(&mut self.socket);

        let mut buffer = String::new();
        let bytes_read = reader.read_line(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow!("Failed to read message from socket"));
        }

        let message = serde_json::from_slice(buffer.as_bytes())?;
        Ok(message)
    }

    pub async fn recv_buffered_message(&mut self) -> Result<TerminalClientMessage> {
        let mut buffer = Vec::new();
        let bytes_read = self.socket.read_to_end(&mut buffer).await?;

        if bytes_read == 0 {
            return Err(anyhow!("Failed to read message from socket"));
        }

        let message = serde_json::from_slice(&buffer)?;
        Ok(message)
    }

    pub async fn send_message(&mut self, message: &TerminalClientMessage) -> Result<()> {
        let message = create_message(message);
        self.socket.write_all(&message).await?;

        Ok(())
    }

    pub async fn send_buffered_message(&mut self, message: &TerminalClientMessage) -> Result<()> {
        let message = serde_json::to_string(&message).expect("Serialization failed");
        self.socket.write_all(message.as_bytes()).await?;

        Ok(())
    }
}

pub async fn create_client_socket() -> LocalSocketStream {
    let path = unsafe { crate::CLIENT_OPTIONS.socket_path.clone() };
    let path = std::path::Path::new(&path);
    
    match LocalSocketStream::connect(path).await {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("[Error] Failed to connect to the client: {}", e);
            exit(1);
        }
    }
}

pub fn create_message (message: &TerminalClientMessage) -> Vec<u8> {
    let mut serialized_data = serde_json::to_string(message).expect("Serialization failed");
    serialized_data.push('\n');
    serialized_data.as_bytes().to_vec()
}