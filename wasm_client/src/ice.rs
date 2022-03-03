use std::cell::RefCell;
use std::rc::Rc;

use js_sys::JSON;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcIceCandidate, RtcIceCandidateInit, RtcPeerConnection, RtcPeerConnectionIceEvent, WebSocket,
};

use shared_protocol::SignalEnum;

use crate::common::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdpMid: String, // must be non-snake case as this is the key in the parsed JSON
    pub sdpMLineIndex: u16, // must be non-snake case as this is the key in the parsed JSON
}

pub async fn setup_rtc_peer_connection_ice_callbacks(
    rtc_conn: RtcPeerConnection,
    ws: WebSocket,
    rc_state: Rc<RefCell<AppState>>,
) -> Result<RtcPeerConnection, JsValue> {
    let onicecandidate_callback =
        Closure::wrap(
            Box::new(move |ev: RtcPeerConnectionIceEvent| 
                {
                    let ws = ws.clone();
                    let rc_state = rc_state.clone();
                    send_ice_candidate(ws,rc_state,ev);
                }
            ) as Box<dyn FnMut(RtcPeerConnectionIceEvent)>,
        );
    rtc_conn.set_onicecandidate(Some(onicecandidate_callback.as_ref().unchecked_ref()));
    onicecandidate_callback.forget();
    Ok(rtc_conn)
}


pub fn sleep(ms: i32) -> js_sys::Promise {
    js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
            .unwrap();
    })
}

pub fn send_ice_candidate(
    ws: WebSocket,
    rc_state: Rc<RefCell<AppState>>,
    ev: RtcPeerConnectionIceEvent
) {
    match ev.candidate() {
        Some(candidate) => {
            let json_obj_candidate = candidate.to_json();
            let res = JSON::stringify(&json_obj_candidate).unwrap_throw();

            let js_ob = String::from(res.clone());
            
            let ws= ws.clone();
            let rc_state = rc_state.clone();

            let state = rc_state.borrow();
            let opt_session_id = state.get_session_id_ref().clone();
            drop(state);
            let session_id = match opt_session_id {
                Some(sid) => sid,
                None => {
                    error!("No Session ID has been set yet");
                    let sleep_promise= sleep(3000);
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ = wasm_bindgen_futures::JsFuture::from(sleep_promise).await;
                        send_ice_candidate(ws,rc_state,ev);
                        error!("Session ID set now ???? ");
                    });
                    return;
                }
            };
            let signal = SignalEnum::IceCandidate(js_ob, session_id);
            let ice_candidate: String = serde_json_wasm::to_string(&signal).unwrap();
            info!("Sending IceCandidate to Other peer {:?}", res);
            match ws.send_with_str(&ice_candidate) {
                Ok(_) => info!("IceCandidate sent {}", ice_candidate),
                Err(err) => error!("error sending IceCandidate SignalEnum: {:?}", err),
            }
        }
        None => {
            info!("No ICE candidate in RtcPeerConnectionIceEvent");
        }
    }
}


pub async fn received_new_ice_candidate(
    candidate: String,
    rtc_conn: RtcPeerConnection,
) -> Result<(), JsValue> {
    warn!("ICECandidate Received! {}", candidate);

    let icecandidate = serde_json_wasm::from_str::<IceCandidate>(&candidate).map_err(|_| {
        let message = format!("Could not deserialize Ice Candidate {} ", candidate);
        JsValue::from_str(&message)
    })?;

    let mut rtc_ice_init = RtcIceCandidateInit::new("");
    rtc_ice_init.candidate(&icecandidate.candidate);
    rtc_ice_init.sdp_m_line_index(Some(icecandidate.sdpMLineIndex));
    rtc_ice_init.sdp_mid(Some(&icecandidate.sdpMid));

    match RtcIceCandidate::new(&rtc_ice_init) {
        Ok(x) => {
            let result =
                JsFuture::from(rtc_conn.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&x)))
                    .await?;
            info!("Added other peer's Ice Candidate ! {:?}", result);
        }
        Err(e) => {
            info!("Ice Candidate Addition error, {} | {:?}", candidate, e);
            return Err(e);
        }
    };
    Ok(())
}
