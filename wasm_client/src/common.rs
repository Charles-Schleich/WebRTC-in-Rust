use std::cell::RefCell;
use std::convert::TryInto;
use std::rc::Rc;


use js_sys::{Array, Object, Promise, Reflect};
use log::{debug, error, info, warn};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::{
    Document, Element, HtmlButtonElement, HtmlInputElement, HtmlLabelElement, HtmlVideoElement,
    MediaStream, MediaStreamConstraints, MessageEvent, RtcConfiguration, RtcDataChannel,
    RtcDataChannelEvent, RtcIceConnectionState, RtcPeerConnection, WebSocket,
    RtcIceCredentialType, RtcIceServer, RtcIceTransportPolicy,
};

use shared_protocol::{SessionID, SignalEnum, UserID};

use crate::{
    create_sdp_offer, receive_sdp_answer, receive_sdp_offer_send_answer,
    received_new_ice_candidate, setup_rtc_peer_connection_ice_callbacks, wasm_bindgen,
};

const STUN_SERVER: &str = "stun:stun.l.google.com:19302";
const TURN: &str = "turn:192.168.178.60:3478";

#[derive(Debug)]
pub struct AppState {
    session_id: Option<SessionID>,
    user_id: Option<UserID>,
}

impl AppState {
    pub(crate) fn new() -> Self {
        AppState {
            session_id: None,
            user_id: None,
        }
    }

    pub(crate) fn set_session_id(&mut self, s_id: SessionID) {
        self.session_id = Some(s_id)
    }

    pub(crate) fn get_session_id(&mut self) -> Option<SessionID> {
        self.session_id.clone()
    }

    pub(crate) fn get_session_id_ref(&self) -> Option<SessionID> {
        self.session_id.clone()
    }

    pub(crate) fn set_user_id(&mut self, user_id: UserID) {
        self.user_id = Some(user_id)
    }

    pub(crate) fn get_user_id(&mut self) -> Option<UserID> {
        self.user_id.clone()
    }
}

pub fn create_plain_peer_connection() -> Result<RtcPeerConnection, JsValue> {
    RtcPeerConnection::new()
}

pub fn create_turn_peer_connection() -> Result<RtcPeerConnection, JsValue> {
    
    // STUN HERE 
    let mut stun_server = RtcIceServer::new();
    stun_server.url(&STUN_SERVER);

    // TURN SERVER
    let turn_url = format!("{}",TURN);
    warn!("Turn URL: {}", TURN);
    let mut turn_server = RtcIceServer::new();
    turn_server.url(&turn_url);
    let r_num= f64::ceil(js_sys::Math::random()*10.0) ;
    let r_num2 = r_num as u8;

    // Both users can have the same username + password,
    // The turn server doesnt really care
    let user = format!("user{}",r_num2);
    let pass = format!("pass{}",r_num2);

    info!("{}",format!("Creds: user:{} pass:{}",user, pass));
    turn_server.username(&user);
    turn_server.credential(&pass);

    // turn_server.credential_type( RtcIceCredentialType::Token);
    turn_server.credential_type(RtcIceCredentialType::Password);
    let turn_server_ref: &JsValue = turn_server.as_ref();
    let mut rtc_config = RtcConfiguration::new();
    // let arr_ice_svr = Array::of2(turn_server_ref,stun_server_ref);
    let arr_ice_svr = Array::of1(turn_server_ref);
    warn!("ICE server Length {}",arr_ice_svr.length());
    let arr_ice_svr_ref: &JsValue = arr_ice_svr.as_ref();
    rtc_config.ice_servers(arr_ice_svr_ref);

    // rtc_config.ice_transport_policy(RtcIceTransportPolicy::All);
    // warn!("All transport");
    // let transport_policy = RtcIceTransportPolicy::All;
    let transport_policy = RtcIceTransportPolicy::Relay;
    warn!("ICE transport {:?}",transport_policy);
    rtc_config.ice_transport_policy(transport_policy); // This is to force use of a TURN Server

    RtcPeerConnection::new_with_configuration(&rtc_config)

}

pub fn create_stun_peer_connection() -> Result<RtcPeerConnection, JsValue> {
    let ice_servers = Array::new();
    {
        let server_entry = Object::new();

        Reflect::set(&server_entry, &"urls".into(), &STUN_SERVER.into())?;

        ice_servers.push(&*server_entry);
    }

    let mut rtc_configuration = RtcConfiguration::new();
    rtc_configuration.ice_servers(&ice_servers);

    RtcPeerConnection::new_with_configuration(&rtc_configuration)
}

pub async fn handle_message_reply(
    message: String,
    peer_connection: RtcPeerConnection,
    websocket: WebSocket,
    app_state: Rc<RefCell<AppState>>,
) -> Result<(), JsValue> {
    let result = match serde_json_wasm::from_str(&message) {
        Ok(x) => x,
        Err(_) => {
            error!("Could not deserialize Message {} ", message);
            return Ok(());
        }
    };

    match result {
        SignalEnum::VideoOffer(offer, session_id) => {
            warn!("VideoOffer Received ");
            let sdp_answer = receive_sdp_offer_send_answer(peer_connection.clone(), offer).await?;
            let signal = SignalEnum::VideoAnswer(sdp_answer, session_id);
            let response: String = match serde_json_wasm::to_string(&signal) {
                Ok(x) => x,
                Err(e) => {
                    error!("Could not Serialize Video Offer {}", e);
                    return Err(JsValue::from_str("Could not Serialize Video Offer"));
                }
            };

            match websocket.send_with_str(&response) {
                Ok(_) => info!("Video Offer SignalEnum sent"),
                Err(err) => error!("Error sending Video Offer SignalEnum: {:?}", err),
            }
        }
        SignalEnum::VideoAnswer(answer, _) => {
            info!("Video Answer Received! {}", answer);
            receive_sdp_answer(peer_connection.clone(), answer).await?;
        }
        SignalEnum::IceCandidate(candidate, _) => {
            received_new_ice_candidate(candidate, peer_connection.clone()).await?;
        }
        SignalEnum::SessionReady(session_id) => {
            info!("SessionReady Received ! {:?}", session_id);
            let mut state = app_state.borrow_mut();
            state.set_session_id(session_id.clone());
            set_html_label("sessionid_lbl", session_id.inner());
        }
        SignalEnum::SessionJoinSuccess(session_id) => {
            info!("SessionJoinSuccess {}", session_id.clone().inner());
            set_session_connection_status_error("".into());
            let mut state = app_state.borrow_mut();
            state.set_session_id(session_id.clone());
            drop(state);
            // Initiate the video call
            send_video_offer(
                peer_connection.clone(),
                websocket.clone(),
                session_id.clone(),
            )
            .await;
            let full_string = format!("Connecting to Session: {}", session_id.inner());
            set_html_label("session_connection_status", full_string);
            set_html_label("sessionid_heading", "".into());
        }
        SignalEnum::SessionJoinError(session_id) => {
            error!("SessionJoinError! {}", session_id.clone().inner());
            set_session_connection_status_error(session_id.inner());
        }
        SignalEnum::SessionJoin(session_id) => {
            info!("{}", session_id.inner())
        }
        SignalEnum::NewUser(user_id) => {
            info!("New User Received ! {}", user_id.clone().inner());
            let mut state = app_state.borrow_mut();
            state.set_user_id(user_id);
        }
        SignalEnum::ICEError(err, session_id) => {
            error!("ICEError! {}, {} ", err, session_id.inner());
        }
        remaining => {
            error!("Frontend should not receive {:?}", remaining);
        }
    };
    Ok(())
}

#[wasm_bindgen]
pub async fn get_video(video_id: String) -> Result<MediaStream, JsValue> {
    info!("Starting Video Device Capture!");
    let window = web_sys::window().expect("No window Found");
    let navigator = window.navigator();
    let media_devices = match navigator.media_devices() {
        Ok(md) => md,
        Err(e) => return Err(e),
    };

    let mut constraints = MediaStreamConstraints::new();
    constraints.audio(&JsValue::FALSE); // Change this if you want Audio as well !
    constraints.video(&JsValue::TRUE);

    let stream_promise: Promise = match media_devices.get_user_media_with_constraints(&constraints)
    {
        Ok(s) => s,
        Err(e) => return Err(e),
    };

    let document: Document = window.document().expect("Couldn't Get Document");

    let video_element: Element = match document.get_element_by_id(&video_id) {
        Some(ms) => ms,
        None => return Err(JsValue::from_str("No Element video found")),
    };

    // debug!("video_element {:?}", video_element);

    let media_stream: MediaStream = match wasm_bindgen_futures::JsFuture::from(stream_promise).await
    {
        Ok(ms) => MediaStream::from(ms),
        Err(e) => {
            error!("{:?}", e);
            error!("{:?}","Its possible that the There is already a tab open with a handle to the Media Stream");
            error!(
                "{:?}",
                "Check if Other tab is open with Video/Audio Stream open"
            );
            return Err(JsValue::from_str("User Did not allow access to the Camera"));
        }
    };

    let vid_elem: HtmlVideoElement = match video_element.dyn_into::<HtmlVideoElement>() {
        Ok(x) => x,
        Err(e) => {
            error!("{:?}", e);
            return Err(JsValue::from_str("User Did not allow access to the Camera"));
        }
    };

    vid_elem.set_src_object(Some(&media_stream));
    Ok(media_stream)
}

pub fn setup_show_state(rtc_conn: RtcPeerConnection, state: Rc<RefCell<AppState>>) {
    let window = web_sys::window().expect("No window Found");
    let document: Document = window.document().expect("Couldn't Get Document");

    // DEBUG BUTTONS
    let rtc_clone_external = rtc_conn;
    let btn_cb = Closure::wrap(Box::new(move || {
        let rtc_clone = rtc_clone_external.clone();
        show_rtc_state(rtc_clone, state.clone());
    }) as Box<dyn FnMut()>);

    document
        .get_element_by_id("debug_client_state")
        .expect("should have debug_client_state on the page")
        .dyn_ref::<HtmlButtonElement>()
        .expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
}

fn show_rtc_state(rtc_conn: RtcPeerConnection, state: Rc<RefCell<AppState>>) {
    debug!("===========================");
    debug!("Signalling State : {:?}", rtc_conn.signaling_state());
    debug!("Ice Conn State : {:?}", rtc_conn.ice_connection_state());
    debug!("ice gathering_state : {:?}", rtc_conn.ice_gathering_state());
    debug!("local_description : {:?}", rtc_conn.local_description());
    debug!("remote_description : {:?}", rtc_conn.remote_description());
    debug!("get_senders : {:?}", rtc_conn.get_senders());
    debug!("get_receivers : {:?}", rtc_conn.get_receivers());
    debug!("===========================");

    let mut state = state.borrow_mut();

    debug!("===========================");
    debug!(" User ID : {:?}", state.get_user_id());
    debug!(" Session ID : {:?}", state.get_session_id());
}

pub fn setup_show_signalling_server_state(ws: WebSocket) {
    let window = web_sys::window().expect("No window Found");
    let document: Document = window.document().expect("Couldn't Get Document");

    // DEBUG BUTTONS
    let btn_cb = Closure::wrap(Box::new(move || {
        let msg = SignalEnum::Debug;
        let ser_msg: String =
            serde_json_wasm::to_string(&msg).expect("Couldn't Serialize SignalEnum::Debug Message");

        match ws.clone().send_with_str(&ser_msg) {
            Ok(_) => {}
            Err(e) => {
                error!("Error Sending SessionNew {:?}", e);
            }
        }
    }) as Box<dyn FnMut()>);

    document
        .get_element_by_id("debug_signal_server_state")
        .expect("should have debug_signal_server_state on the page")
        .dyn_ref::<HtmlButtonElement>()
        .expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
}

/// RTC Listener
pub async fn setup_listener(
    peer_b: RtcPeerConnection,
    websocket: WebSocket,
    rc_state: Rc<RefCell<AppState>>,
) -> Result<(), JsValue> {
    let window = web_sys::window().expect("No window Found");
    let document: Document = window.document().expect("Couldn't Get Document");

    let ws_clone_external = websocket;
    let peer_b_clone_external = peer_b;
    let document_clone_external = document.clone();
    let rc_state_clone_external = rc_state;

    let btn_cb = Closure::wrap(Box::new(move || {
        let ws_clone = ws_clone_external.clone();
        let peer_b_clone = peer_b_clone_external.clone();
        let document_clone = document_clone_external.clone();
        let rc_state_clone_internal = rc_state_clone_external.clone();

        // Start Remote Video Callback
        let video_elem = "peer_a_video".into();

        let ice_state_change =
            rtc_ice_state_change(peer_b_clone.clone(), document_clone, video_elem);
        peer_b_clone
            .set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
        ice_state_change.forget();

        // Start Local Video Callback
        let peer_b_clone_media = peer_b_clone_external.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let media_stream = get_video(String::from("peer_b_video"))
                .await
                .expect_throw("Couldn't Get Media Stream");
            peer_b_clone_media.add_stream(&media_stream);
        });

        // Need to setup Media Stream BEFORE sending SDP offer!!!
        // SDP offer Contains information about the Video Streaming technologies available to this and the other browser
        // If negotiation has completed, this closure will be called
        let ondatachannel_callback = Closure::wrap(Box::new(move |ev: RtcDataChannelEvent| {
            let dc2 = ev.channel();
            info!("peer_b.ondatachannel! : {}", dc2.label());
            let onmessage_callback = Closure::wrap(Box::new(move |ev: MessageEvent| {
                if let Some(message) = ev.data().as_string() {
                    warn!("{:?}", message)
                }
            }) as Box<dyn FnMut(MessageEvent)>);
            dc2.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
            onmessage_callback.forget();
            dc2.send_with_str("Ping from peer_b.dc!").unwrap();
        })
            as Box<dyn FnMut(RtcDataChannelEvent)>);

        peer_b_clone.set_ondatachannel(Some(ondatachannel_callback.as_ref().unchecked_ref()));
        ondatachannel_callback.forget();

        let peer_b_clone = peer_b_clone_external.clone();
        let ws_clone1 = ws_clone.clone();
        let rc_state_clone = rc_state_clone_internal;

        // Setup ICE callbacks
        // let res = setup_rtc_peer_connection_ice_callbacks(peer_b_clone, ws_clone1, rc_state_clone).await;
        // if res.is_err() {
        //     log::error!("Error Setting up ice callbacks {:?}", res.unwrap_err())
        // }

        wasm_bindgen_futures::spawn_local(async move {
            let res = setup_rtc_peer_connection_ice_callbacks(peer_b_clone, ws_clone1, rc_state_clone).await;
                if res.is_err() {
                    log::error!("Error Setting up ice callbacks {:?}", res.unwrap_err())
                }
        });


        host_session(ws_clone);
    }) as Box<dyn FnMut()>);

    document
        .get_element_by_id("start_session")
        .expect("should have start_session on the page")
        .dyn_ref::<HtmlButtonElement>()
        .expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();

    Ok(())
}

fn host_session(ws: WebSocket) {
    info!("Sending SessionNew");
    let msg = SignalEnum::SessionNew;
    let ser_msg: String = match serde_json_wasm::to_string(&msg) {
        Ok(x) => x,
        Err(e) => {
            error!("Could not serialize SessionNew {}", e);
            return;
        }
    };

    match ws.send_with_str(&ser_msg) {
        Ok(_) => {}
        Err(e) => {
            error!("Error Sending SessionNew {:?}", e);
        }
    }
}

fn peer_a_dc_on_message(dc: RtcDataChannel) -> Closure<dyn FnMut(MessageEvent)> {
    Closure::wrap(Box::new(move |ev: MessageEvent| {
        if let Some(message) = ev.data().as_string() {
            warn!("{:?}", message);
            dc.send_with_str("Pong from peer_a data channel!").unwrap();
        }
    }) as Box<dyn FnMut(MessageEvent)>)
}

pub async fn setup_initiator(
    peer_a: RtcPeerConnection,
    websocket: WebSocket,
    rc_state: Rc<RefCell<AppState>>,
) -> Result<(), JsValue> {
    let window = web_sys::window().expect("No window Found");
    let document: Document = window.document().expect("Couldn't Get Document");

    let ws_clone_external = websocket;
    let peer_a_clone_external = peer_a.clone();
    let rc_state_clone_ext = rc_state;

    /*
     * Create DataChannel on peer_a to negotiate
     * Message will be shown here after connection established
     */

    info!("peer_a State 1: {:?}", peer_a.signaling_state());
    let dc1 = peer_a.create_data_channel("my-data-channel");
    info!("dc1 created: label {:?}", dc1.label());

    let dc1_clone = dc1.clone();
    let onmessage_callback = peer_a_dc_on_message(dc1_clone);
    dc1.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let btn_cb = Closure::wrap(Box::new(move || {
        let ws_clone = ws_clone_external.clone();
        let peer_a_clone = peer_a_clone_external.clone();
        let rc_state_clone = rc_state_clone_ext.clone();

        // let res =
        //     setup_rtc_peer_connection_ice_callbacks(peer_a_clone, ws_clone.clone(), rc_state_clone);
        // if res.is_err() {
        //     error!(
        //         "Error Setting up RTCPeerConnection ICE Callbacks {:?}",
        //         res.unwrap_err()
        //     )
        // }

        let ws_clone1 = ws_clone.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let res = setup_rtc_peer_connection_ice_callbacks(peer_a_clone, ws_clone1, rc_state_clone).await;
                if res.is_err() {
                    log::error!("Error Setting up ice callbacks {:?}", res.unwrap_err())
                }
        });


        try_connect_to_session(ws_clone);
    }) as Box<dyn FnMut()>);
    document
        .get_element_by_id("connect_to_session")
        .expect("should have connect_to_session on the page")
        .dyn_ref::<HtmlButtonElement>()
        .expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();

    // Start Remote Video Callback
    let video_element = "peer_b_video".into();
    // let state_lbl = "InitiatorState".into();
    let ice_state_change = rtc_ice_state_change(peer_a.clone(), document, video_element);
    peer_a.set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
    ice_state_change.forget();

    Ok(())
}

fn rtc_ice_state_change(
    rtc_connection: RtcPeerConnection,
    document: Document,
    video_element: String,
) -> Closure<dyn FnMut()> {
    Closure::wrap(Box::new(move || {
        ///////////////////////////////////////////////////////////////
        /////// Start Video When connected
        ///////////////////////////////////////////////////////////////
        match rtc_connection.ice_connection_state() {
            RtcIceConnectionState::Connected => {
                // let remote_streams = rtc_conn.get_senders().to_vec();
                let remote_streams = rtc_connection.get_remote_streams().to_vec();
                debug!("remote_streams {:?}", remote_streams);
                // remote_streams
                if remote_streams.len() == 1 {
                    let first_stream = remote_streams[0].clone();
                    debug!("First Stream {:?}", first_stream);
                    let res_media_stream: Result<MediaStream, _> = first_stream.try_into();
                    let media_stream = res_media_stream.unwrap();
                    debug!("Media Stream {:?}", media_stream);
                    let video_element: Element =
                        document.get_element_by_id(&video_element).unwrap_throw();
                    let vid_elem: HtmlVideoElement =
                        video_element.dyn_into::<HtmlVideoElement>().unwrap_throw();
                    let res = vid_elem.set_src_object(Some(&media_stream));
                    debug!("Result Video Set src Object {:?} ", res);
                }
            }
            _ => {
                warn!("Ice State: {:?}", rtc_connection.ice_connection_state());
            }
        }
    }) as Box<dyn FnMut()>)
}

fn set_html_label(html_label: &str, session_id: String) {
    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document: Document = window.document().expect("Couldn't Get Document");
    document
        .get_element_by_id(html_label)
        .unwrap_or_else(|| panic!("Should have {} on the page", html_label))
        .dyn_ref::<HtmlLabelElement>()
        .expect("#Button should be a be an `HtmlLabelElement`")
        .set_text_content(Some(&session_id));
}

fn get_session_id_from_input() -> String {
    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document: Document = window.document().expect("Couldn't Get Document");
    let sid_input = "sid_input";

    let sid_input = document
        .get_element_by_id(sid_input)
        .unwrap_or_else(|| panic!("Should have {} on the page", sid_input))
        .dyn_ref::<HtmlInputElement>()
        .expect("#HtmlInputElement should be a be an `HtmlInputElement`")
        .value()
        .trim()
        .to_string();
    info!("sid_inputs {}", sid_input);
    sid_input
}

fn set_session_connection_status_error(error: String) {
    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document: Document = window.document().expect("Couldn't Get Document");
    let ws_conn_lbl = "session_connection_status_error";

    let e_string;
    if error.is_empty() {
        e_string = format!("")
    } else {
        e_string = format!("Could not connect: {} ", error)
    }

    document
        .get_element_by_id(ws_conn_lbl)
        .unwrap_or_else(|| panic!("Should have {} on the page", ws_conn_lbl))
        .dyn_ref::<HtmlLabelElement>()
        .expect("#Button should be a be an `HtmlLabelElement`")
        .set_text_content(Some(&e_string));
}

fn try_connect_to_session(ws: WebSocket) {
    let session_id_string = get_session_id_from_input();
    let session_id = SessionID::new(session_id_string);
    let msg = SignalEnum::SessionJoin(session_id);
    let ser_msg: String = match serde_json_wasm::to_string(&msg) {
        Ok(x) => x,
        Err(e) => {
            error!("Could not serialize SessionJoin {}", e);
            return;
        }
    };
    match ws.send_with_str(&ser_msg) {
        Ok(_) => {}
        Err(e) => {
            error!("Error Sending SessionJoin {:?}", e);
        }
    }
}

async fn send_video_offer(rtc_conn: RtcPeerConnection, ws: WebSocket, session_id: SessionID) {
    //  NB !!!
    // Need to setup Media Stream BEFORE sending SDP offer
    // SDP offer Contains information about the Video Streaming technologies available to this and the other browser
    let media_stream = get_video(String::from("peer_a_video"))
        .await
        .expect_throw("Couldn't Get Media Stream");
    debug!("peer_a_video result {:?}", media_stream);
    rtc_conn.add_stream(&media_stream);
    let tracks = media_stream.get_tracks();
    debug!("peer_a_video Tracks {:?}", tracks);

    // Send SDP offer
    let sdp_offer = create_sdp_offer(rtc_conn).await.unwrap_throw();
    let msg = SignalEnum::VideoOffer(sdp_offer, session_id);
    let ser_msg: String = match serde_json_wasm::to_string(&msg) {
        Ok(x) => x,
        Err(e) => {
            error!("Could not serialize VideoOffer {}", e);
            return;
        }
    };

    info!("SDP VideoOffer {}", ser_msg);
    match ws.clone().send_with_str(&ser_msg) {
        Ok(_) => {}
        Err(e) => {
            error!("Error Sending Video Offer {:?}", e);
        }
    }
}
