
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

// From Workspace
use shared_protocol::*;

// __          __         _          _____                  _             _   
// \ \        / /        | |        / ____|                | |           | |  
//  \ \  /\  / /    ___  | |__     | (___     ___     ___  | | __   ___  | |_ 
//   \ \/  \/ /    / _ \ | '_ \     \___ \   / _ \   / __| | |/ /  / _ \ | __|
//    \  /\  /    |  __/ | |_) |    ____) | | (_) | | (__  |   <  |  __/ | |_ 
//     \/  \/      \___| |_.__/    |_____/   \___/   \___| |_|\_\  \___|  \__|

const WS_IP_PORT : &str = "ws://192.168.178.28:2794";

// #[wasm_bindgen]
pub async fn open_web_socket(rtc_conn:RtcPeerConnection, rc_state: Rc<RefCell<AppState>>) -> Result<WebSocket,JsValue> {
    info!("Openning WS Connection");
   
    let ws = WebSocket::new(WS_IP_PORT)?;
   
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
    let cloned_ws_ext = ws.clone();
    let cloned_state_ext = rc_state.clone();
    //  ON MESSAGE CALLBACK
    let onmessage_callback = Closure::wrap(Box::new( move |ev :MessageEvent| {
        if let Ok(abuf) = ev.data().dyn_into::<js_sys::ArrayBuffer>(){
            info!("WS: message event, recieved arraybuffer: {:?}", abuf);
        } else if let Ok(blob) = ev.data().dyn_into::<web_sys::Blob>() {
            info!("WS: message event, recieved blob: {:?}", blob);
        } else if let Ok(txt) = ev.data().dyn_into::<js_sys::JsString>() {
            info!("WS: message event, recieved string: {:?}", txt);
            let rust_string = String::from(txt);
            // put the below line in an asycn
            let rtc_conn_clone= rtc_conn.clone();
            let cloned_ws= cloned_ws_ext.clone();
            let cloned_state = cloned_state_ext.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let result= handle_message_reply(rust_string, rtc_conn_clone.clone(), cloned_ws.clone(),cloned_state ).await;
                match result {
                    Err(x) => error!("{:?}",x),
                    _ => {debug!("Handle Signalling message done")}
                }
            });
        } else {
            info!("message event, received Unknown: {:?}", ev.data());
        }

    }) as Box<dyn FnMut(MessageEvent)>);
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document:Document = window.document().expect("Couldnt Get Document");
    let ws_conn_lbl = "ws_conn_lbl";
    let ws_conn_lbl_err = "ws_conn_lbl_err";
        
    //  ON ERROR
    let document_clone:Document = document.clone();
    let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
        error!("WS: onerror_callback error event: {:?}", e);

        document_clone
            .get_element_by_id(ws_conn_lbl_err)
            .expect(&format!("Should have {} on the page",ws_conn_lbl_err))
            .dyn_ref::<HtmlLabelElement>()
            .expect("#Button should be a be an `HtmlLabelElement`")
            .set_text_content(Some(&format!("{}","Could not make Websocket Connection, Is the Signalling Server running ? ")));
            
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();

    let ws_clone_ext = ws.clone();
    //  ON OPEN
    let document_clone:Document = document.clone();
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        // info!("WS: opened");
        let ws_clone = ws_clone_ext.clone();
        
        document_clone
            .get_element_by_id(ws_conn_lbl)
            .expect(&format!("Should have {} on the page",ws_conn_lbl))
            .dyn_ref::<HtmlLabelElement>()
            .expect("#Button should be a be an `HtmlLabelElement`")
            .set_text_content(Some(&format!("{}","Websocket Connected !")));
        
        document_clone
            .get_element_by_id(ws_conn_lbl_err)
            .expect(&format!("Should have {} on the page",ws_conn_lbl_err))
            .dyn_ref::<HtmlLabelElement>()
            .expect("#Button should be a be an `HtmlLabelElement`")
            .set_text_content(Some(&format!("{}","")));
        // Start SDP connection here
        // info!("WS: opened end");
        request_session(ws_clone);

    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    // input
    Ok(ws.clone())
}




fn request_session(ws:WebSocket){
    info!("Sending SessionNew");

    let msg =  SignalEnum::SessionNew;
    let ser_msg : String  = match serde_json_wasm::to_string(&msg){
        Ok(x) => x,
        Err(e) => {
            error!("Could not Seralize SessionNew {}",e);
            return ;
        } 
    };

    match ws.clone().send_with_str(&ser_msg){
        Ok(_) =>{}
        Err(e) =>{
            error!("Error Sending SessionNew {:?}",e);
        }
    }
}