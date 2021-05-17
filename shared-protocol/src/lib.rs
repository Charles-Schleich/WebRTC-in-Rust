
use serde::{Deserialize,Serialize};

pub type SessionID = String;

pub const SERVER_PORT : &str = "9000";

#[derive(Debug,Serialize, Deserialize)]
pub enum SignalEnum {
    // Return called by the server as soon as the user connects
    NewUser(String),

    // To manage a live session between two users
    SessionNew,
    SessionReady(String),
    SessionJoin(String),
    SessionJoinSuccess(String),
    SessionJoinError(String),

    // When Connecting to a Session
    VideoOffer(String, SessionID),
    VideoAnswer(String, SessionID),
    IceCandidate(String, SessionID),
    ICEError(String, SessionID),
    
    // 
    Debug,
}

// debug_signal_server_state