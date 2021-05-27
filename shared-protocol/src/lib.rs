
use serde::{Deserialize,Serialize};

pub const SERVER_PORT : &str = "9000";

// pub type SessionID = String;
#[derive(Debug,Serialize, Deserialize,Clone,Eq,PartialEq,Hash)]
pub struct SessionID(String);
impl SessionID {
    pub fn new(inner: String) -> Self {
        SessionID(inner)
    }
    pub fn inner(self) -> String {
        self.0
    }
}

impl Into<SessionID> for &str{
    fn into(self) -> SessionID {
        SessionID::new(self.into())
    }
} 

#[derive(Debug,Serialize, Deserialize,Clone,Eq,PartialEq,Hash)]
pub struct UserID(String);
impl UserID {
    pub fn new(inner: String) -> Self {
        UserID(inner)
    }
    pub fn inner(self) -> String {
        self.0
    }
}

#[derive(Debug,Serialize, Deserialize)]
pub enum SignalEnum {
    // Return called by the server as soon as the user connects
    NewUser(UserID),

    // To manage a live session between two users
    SessionNew,
    SessionReady(SessionID),
    SessionJoin(SessionID),
    SessionJoinSuccess(SessionID),
    SessionJoinError(SessionID),

    // When Connecting to a Session
    VideoOffer(String, SessionID),
    VideoAnswer(String, SessionID),
    IceCandidate(String, SessionID),
    ICEError(String, SessionID),
    
    // 
    Debug,
}

// debug_signal_server_state