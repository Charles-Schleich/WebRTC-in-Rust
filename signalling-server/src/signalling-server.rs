#[macro_use]
extern crate log;
extern crate simplelog;

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::prelude::*;
use std::{collections::hash_map, fmt::Error, fs::File, process};

use log::{warn, SetLoggerError};
use simplelog::{CombinedLogger, LevelFilter, TermLogger, TerminalMode, WriteLogger};

use std::{
    collections::HashMap,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures::prelude::*;
use futures::{
    channel::mpsc::{unbounded, UnboundedSender},
    future, pin_mut,
};

use async_std::net::{TcpListener, TcpStream};
use async_std::task;
use async_tungstenite::tungstenite::protocol::Message;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

// Type Alias
type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

// Constants
const PORT: &str = "3000";
const LOG_FILE: &str = "signalling_server_prototype.log";

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Setup Logging
fn setup_logging() -> Result<(), SetLoggerError> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        TerminalMode::Mixed,
    )])
}

// Get Server IP
use std::net::UdpSocket;
pub fn get_local_ip() -> Option<String> {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };
    match socket.connect("8.8.8.8:80") {
        Ok(()) => (),
        Err(_) => return None,
    };
    match socket.local_addr() {
        Ok(addr) => return Some(addr.ip().to_string()),
        Err(_) => return None,
    };
}

//   _    _                       _   _             _____                                         _     _
//  | |  | |                     | | | |           / ____|                                       | |   (_)
//  | |__| |   __ _   _ __     __| | | |   ___    | |        ___    _ __    _ __     ___    ___  | |_   _    ___    _ __
//  |  __  |  / _` | | '_ \   / _` | | |  / _ \   | |       / _ \  | '_ \  | '_ \   / _ \  / __| | __| | |  / _ \  | '_ \
//  | |  | | | (_| | | | | | | (_| | | | |  __/   | |____  | (_) | | | | | | | | | |  __/ | (__  | |_  | | | (_) | | | | |
//  |_|  |_|  \__,_| |_| |_|  \__,_| |_|  \___|    \_____|  \___/  |_| |_| |_| |_|  \___|  \___|  \__| |_|  \___/  |_| |_|

async fn handle_connection(peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr) {
    info!("Incoming TCP connection from: {}", addr);

    let ws_stream = async_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("WebSocket connection established: {}", addr);

    // Insert the write part of this peer to the peer map.
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx.clone());

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming
        .try_filter(|msg| {
            // Broadcasting a Close message from one client
            // will close the other clients.
            future::ready(!msg.is_close())
        })
        .try_for_each(|msg| {
            warn!(
                "Received a message from {}: {}",
                addr,
                msg.to_text().unwrap()
            );

            let peers = peer_map.lock().unwrap();
            // We want to broadcast the message to everyone except ourselves.
            let broadcast_recipients = peers
                .iter()
                .filter(|(peer_addr, _)| peer_addr != &&addr)
                .map(|(_, ws_sink)| ws_sink);

            for recp in broadcast_recipients {
                recp.unbounded_send(msg.clone()).unwrap();
            }

            future::ok(())
        });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    info!("{} disconnected", &addr);

    // Remove from peer map
    peer_map.lock().unwrap().remove(&addr);
}

async fn run() -> Result<(), IoError> {
    let mut addr = get_local_ip().expect("Couldn't get IP");
    addr.push_str(":2794");
    let peer_map = PeerMap::new(Mutex::new(HashMap::new()));

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;

    let listener = try_socket.expect("Failed to bind");

    info!("Listening on: {}", addr);

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, addr)) = listener.accept().await {
        task::spawn(handle_connection(peer_map.clone(), stream, addr));
    }
    Ok(())
}

fn main() -> Result<(), IoError> {
    // Setup Basic Logging
    match setup_logging() {
        Ok(_) => (),
        Err(e) => {
            println!("Could not start logger,{}\n...exiting", e);
            std::process::exit(1);
        }
    }

    task::block_on(run())
}
