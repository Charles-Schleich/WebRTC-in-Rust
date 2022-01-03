#![allow(non_snake_case)]

use js_sys::JSON;

use log::{error, info, warn};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use web_sys::{
    RtcIceCandidate, RtcIceCandidateInit, RtcPeerConnection, RtcPeerConnectionIceEvent, WebSocket,
};

use serde::{Deserialize, Serialize};

use std::cell::RefCell;
use std::rc::Rc;

use shared_protocol::*;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct IceCandidateSend {
    pub candidate: String,
    pub sdpMid: String,
    pub sdpMLineIndex: u16,
    // pub usernameFragment:String // This seems to be specific to FireFox
}

//  _____    _____   ______     _   _                          _     _           _     _
// |_   _|  / ____| |  ____|   | \ | |                        | |   (_)         | |   (_)
//   | |   | |      | |__      |  \| |   ___    __ _    ___   | |_   _    __ _  | |_   _    ___    _ __
//   | |   | |      |  __|     | . ` |  / _ \  / _` |  / _ \  | __| | |  / _` | | __| | |  / _ \  | '_ \
//  _| |_  | |____  | |____    | |\  | |  __/ | (_| | | (_) | | |_  | | | (_| | | |_  | | | (_) | | | | |
// |_____|  \_____| |______|   |_| \_|  \___|  \__, |  \___/   \__| |_|  \__,_|  \__| |_|  \___/  |_| |_|
//                                              __/ |
//                                             |___/

// As soon as This peer has an ICE Candidate then send it over the websocket connection
#[allow(non_snake_case)]
pub async fn setup_RTCPeerConnection_ICECallbacks(
    rtc_conn: RtcPeerConnection,
    ws: WebSocket,
    rc_state: Rc<RefCell<AppState>>,
) -> Result<RtcPeerConnection, JsValue> {
    let onicecandidate_callback1 =
        Closure::wrap(
            Box::new(move |ev: RtcPeerConnectionIceEvent| match ev.candidate() {
                Some(candidate) => {
                    let json_obj_candidate = candidate.to_json();
                    let res = JSON::stringify(&json_obj_candidate).unwrap_throw();

                    let js_ob = String::from(res.clone());

                    let mut state = rc_state.borrow_mut();
                    let session_id = match state.get_session_id() {
                        Some(sid) => sid,
                        None => {
                            error!("No Session ID has been set yet");
                            return ;
                        }
                    };

                    // state.set_session_id(session_id.clone());
                    // let session_id= String::from("12345");

                    let signal = SignalEnum::IceCandidate(js_ob, session_id);
                    let ice_candidate: String = serde_json_wasm::to_string(&signal).unwrap();

                    info!("Sending IceCandidate to Other peer {:?}", res);
                    match ws.send_with_str(&ice_candidate) {
                        Ok(_) => info!("IceCandidate sent {}", ice_candidate),
                        Err(err) => error!("error sending IceCandidate SignalEnum: {:?}", err),
                    }
                }
                None => {
                    info!("No ICE candidate found");
                }
            }) as Box<dyn FnMut(RtcPeerConnectionIceEvent)>,
        );
    rtc_conn.set_onicecandidate(Some(onicecandidate_callback1.as_ref().unchecked_ref()));
    onicecandidate_callback1.forget();
    Ok(rtc_conn)
}

pub async fn recieved_new_ice_candidate(
    candidate: String,
    rtc_conn: RtcPeerConnection,
) -> Result<(), JsValue> {
    warn!("ICECandidate Recieved! {}", candidate);

    if candidate.eq("") {
        info!("ICECandidate! is empty doing nothing");
    } else {
        let icecandidate: IceCandidateSend = match serde_json_wasm::from_str(&candidate) {
            Ok(x) => x,
            Err(_e) => {
                let message = format!("Could not deserialize Ice Candidate {} ", candidate);
                return Err(JsValue::from_str(&message));
            }
        };

        let mut rtc_ice_init = RtcIceCandidateInit::new("");
        rtc_ice_init.candidate(&icecandidate.candidate);
        rtc_ice_init.sdp_m_line_index(Some(icecandidate.sdpMLineIndex));
        rtc_ice_init.sdp_mid(Some(&icecandidate.sdpMid));

        match RtcIceCandidate::new(&rtc_ice_init) {
            Ok(x) => {
                let promise = rtc_conn
                    .clone()
                    .add_ice_candidate_with_opt_rtc_ice_candidate(Some(&x));
                let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
                info!("Added other peer's Ice Candidate ! {:?}", result);
            }
            Err(e) => {
                info!("Ice Candidate Addition error, {} | {:?}", candidate, e);
                return Err(e);
            }
        };
    }
    Ok(())
}
