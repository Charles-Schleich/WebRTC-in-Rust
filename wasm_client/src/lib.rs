mod common;
mod ice;
mod sdp;
mod utils;
mod websockets;

use std::cell::RefCell;
use std::rc::Rc;

use log::info;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::UnwrapThrowExt;

use common::{
    create_stun_peer_connection, create_turn_peer_connection, create_plain_peer_connection, setup_initiator, setup_listener,
    setup_show_signalling_server_state, setup_show_state, AppState,
};
use ice::{received_new_ice_candidate, setup_rtc_peer_connection_ice_callbacks};
use sdp::{create_sdp_offer, receive_sdp_answer, receive_sdp_offer_send_answer};
use utils::set_panic_hook;
use websockets::open_web_socket;

#[wasm_bindgen(start)]
pub async fn start() {
    set_panic_hook();

    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));

    let state: Rc<RefCell<AppState>> = Rc::new(RefCell::new(AppState::new()));

    let rtc_connection = create_plain_peer_connection().unwrap_throw();

    // To run the TURN client you need to be running a turn server as well.
    // let rtc_connection = create_turn_peer_connection().unwrap_throw(); 
    
    let websocket = open_web_socket(rtc_connection.clone(), state.clone())
        .await
        .unwrap_throw();

    setup_show_state(rtc_connection.clone(), state.clone());
    setup_show_signalling_server_state(websocket.clone());

    setup_initiator(rtc_connection.clone(), websocket.clone(), state.clone())
        .await
        .unwrap_throw();
    info!("Setup Initiator");

    setup_listener(rtc_connection.clone(), websocket.clone(), state.clone())
        .await
        .unwrap_throw();
    info!("Setup Listener");
}
