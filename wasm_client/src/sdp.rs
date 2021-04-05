#[allow(non_snake_case)]


use std::{convert::TryInto, ops::Deref};

use js_sys::{JSON, Promise, Reflect};

use log::{info,warn,error,debug};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture};
use wasm_bindgen::JsCast;

use web_sys::{
    RtcPeerConnection, RtcSdpType,RtcSessionDescriptionInit,
};

use std::rc::Rc;
use std::cell::{RefCell,Cell, RefMut};


//    _____   _____    _____      _    _                       _   _                     
//   / ____| |  __ \  |  __ \    | |  | |                     | | | |                    
//  | (___   | |  | | | |__) |   | |__| |   __ _   _ __     __| | | |   ___   _ __   ___ 
//   \___ \  | |  | | |  ___/    |  __  |  / _` | | '_ \   / _` | | |  / _ \ | '__| / __|
//   ____) | | |__| | | |        | |  | | | (_| | | | | | | (_| | | | |  __/ | |    \__ \
//  |_____/  |_____/  |_|        |_|  |_|  \__,_| |_| |_|  \__,_| |_|  \___| |_|    |___/

#[allow(non_snake_case)]
pub async fn receieve_SDP_answer(peer_A: RtcPeerConnection, answer_sdp:String) -> Result<(),JsValue> {
    warn!("SDP: Receive Answer {:?}", answer_sdp);
    
    // Setting Remote Description
    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    answer_obj.sdp(&answer_sdp);
    let srd_promise = peer_A.set_remote_description(&answer_obj);
    JsFuture::from(srd_promise).await?;
    Ok(())
}

#[allow(non_snake_case)]
pub async fn receieve_SDP_offer_send_answer(peer_B: RtcPeerConnection, offer_sdp:String) -> Result<String,JsValue> {
    warn!("SDP: Video Offer Recieve! {:?}", offer_sdp);

    // Set Remote Description    
    let mut offer_obj =  RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    offer_obj.sdp(&offer_sdp);
    let srd_promise = peer_B.set_remote_description(&offer_obj);
    JsFuture::from(srd_promise).await?;

    // Create SDP Answer
    let answer = JsFuture::from(peer_B.create_answer()).await?;
    let answer_sdp = Reflect::get(&answer,&JsValue::from_str("sdp"))?
        .as_string()
        .unwrap();

    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    answer_obj.sdp(&answer_sdp);

    let sld_promise = peer_B.set_local_description(&answer_obj);
    JsFuture::from(sld_promise).await?;

    info!("SDP: Sending Video Answer {:?}", answer_sdp);
    Ok(answer_sdp)
}

#[allow(non_snake_case)]
pub async fn create_SDP_offer(peer_A:RtcPeerConnection) -> Result<String,JsValue> {

    // Create SDP Offer
    let offer = JsFuture::from(peer_A.create_offer()).await?;
    let offer_sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .unwrap();

    // Set SDP Type -> Offer
    let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    offer_obj.sdp(&offer_sdp);

    // Set SDP Type -> Offer
    let sld_promise = peer_A.set_local_description(&offer_obj);
    JsFuture::from(sld_promise).await?;

    // Send Offer from Peer A -> Peer B Via WebSocket
    info!("SDP: Sending Offer {:?}", offer_sdp);

    Ok(offer_sdp)
}
