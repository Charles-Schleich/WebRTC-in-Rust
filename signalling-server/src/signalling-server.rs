#[macro_use]
extern crate log;
extern crate simplelog;

use std::fs::File;

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

// From Workspace
use shared_protocol::*;

// Type Alias
type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;
type UserList = Arc<Mutex<HashMap<UserID, SocketAddr>>>;

// type UserID      = String;
type SessionList = Arc<Mutex<HashMap<SessionID, SessionMembers>>>;

// Constants
const LOG_FILE: &str = "signalling_server_prototype.log";

#[derive(Debug, Clone)]
struct SessionMembers {
    host: UserID,
    guest: Option<UserID>,
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
// Setup Logging
fn setup_logging() -> Result<(), SetLoggerError> {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            TerminalMode::Mixed,
        ),
        WriteLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            File::create(LOG_FILE).unwrap(),
        ),
    ])
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
        Ok(addr) => Some(addr.ip().to_string()),
        Err(_) => None,
    }
}

fn generate_id(length: u8) -> String {
    let rand_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length as usize)
        .map(char::from)
        .collect();
    println!("{}", rand_string);
    rand_string
}

type PeerID = UserID;

#[derive(Debug, Clone)]
enum Destination {
    SourcePeer,
    OtherPeer(PeerID),
}

//   _    _                       _   _            __  __
//  | |  | |                     | | | |          |  \/  |
//  | |__| |   __ _   _ __     __| | | |   ___    | \  / |   ___   ___   ___    __ _    __ _    ___
//  |  __  |  / _` | | '_ \   / _` | | |  / _ \   | |\/| |  / _ \ / __| / __|  / _` |  / _` |  / _ \
//  | |  | | | (_| | | | | | | (_| | | | |  __/   | |  | | |  __/ \__ \ \__ \ | (_| | | (_| | |  __/
//  |_|  |_|  \__,_| |_| |_|  \__,_| |_|  \___|   |_|  |_|  \___| |___/ |___/  \__,_|  \__, |  \___|
//                                                                                      __/ |
//                                                                                     |___/

fn handle_message(
    peer_map: PeerMap,
    user_list: UserList,
    session_list: SessionList,
    addr: SocketAddr,
    user_id: UserID,
    message_from_client: String,
) -> Result<(), String> {
    let result: SignalEnum = match serde_json::from_str(&message_from_client) {
        Ok(x) => x,
        Err(_) => {
            println!("Could not deserialize Message {} ", message_from_client);
            return Err("Could not deserialize Message".to_string());
        }
    };
    warn!("Handle {:?} from {:?} , {:?}", result, addr, user_id);

    // Result and who it needs to go to
    // 2 types of messages, either send to origin, or to other peer
    // match (message, destination) {
    let (message_to_client, destination) = match result {
        SignalEnum::VideoOffer(offer, session_id) => {
            let mut session_list_lock = session_list.lock().unwrap();
            let possible_session = session_list_lock.get_mut(&session_id);

            match possible_session {
                None => {
                    let e_msg = format!(
                        "VideoOffer Session {} Does NOT Exist, Groot kak",
                        session_id.inner()
                    );
                    error!("VideoOffer Session Doesn NOT Exist, Groot kak");
                    return Err(e_msg);
                }
                Some(session_members) => {
                    let doc_id = session_members.host.clone();
                    let sig_msg = SignalEnum::VideoOffer(offer, session_id.clone());
                    let message = match serde_json::to_string(&sig_msg) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let e_msg = format!(
                                "Could not Serialize {:?} as VideoOffer, {:?}",
                                session_id, e
                            );
                            return Err(e_msg);
                        }
                    };
                    (message, Destination::OtherPeer(doc_id))
                }
            }
        }
        SignalEnum::VideoAnswer(answer, session_id) => {
            let mut session_list_lock = session_list.lock().unwrap();
            let possible_session = session_list_lock.get_mut(&session_id);

            match possible_session {
                None => {
                    let e_msg = format!(
                        "VideoAnswer Session {} Doesn NOT Exist, Groot kak",
                        session_id.inner()
                    );
                    error!("VideoAnswer Session Doesn NOT Exist, Groot kak");
                    return Err(e_msg);
                }
                Some(session_members) => {
                    let opt_guest = session_members.guest.clone();
                    let guest = match opt_guest {
                        Some(guest) => guest,
                        None => {
                            let emsg= String::from("IceCandidate Error: No guest in Session, where are you sending Ice Candidates mate? ");
                            return Err(emsg);
                        }
                    };
                    let sig_msg = SignalEnum::VideoAnswer(answer, session_id.clone());
                    let message = match serde_json::to_string(&sig_msg) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let e_msg = format!(
                                "Could not Serialize {:?} as VideoAnswer, {:?}",
                                session_id, e
                            );
                            return Err(e_msg);
                        }
                    };
                    (message, Destination::OtherPeer(guest))
                }
            }
        }
        SignalEnum::IceCandidate(candidate, session_id) => {
            let mut session_list_lock = session_list.lock().unwrap();
            let possible_session = session_list_lock.get_mut(&session_id);

            match possible_session {
                None => {
                    let e_msg = format!(
                        "IceCandidate Session {} Doesn NOT Exist, Groot kak",
                        session_id.inner()
                    );
                    error!("IceCandidate Session Doesn NOT Exist, Groot kak");
                    return Err(e_msg);
                }
                Some(session_members) => {
                    let opt_guest = session_members.guest.clone();
                    let guest = match opt_guest {
                        Some(guest) => guest,
                        None => {
                            let emsg= String::from("IceCandidate Error: No guest in Session, where are you sending Ice Candidates mate? ");
                            return Err(emsg);
                        }
                    };

                    let host = session_members.host.clone();
                    let destination_peer;
                    if user_id == guest {
                        destination_peer = host;
                    } else if user_id == host {
                        destination_peer = guest;
                    } else {
                        let user_list_lock = user_list.lock().unwrap();
                        let socket_of_misalligned_user = user_list_lock.get(&user_id);
                        error!("UserID connection with {} attempted to send ICE peers to session {} when not assigned to the session", user_id.clone().inner(), session_id.clone().inner());
                        error!(
                            "Socket Address of Illegal user {:?}",
                            socket_of_misalligned_user
                        );
                        error!("Not Forwarding Ice candidate");
                        let e_msg = format!("User {:?}, attempted to send Ice Candidate on session {:?}, which User is not a part of", user_id.inner(), session_id.clone());
                        return Err(e_msg);
                    }

                    let sig_msg = SignalEnum::IceCandidate(candidate, session_id.clone());
                    let message = match serde_json::to_string(&sig_msg) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let e_msg = format!(
                                "Could not Serialize {:?} as VideoAnswer, {:?}",
                                session_id.clone(),
                                e
                            );
                            return Err(e_msg);
                        }
                    };
                    (message, Destination::OtherPeer(destination_peer))
                }
            }
        }
        SignalEnum::ICEError(_, _) => {
            unimplemented!("IceError Handling")
        }
        SignalEnum::SessionNew => {
            let session_id = SessionID::new(generate_id(5));
            let sig_msg = SignalEnum::SessionReady(session_id.clone());
            let message = match serde_json::to_string(&sig_msg) {
                Ok(msg) => msg,
                Err(e) => {
                    let e_msg = format!(
                        "Could not Serialize {:?} as SessionReady, {:?}",
                        session_id, e
                    );
                    return Err(e_msg);
                }
            };
            let session = SessionMembers {
                host: user_id,
                guest: None,
            };
            let insert_result = session_list
                .lock()
                .unwrap()
                .insert(session_id.clone(), session.clone());
            if insert_result.is_some() {
                warn!("Session_id {:?} Replaced \n    old Session value: {:?} \n    New Session value: {:?} \n ",session_id,insert_result, session);
            }
            (message, Destination::SourcePeer)
        }
        ///////////////////////////////////
        SignalEnum::SessionJoin(session_id) => {
            debug!("inside Session Join ");
            // Either Send back SessionJoinError Or SessionJoinSuccess
            let mut session_list_lock = session_list.lock().unwrap();
            let possible_session = session_list_lock.get_mut(&session_id);

            match possible_session {
                None => {
                    debug!("Session Doesn NOT Exist");
                    //  Session Does not Exists Send back error !
                    let sig_msg = SignalEnum::SessionJoinError("Session Does Not Exist".into());
                    let message = match serde_json::to_string(&sig_msg) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let e_msg = format!(
                                "Could not Serialize {:?} as SessionJoinError, {:?}",
                                session_id, e
                            );
                            return Err(e_msg);
                        }
                    };
                    (message, Destination::SourcePeer)
                }
                Some(session_members) => {
                    debug!("Session Exists ! Begin Signalling Flow ... ");

                    //  Session Exists Send back ready to start signalling !
                    session_members.guest = Some(user_id);

                    let sig_msg = SignalEnum::SessionJoinSuccess(session_id.clone());
                    let message = match serde_json::to_string(&sig_msg) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let e_msg = format!(
                                "Could not Serialize {:?} as SessionJoinSuccess, {:?}",
                                session_id, e
                            );
                            return Err(e_msg);
                        }
                    };
                    (message, Destination::SourcePeer)
                }
            }
        }
        SignalEnum::Debug => {
            debug!("=====================================");
            debug!("====== Signalling Server State ======");
            debug!("    User List {:?}", user_list);
            debug!("    Session List {:?}", session_list);
            debug!("====================================");
            return Ok(());
        }
        _ => {
            error!("Should not recieve state, {:?}", result);
            return Err(format!("Should not recieve state, {:?}", result));
        }
    };

    info!(
        "Message Handled, Replying to Client {:?} {:?}",
        message_to_client, destination
    );
    // Sending Message
    match destination {
        Destination::SourcePeer => {
            let peers = peer_map.lock().unwrap();
            let sender = match peers.get(&addr) {
                Some(x) => x,
                None => {
                    warn!("Peer was connection dropped from Hashmap, do nothing");
                    return Err("Peer was connection dropped from Hashmap, do nothing".into());
                }
            };

            debug!("Sending {} to {}", message_to_client, addr);
            let send_res = sender.unbounded_send(Message::Text(message_to_client));
            if send_res.is_err() {
                error!("{}", format!("Error Sending {:?}", send_res))
            }
        }
        Destination::OtherPeer(destination_peer) => {
            let user_list_lock = user_list.lock().unwrap();
            let opt_dest_socket = user_list_lock.get(&destination_peer);

            match opt_dest_socket {
                None => {
                    let e_msg = format!("Could not find socket with address {:?}", opt_dest_socket);
                    error!("{}", e_msg);
                    return Err(e_msg);
                }
                Some(socketaddr) => {
                    let peers = peer_map.lock().unwrap();
                    let sender = match peers.get(socketaddr) {
                        Some(x) => x,
                        None => {
                            warn!("Peer was connection dropped from Hashmap, do nothing");
                            return Err(
                                "Peer was connection dropped from Hashmap, do nothing".into()
                            );
                        }
                    };

                    debug!("Sending {} to {}", message_to_client, addr);
                    let send_res = sender.unbounded_send(Message::Text(message_to_client));
                    if send_res.is_err() {
                        error!("{}", format!("Error Sending {:?}", send_res))
                    }
                }
            }
        }
    }

    Ok(())
}

fn reply_with_id(tx: UnboundedSender<Message>, user_id: UserID) -> Result<(), String> {
    let sig_enum = SignalEnum::NewUser(user_id);

    let message = match serde_json::to_string(&sig_enum) {
        Ok(x) => x,
        Err(_) => {
            error!("Could not deserialize Message {:?} ", sig_enum);
            return Err("Could not deserialize Message".to_string());
        }
    };

    // Todo better error handling
    let res = tx.unbounded_send(Message::Text(message));
    if res.is_err() {
        error!("{:?}", res.unwrap_err());
    } else {
        info!("{:?}", res);
    }
    Ok(())
}

//   _    _                       _   _             _____                                         _     _
//  | |  | |                     | | | |           / ____|                                       | |   (_)
//  | |__| |   __ _   _ __     __| | | |   ___    | |        ___    _ __    _ __     ___    ___  | |_   _    ___    _ __
//  |  __  |  / _` | | '_ \   / _` | | |  / _ \   | |       / _ \  | '_ \  | '_ \   / _ \  / __| | __| | |  / _ \  | '_ \
//  | |  | | | (_| | | | | | | (_| | | | |  __/   | |____  | (_) | | | | | | | | | |  __/ | (__  | |_  | | | (_) | | | | |
//  |_|  |_|  \__,_| |_| |_|  \__,_| |_|  \___|    \_____|  \___/  |_| |_| |_| |_|  \___|  \___|  \__| |_|  \___/  |_| |_|

async fn handle_connection(
    peer_map: PeerMap,
    user_list: UserList,
    session_list: SessionList,
    raw_stream: TcpStream,
    addr: SocketAddr,
) {
    info!("Incoming TCP connection from: {}", addr);

    let ws_stream = async_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    info!("WebSocket connection established: {}", addr);

    // Insert the write part of this peer to the peer map.
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx.clone());

    // Insert the User_ID to the user_list
    let user_id = UserID::new(generate_id(10));

    {
        user_list.lock().unwrap().insert(user_id.clone(), addr);
    }

    // Here we reply with WS Connection ID
    // TODO better Error handling
    reply_with_id(tx, user_id.clone()).unwrap_or_else(|e| {
        error!("Failed to reply with id: {}", e);
    });

    // HERE THE FUN BEGINS
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

            let message = msg.to_text().unwrap().to_string();
            let result = handle_message(
                peer_map.clone(),
                user_list.clone(),
                session_list.clone(),
                addr,
                user_id.clone(),
                message,
            );
            //   handle_message(peer_map: PeerMap, user_list:UserList, addr: SocketAddr, message:String) -> Result<(), String>{
            if result.is_err() {
                error!("Handle Message Error {:?}", result);
            } else {
                info!("Handle Message Ok : result {:?}", result);
            }

            future::ok(())
        });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    info!("{} disconnected", &addr);
    // Remove from peer map
    peer_map.lock().unwrap().remove(&addr);

    // Remove from User_List
    user_list.lock().unwrap().remove(&user_id);

    // TODO: Close any sessions assoicated with the address IF user hosted the session.
    let sess_list: Vec<SessionID> = {
        session_list
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(sid, members)| {
                if members.host == user_id {
                    Some(sid.clone())
                } else {
                    None
                }
            })
            .collect()
    };

    for s in sess_list {
        session_list.lock().unwrap().remove(&s);
    }
}

async fn run() -> Result<(), IoError> {
    let mut addr = get_local_ip().expect("Couldn't get IP");
    addr.push_str(":2794");

    let user_list = UserList::new(Mutex::new(HashMap::new()));
    let session_list = SessionList::new(Mutex::new(HashMap::new()));
    let peer_map = PeerMap::new(Mutex::new(HashMap::new()));

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;

    let listener = try_socket.expect("Failed to bind");

    info!("Listening on: {}", addr);

    // Let's spawn the handling of each connection in a separate Async task.
    while let Ok((stream, addr)) = listener.accept().await {
        task::spawn(handle_connection(
            peer_map.clone(),
            user_list.clone(),
            session_list.clone(),
            stream,
            addr,
        ));
    }
    Ok(())
}

fn main() -> Result<(), IoError> {
    match setup_logging() {
        Ok(_) => (),
        Err(e) => {
            println!("Could not start logger,{}\n...exiting", e);
            std::process::exit(1);
        }
    }

    task::block_on(run())
}
