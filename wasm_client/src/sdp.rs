use js_sys::Reflect;
use log::{info, warn};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcPeerConnection, RtcSdpType, RtcSessionDescriptionInit};

pub async fn receive_sdp_answer(
    peer_a: RtcPeerConnection,
    answer_sdp: String,
) -> Result<(), JsValue> {
    warn!("SDP: Receive Answer {:?}", answer_sdp);

    // Setting Remote Description
    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    answer_obj.sdp(&answer_sdp);
    let srd_promise = peer_a.set_remote_description(&answer_obj);
    JsFuture::from(srd_promise).await?;
    Ok(())
}

pub async fn receive_sdp_offer_send_answer(
    peer_b: RtcPeerConnection,
    offer_sdp: String,
) -> Result<String, JsValue> {
    warn!("SDP: Video Offer Receive! {}", offer_sdp);

    // Set Remote Description
    let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    offer_obj.sdp(&offer_sdp);
    let srd_promise = peer_b.set_remote_description(&offer_obj);
    JsFuture::from(srd_promise).await?;

    // Create SDP Answer
    let answer = JsFuture::from(peer_b.create_answer()).await?;
    let answer_sdp = Reflect::get(&answer, &JsValue::from_str("sdp"))?
        .as_string()
        .unwrap();

    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    answer_obj.sdp(&answer_sdp);

    let sld_promise = peer_b.set_local_description(&answer_obj);
    JsFuture::from(sld_promise).await?;

    info!("SDP: Sending Video Answer {:?}", answer_sdp);
    Ok(answer_sdp)
}

pub async fn create_sdp_offer(peer_a: RtcPeerConnection) -> Result<String, JsValue> {
    // Create SDP Offer
    let offer = JsFuture::from(peer_a.create_offer()).await?;
    let offer_sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .unwrap();

    // Set SDP Type -> Offer
    let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    offer_obj.sdp(&offer_sdp);

    // Set SDP Type -> Offer
    let sld_promise = peer_a.set_local_description(&offer_obj);
    JsFuture::from(sld_promise).await?;

    // Send Offer from Peer A -> Peer B Via WebSocket
    info!("SDP: Sending Offer {:?}", offer_sdp);

    Ok(offer_sdp)
}
