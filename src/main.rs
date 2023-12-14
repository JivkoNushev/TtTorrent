use interprocess::local_socket::LocalSocketListener;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use std::io::Read;

use torrent_client::client::Client;
use torrent_client::client::messager::InterProcessMessage;

fn save_state() {
    // todo!()
}

fn setup_graceful_shutdown() {
    use tokio::signal::unix::SignalKind;

    // Set up a Ctrl+C signal handler (SIGINT)
    let ctrl_c = tokio::signal::ctrl_c();

    // Set up a termination signal handler (SIGTERM)
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    let mut sigquit = tokio::signal::unix::signal(SignalKind::quit()).unwrap();

    // Spawn an async task to handle the signals
    tokio::spawn(async move {
        tokio::select! {
            _ = ctrl_c => {
                // Handle Ctrl+C (SIGINT)
                println!("Ctrl+C received. Cleaning up...");
            },
            _ = sigterm.recv() => {
                // Handle SIGTERM
                println!("SIGTERM received. Cleaning up...");
            },
            _ = sigint.recv() => {
                // Handle SIGINT
                println!("SIGINT received. Cleaning up...");
            },
            _ = sigquit.recv() => {
                // Handle SIGQUIT
                println!("SIGQUIT received. Cleaning up...");
            }
        }

        // Perform cleanup or graceful shutdown logic here
        save_state();

        // Terminate the program
        std::process::exit(0);
    });
}


#[tokio::main] // flavor = "current_thread" ( maybe use one thread if its faster )
async fn main() {
    setup_graceful_shutdown();

    let (client_tx, client_rx) = mpsc::channel::<InterProcessMessage>(100);

    let client = Client::new(client_tx.clone(), client_rx);
    tokio::spawn(client.run());
    
    // remove socket if it exists and create new one
    let _ = tokio::fs::remove_file("/tmp/TtTClient.sock").await;
    let client_socket = LocalSocketListener::bind("/tmp/TtTClient.sock").unwrap();
    
    // accept commands
    let mut stream = tokio_stream::iter(client_socket.incoming());
    while let Some(Ok(mut s)) = stream.next().await {
        let mut message = Vec::new();
        let _ = s.read_to_end(&mut message).unwrap();

        let message = match serde_json::from_slice::<InterProcessMessage>(&message) {
            Ok(message) => message,
            Err(e) => {
                println!("Failed to deserialize message: {}", e);
                continue;
            }
        };

        let _ = client_tx.send(message).await;
    }
}