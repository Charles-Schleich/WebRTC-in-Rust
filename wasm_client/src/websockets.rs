
// Imports 
use wasm_bindgen::prelude::*;

use web_sys::{
    MessageEvent, 
    RtcPeerConnection,
    ErrorEvent, WebSocket,
};
use log::{info,warn,error,debug};
 
use wasm_bindgen::JsCast;
use serde::{Serialize, Deserialize};

// local
use super::*;

// __          __         _          _____                  _             _   
// \ \        / /        | |        / ____|                | |           | |  
//  \ \  /\  / /    ___  | |__     | (___     ___     ___  | | __   ___  | |_ 
//   \ \/  \/ /    / _ \ | '_ \     \___ \   / _ \   / __| | |/ /  / _ \ | __|
//    \  /\  /    |  __/ | |_) |    ____) | | (_) | | (__  |   <  |  __/ | |_ 
//     \/  \/      \___| |_.__/    |_____/   \___/   \___| |_|\_\  \___|  \__|

const WS_IP_PORT : &str = "ws://192.168.178.28:2794";

#[derive(Serialize, Deserialize)]
pub enum SignallingMessage {
    VideoOffer(String),
    VideoAnswer(String),
    IceCandidate(String),
    ICEError(String),
}

#[derive(Debug,Serialize, Deserialize)]
pub struct IceCandidateSend {
    pub candidate: String,
    pub sdpMid:String,
    pub sdpMLineIndex:u16,
    // pub usernameFragment:String // This seems to be specific to FireFox
}

#[wasm_bindgen]
pub async fn open_web_socket(rtc_conn:RtcPeerConnection) -> Result<WebSocket,JsValue> {
    info!("Openning WS Connection");
   
    let ws = WebSocket::new(WS_IP_PORT)?;
   
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
    let cloned_ws = ws.clone();

    //  ON MESSAGE CALLBACK
    let onmessage_callback = Closure::wrap(Box::new( move |ev :MessageEvent| {

        if let Ok(abuf) = ev.data().dyn_into::<js_sys::ArrayBuffer>(){
            info!("WS: message event, recieved arraybuffer: {:?}", abuf);

        } else if let Ok(blob) = ev.data().dyn_into::<web_sys::Blob>() {
            info!("WS: message event, recieved blob: {:?}", blob);

        } else if let Ok(txt) = ev.data().dyn_into::<js_sys::JsString>() {
            // TEXT
            // TEXT
            // TEXT
            let rust_string = String::from(txt);
            // put the below line in an asycn
            let rtc_conn_clone= rtc_conn.clone();
            let cloned_ws_clone= cloned_ws.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let result= handle_message_reply(rust_string, rtc_conn_clone.clone(), cloned_ws_clone.clone()).await;
                match result {
                    Err(x) => error!("{:?}",x),
                    _ => info!("Handle Signalling message done")
                }
            });

            
        } else {
            info!("message event, received Unknown: {:?}", ev.data());
        }

    }) as Box<dyn FnMut(MessageEvent)>);
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    //  ON ERROR
    let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
        error!("WS: onerror_callback error event: {:?}", e);
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();


    //  ON OPEN
    let cloned_ws = ws.clone();
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        info!("WS: opened");
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    let ws_cloned2 = ws.clone();

    // input
    Ok(ws.clone())
}
