mod utils;

use std::convert::TryInto;

use js_sys::{JSON, Promise, Reflect};

use wasm_bindgen::prelude::*;

use web_sys::{
    MessageEvent, 
    RtcDataChannelEvent, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType,RtcSessionDescriptionInit,
    RtcDataChannel, RtcIceCandidate, RtcIceCandidateInit,  RtcIceConnectionState,
    Navigator, 
    MediaDevices, MediaStream, MediaStreamConstraints,
    Document, 
    ErrorEvent, WebSocket,
    EventListener, HtmlInputElement, HtmlLabelElement,
    Element, HtmlVideoElement, HtmlButtonElement,
};
use log::{info,warn,error,debug};
 
use wasm_bindgen_futures::{JsFuture, spawn_local};
use wasm_bindgen::JsCast;

// local
mod websockets;
use websockets::*;

// Functions !
async fn handle_message_reply(message:String,rtc_conn:RtcPeerConnection,ws:WebSocket) -> Result<(),JsValue> {
    // debug!("Handling message {}",message);
    let result = match serde_json_wasm::from_str(&message){
        Ok(x)=> x , 
        Err(_) => {
            error!("Could not deserialize Message {} ",message);
            return Ok(()); 
        }
    };

    match result {
        SignallingMessage::VideoOffer(offer)=>{
            info!("VideoOffer! {}",offer);
            let sdp_answer = receieve_SDP_offer_send_answer(rtc_conn.clone(),offer).await?;
            let signal = SignallingMessage::VideoAnswer(sdp_answer);
            let response : String  = match serde_json_wasm::to_string(&signal){
                Ok(x) => x,
                Err(e) => {
                    error!("Could not Seralize Video Answer {}",e);
                    "Could not Seralize Video Offer".into()
                } 
            };

            match ws.send_with_str(&response) {
                Ok(_) => warn!("VideoAnswer SignallingMessage  sent"),
                Err(err) => error!("error sending VideoAnswer SignallingMessage: {:?}", err),
            }
        },
        SignallingMessage::VideoAnswer(answer)=>{
            info!("VideoAnswer Recieved! {}",answer);
            let res = receieve_SDP_answer(rtc_conn.clone(), answer).await?;
        },
        SignallingMessage::IceCandidate(candidate) =>{
            let x = recieved_new_ice_candidate(candidate,rtc_conn.clone()).await?;
        },
        SignallingMessage::ICEError(err) =>{
            warn!("ICEError! {}",err);
        }

    };
    Ok(())
}


//   _____          _      __      __  _       _                
//  / ____|        | |     \ \    / / (_)     | |               
// | |  __    ___  | |_     \ \  / /   _    __| |   ___    ___  
// | | |_ |  / _ \ | __|     \ \/ /   | |  / _` |  / _ \  / _ \ 
// | |__| | |  __/ | |_       \  /    | | | (_| | |  __/ | (_) |
// \_____|  \___|  \__|       \/     |_|  \__,_|  \___|  \___/ 
                                                              

#[wasm_bindgen]
pub async fn get_video(video_id: String) -> Result<MediaStream,JsValue>{
    info!("Starting Video Device Capture!");
    let window = web_sys::window().expect("No window Found");
    let navigator = window.navigator();
    let media_devices= match navigator.media_devices() {
        Ok(md)=> md,
        Err(e) =>  return  Err(e) 
    };
    
    debug!("media_devices {:?}",media_devices);
    debug!("media_devices {:?}",navigator.media_devices());
 
    let mut constraints = MediaStreamConstraints::new(); 
    constraints.audio(&JsValue::FALSE);
    constraints.video(&JsValue::TRUE);
    info!("Constraints {:?}",constraints);

    let stream_promise: Promise = match media_devices.get_user_media_with_constraints(&constraints){ 
        Ok(s) => s,
        Err(e) => return Err(e)
    };

    let document:Document = window.document().expect("Coudlnt Get Document");
    debug!("after Doc ",);

    // let video_element:Element =  document.get_element_by_id(&video).expect(&format!("Could not get video with id {}", video));
    let video_element:Element =  match  document.get_element_by_id(&video_id){
        Some(ms) => ms,
        None=> return Err(JsValue::from_str("No Element video found"))
    };

    info!("video_element {:?}",video_element);
    
    let media_stream: MediaStream = match wasm_bindgen_futures::JsFuture::from(stream_promise).await {
        Ok(ms) => MediaStream::from(ms),
        Err(e) => {
            error!("{:?}",e);
            error!("{:?}","Its possible that the There is already a tab open with a handle to the Media Stream");
            error!("{:?}","Check if Other tab is open with Video/Audio Stream open");
            return Err(JsValue::from_str("User Did not allow access to the Camera"))
        }
    };
    
    let vid_elem : HtmlVideoElement = match video_element.dyn_into::<HtmlVideoElement>(){
        Ok(x)=>x,
        Err(e) => {
            error!("{:?}",e);
            return Err(JsValue::from_str("User Did not allow access to the Camera"))
        }
    };

    info!("vid_elem {:?}",vid_elem);
    let x = vid_elem.set_src_object(Some(&media_stream));
    info!("media_stream {:?}",media_stream);

    Ok(media_stream)
}


//  _____    _____   ______     _   _                          _     _           _     _                 
// |_   _|  / ____| |  ____|   | \ | |                        | |   (_)         | |   (_)                
//   | |   | |      | |__      |  \| |   ___    __ _    ___   | |_   _    __ _  | |_   _    ___    _ __  
//   | |   | |      |  __|     | . ` |  / _ \  / _` |  / _ \  | __| | |  / _` | | __| | |  / _ \  | '_ \ 
//  _| |_  | |____  | |____    | |\  | |  __/ | (_| | | (_) | | |_  | | | (_| | | |_  | | | (_) | | | | |
// |_____|  \_____| |______|   |_| \_|  \___|  \__, |  \___/   \__| |_|  \__,_|  \__| |_|  \___/  |_| |_|
//                                              __/ |                                                    
//                                             |___/                                                     

pub async fn setup_RTCPeerConnection_ICECallbacks(rtc_conn: RtcPeerConnection, ws:WebSocket ) -> Result<RtcPeerConnection,JsValue> {

    // let pc2_clone = pc2.clone();
    let onicecandidate_callback1 = 
        Closure::wrap(
        Box::new(move |ev:RtcPeerConnectionIceEvent| match ev.candidate() {
                Some(candidate) => {
                    info!("pc1.onicecandidate: {:#?}", candidate.candidate());
                    warn!("IceCandidate candidate {:?}",candidate); // candidate = RtcIceCandidate
                    let json_obj_candidate = candidate.to_json();
                    let res = JSON::stringify(&json_obj_candidate).unwrap_throw();
                    warn!("IceCandidate as_string() {:?}",res);

                    let js_ob = String::from(res);
                    let signal = SignallingMessage::IceCandidate(js_ob);
                    let ice_candidate : String  = serde_json_wasm::to_string(&signal).unwrap();
                    match ws.send_with_str(&ice_candidate) {
                        Ok(_) => 
                            warn!("IceCandidate sent {}",ice_candidate),
                        Err(err) => 
                            error!("error sending IceCandidate SignallingMessage: {:?}", err),
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

pub async fn recieved_new_ice_candidate(candidate:String, rtc_conn: RtcPeerConnection) -> Result<(),JsValue>{
    debug!("ICECandidate Recieved! {}",candidate);

    if candidate.eq("") {
        info!("ICECandidate! is empty doing nothing");
    } else{

        let icecandidate:IceCandidateSend = match serde_json_wasm::from_str(&candidate){
            Ok(x)=> x , 
            Err(e) => {
                let message  = format!("Could not deserialize Ice Candidate {} ",candidate);
                return Err(JsValue::from_str(&message)); 
            }
        };
        debug!("ICECandidate Recieved! DEBUG {:?}",icecandidate);

        let mut rtc_ice_init = RtcIceCandidateInit::new(&"");
        rtc_ice_init.candidate(&icecandidate.candidate);
        rtc_ice_init.sdp_m_line_index(Some(icecandidate.sdpMLineIndex));
        rtc_ice_init.sdp_mid(Some(&icecandidate.sdpMid));

        match RtcIceCandidate::new(&rtc_ice_init){
            Ok(x)=>{
                let promise = rtc_conn.clone().add_ice_candidate_with_opt_rtc_ice_candidate(Some(&x));
                let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
                debug!("Ice Candidate Add Result {:?}",result);
            }
            Err(e) => {
                debug!("ice candidate creation error, {} |||||| {:?}",candidate,e);
                return Err(e);
            } 
        };
    }
    Ok(())
}

//    _____   _____    _____      _    _                       _   _                     
//   / ____| |  __ \  |  __ \    | |  | |                     | | | |                    
//  | (___   | |  | | | |__) |   | |__| |   __ _   _ __     __| | | |   ___   _ __   ___ 
//   \___ \  | |  | | |  ___/    |  __  |  / _` | | '_ \   / _` | | |  / _ \ | '__| / __|
//   ____) | | |__| | | |        | |  | | | (_| | | | | | | (_| | | | |  __/ | |    \__ \
//  |_____/  |_____/  |_|        |_|  |_|  \__,_| |_| |_|  \__,_| |_|  \___| |_|    |___/

pub async fn receieve_SDP_answer(pc1: RtcPeerConnection, answer_sdp:String) -> Result<(),JsValue> {

    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    answer_obj.sdp(&answer_sdp);
    let srd_promise = pc1.set_remote_description(&answer_obj);
    JsFuture::from(srd_promise).await?;
    debug!("pc1: signaling_state {:?}", pc1.signaling_state());
    debug!("pc1: ice_connection_state {:?}", pc1.ice_connection_state());
    Ok(())
}


pub async fn receieve_SDP_offer_send_answer(pc2: RtcPeerConnection, offer_sdp:String) -> Result<String,JsValue> {

    let mut offer_obj =  RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    offer_obj.sdp(&offer_sdp);
    let srd_promise = pc2.set_remote_description(&offer_obj);
    JsFuture::from(srd_promise).await?;
    info!("pc2: state After Offer {:?}", pc2.signaling_state());
    
    let answer = JsFuture::from(pc2.create_answer()).await?;
    let answer_sdp = Reflect::get(&answer,&JsValue::from_str("sdp"))?
        .as_string()
        .unwrap();
    info!("pc2: answer {:?}", answer_sdp);
    let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    answer_obj.sdp(&answer_sdp);

    let sld_promise = pc2.set_local_description(&answer_obj);
    JsFuture::from(sld_promise).await?;
    info!("pc2: signaling_state {:?}", pc2.signaling_state());
    info!("pc2: ice_connection_state {:?}", pc2.ice_connection_state());
    Ok(answer_sdp)
}

pub async fn create_SDP_offer(pc1:RtcPeerConnection) -> Result<String,JsValue> {

    // Create SDP Offer
    let offer = JsFuture::from(pc1.create_offer()).await?;
    let offer_sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .unwrap();
    // info!("pc1: offer ! {:?}", offer_sdp);

    // Create SDP Offer
    let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    offer_obj.sdp(&offer_sdp);

    let sld_promise = pc1.set_local_description(&offer_obj);
    JsFuture::from(sld_promise).await?;
    debug!("pc1: signaling_state {:?}", pc1.signaling_state());
    debug!("pc1: ice_connection_state {:?}", pc1.ice_connection_state());
    // SEND OFFER from PC1 -> PC2 VIA WEB SOCKET HERE 
    Ok(offer_sdp)
}


//  ____    _    _   _______   _______    ____    _   _              _____   ______   _______   _    _   _____  
// |  _ \  | |  | | |__   __| |__   __|  / __ \  | \ | |            / ____| |  ____| |__   __| | |  | | |  __ \ 
// | |_) | | |  | |    | |       | |    | |  | | |  \| |           | (___   | |__       | |    | |  | | | |__) |
// |  _ <  | |  | |    | |       | |    | |  | | | . ` |            \___ \  |  __|      | |    | |  | | |  ___/ 
// | |_) | | |__| |    | |       | |    | |__| | | |\  |            ____) | | |____     | |    | |__| | | |     
// |____/   \____/     |_|       |_|     \____/  |_| \_|           |_____/  |______|    |_|     \____/  |_|     

// TODO: Investigate safety of using .unchecked_ref()
// TODO: remove Unwrap Statements

#[wasm_bindgen]
pub async fn setup_click_button(rtc_conn: RtcPeerConnection, websocket : WebSocket) -> Result<(),JsValue>{
    
    info!("Setup Button Clicks !");
    // let window = web_sys::window().expect("No window Found");
    // let document:Document = window.document().expect("Coudlnt Get Document");

    // DEBUG BUTTONS 
    setup_show_state(rtc_conn.clone());
    Ok(())
}

fn setup_show_state(rtc_conn:RtcPeerConnection){

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Coudlnt Get Document");
    
    // DEBUG BUTTONS
    let rtc_clone_external = rtc_conn.clone();
    let btn_cb = Closure::wrap( Box::new(move || {
        // let ws_clone = websocket.clone();
        let rtc_clone = rtc_clone_external.clone();
        show_rtc_state("pc1",rtc_clone);

        }) as Box<dyn FnMut()>

    );

    document
        .get_element_by_id("print RTC State").expect("should have print RTC State on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
}

fn show_rtc_state(name:&str, rtc_conn: RtcPeerConnection){

    debug!("===========================");
    debug!("{}: Signalling State {:?}",name, rtc_conn.signaling_state());
    debug!("{}: Ice Conn State {:?}",name, rtc_conn.ice_connection_state());
    debug!("{}: ice gathering_state {:?}",name, rtc_conn.ice_gathering_state());
    debug!("{}: local_description {:?}",name, rtc_conn.local_description());
    debug!("{}: remote_description {:?}",name, rtc_conn.remote_description());

    debug!("{}: get_senders {:?}",name, rtc_conn.get_senders());
    debug!("{}: get_receivers {:?}",name, rtc_conn.get_receivers());
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//  _____    _______    _____   
// |  __ \  |__   __|  / ____|  
// | |__) |    | |    | |       
// |  _  /     | |    | |       
// | | \ \     | |    | |____   
// |_|  \_\    |_|     \_____|  

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//  _        _         _                                        
// | |      (_)       | |                                       
// | |       _   ___  | |_    ___   _ __    _ __     ___   _ __ 
// | |      | | / __| | __|  / _ \ | '_ \  | '_ \   / _ \ | '__|
// | |____  | | \__ \ | |_  |  __/ | | | | | | | | |  __/ | |   
// |______| |_| |___/  \__|  \___| |_| |_| |_| |_|  \___| |_|   

pub async fn setup_listenner(pc2: RtcPeerConnection, websocket:WebSocket) -> Result<(),JsValue>{

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Coudlnt Get Document");
    
    let ws_clone_external = websocket.clone();
    let pc2_clone_external = pc2.clone();

    let document_clone_external = document.clone();
    let btn_cb = Closure::wrap( Box::new(move || {
            let ws_clone = ws_clone_external.clone();
            let pc2_clone= pc2_clone_external.clone();
            let document_clone = document_clone_external.clone();

            ////////////////////////////////////////////////////////////////////////////////////////////////////////////
            // Start Remote Video Callback 
            ////////////////////////////////////////////////////////////////////////////////////////////////////////////
            let videoelem = "peer_1_video".into();
            let state_lbl = "ListennerState".into();
            let ice_state_change = pc1_ice_state_change(pc2_clone.clone(),document_clone.clone(),videoelem,state_lbl);
            pc2_clone.set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
            ice_state_change.forget();
            info!("pc2 State 1: {:?}",pc2_clone.signaling_state());

            ////////////////////////////////////////////////////////////////////////////////////////////////////////////
            // Start Local Video Callback 
            ////////////////////////////////////////////////////////////////////////////////////////////////////////////
            let pc2_clone_media= pc2_clone_external.clone();
            wasm_bindgen_futures::spawn_local( async move {
                let mediastream= get_video(String::from("peer_2_video")).await.expect_throw("Couldnt Get Media Stream");
                debug!("peer_2_video result {:?}", mediastream);
                pc2_clone_media.add_stream(&mediastream);
                let tracks = mediastream.get_tracks();
                debug!("peer_2_video Tracks {:?}", tracks);
            });

            // NB !!!
            // Need to setup Media Stream BEFORE sending SDP offer
            // SDP offer Contains information about the Video Streamming technologies available to this and the other broswer
            /*
            * If negotiation has done, this closure will be called
            *
            */
            let ondatachannel_callback =
                Closure::wrap(Box::new(move | ev: RtcDataChannelEvent| {
                    let dc2 = ev.channel();
                    info!("pc2.ondatachannel! : {}", dc2.label());
                    let onmessage_callback = 
                        Closure::wrap(
                            Box::new(move |ev: MessageEvent| match ev.data().as_string(){
                                Some(message) => warn!("{:?}", message),
                                None => {}
                            }) as Box<dyn FnMut(MessageEvent)>,
                        );
            dc2.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
            onmessage_callback.forget();

            dc2.send_with_str("Ping from pc2.dc!").unwrap();

        }) as Box<dyn FnMut(RtcDataChannelEvent)>);
        
        pc2_clone.set_ondatachannel(Some(ondatachannel_callback.as_ref().unchecked_ref()));
        ondatachannel_callback.forget();

        let pc2_clone= pc2_clone_external.clone();
        wasm_bindgen_futures::spawn_local( async move {
            // Setup ICE callbacks
            setup_RTCPeerConnection_ICECallbacks(pc2_clone,ws_clone).await;
        });


        }) as Box<dyn FnMut()>
    );

    document
        .get_element_by_id("StartListenner")
        .expect("should have StartListenner on the page")
        .dyn_ref::<HtmlButtonElement>()
        .expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();

    // ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    // // Start Remote Video Callback 
    // ////////////////////////////////////////////////////////////////////////////////////////////////////////////
  

    Ok(())
}

//  _____           _   _     _           _     _                
// |_   _|         (_) | |   (_)         | |   (_)               
//   | |    _ __    _  | |_   _    __ _  | |_   _    ___    _ __ 
//   | |   | '_ \  | | | __| | |  / _` | | __| | |  / _ \  | '__|
//  _| |_  | | | | | | | |_  | | | (_| | | |_  | | | (_) | | |   
// |_____| |_| |_| |_|  \__| |_|  \__,_|  \__| |_|  \___/  |_|   
                
// Function that will On dc messacge
fn pc1_dc_on_message(dc:RtcDataChannel) -> Closure<dyn FnMut(MessageEvent)>{
    Closure::wrap( 
   Box::new(move |ev: MessageEvent| match ev.data().as_string(){
            Some(message) => {
                warn!("{:?}", message);
                dc.send_with_str("Pongity Pong from pc1.dc!").unwrap();
            }
            None => {}
        }) as Box<dyn FnMut(MessageEvent)>,
    )
}


fn pc1_dc_send_message(dc:RtcDataChannel, document:Document) -> Closure<dyn FnMut()>{
    Closure::wrap( Box::new(move || {
        let message = document
            .get_element_by_id("local_message").expect("should have local_message on the page")
            .dyn_ref::<HtmlInputElement>().expect("#Button should be a be an `HtmlInputElement`")
            .value();
        let mess = dc.send_with_str(&message);
        if mess.is_err(){
            debug!("ERROR sending message via DC: {:?}", mess);
        }
    }) as Box<dyn FnMut()>)
}

fn pc1_ice_state_change(rtc_conn:RtcPeerConnection, document:Document, videoelem: String, state_lbl:String)-> Closure<dyn FnMut()>{
    
    Closure::wrap( Box::new(move || {
        document
                .get_element_by_id(&state_lbl)
                .expect(&format!("should have {} on the page",state_lbl))
                .dyn_ref::<HtmlLabelElement>()
                .expect("#Button should be a be an `HtmlLabelElement`")
                .set_text_content(Some(&format!("{:?}",rtc_conn.ice_connection_state())));

        ///////////////////////////////////////////////////////////////
        /////// Start Video When connected  
        ///////////////////////////////////////////////////////////////
        match rtc_conn.ice_connection_state(){
            RtcIceConnectionState::Connected=> {
                let remote_streams = rtc_conn.get_remote_streams().to_vec();
                debug!("remote_streams {:?}",remote_streams);
                // remote_streams
                if remote_streams.len() ==1 {
                    let first_stream = remote_streams[0].clone();
                    debug!("First Stream {:?}",first_stream);
                    let res_media_stream: Result<MediaStream,_> = first_stream.try_into();
                    let media_stream = res_media_stream.unwrap();
                    debug!("Media Stream {:?}",media_stream);
                    let video_element:Element =  document.get_element_by_id(&videoelem).unwrap_throw();
                    let vid_elem : HtmlVideoElement = video_element.dyn_into::<HtmlVideoElement>().unwrap_throw();
                    let x = vid_elem.set_src_object(Some(&media_stream));
                    debug!("Result Video Set src Object {:?} ", x);
                }
            }
            _ => {
                warn!("Ice State {:?}",rtc_conn.ice_connection_state());
            },
        }
    }) as Box<dyn FnMut()>)
}


pub async fn setup_initiator(pc1: RtcPeerConnection,websocket : WebSocket) -> Result<(),JsValue>{

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Coudlnt Get Document");

    let ws_clone_external = websocket.clone();
    let pc1_clone_external = pc1.clone();
    
    /*
    * Create DataChannel on pc1 to negotiate
    * Message will be shown here after connection established
    *
    */
    info!("pc1 State 1: {:?}",pc1.signaling_state());
    let dc1 = pc1.clone().create_data_channel("my-data-channel");
    info!("dc1 created: label {:?}", dc1.label());

    let dc1_clone = dc1.clone();
    let onmessage_callback =  pc1_dc_on_message(dc1_clone);
    dc1.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let btn_cb = Closure::wrap( Box::new(move || {
            let ws_clone = ws_clone_external.clone();
            let pc1_clone= pc1_clone_external.clone();
            wasm_bindgen_futures::spawn_local( async move {
                // Setup ICE callbacks
                let res = setup_RTCPeerConnection_ICECallbacks(pc1_clone.clone(),ws_clone.clone()).await;
                if res.is_err(){
                    error!("Error Setting up RTCPeerConnection ICE Callbacks {:?}",res.unwrap_err())
                }

                // NB !!!
                // Need to setup Media Stream BEFORE sending SDP offer
                // SDP offer Contains information about the Video Streamming technologies available to this and the other broswer
                let mediastream= get_video(String::from("peer_1_video")).await.expect_throw("Couldnt Get Media Stream");
                debug!("Peer_1_video result {:?}", mediastream);
                pc1_clone.add_stream(&mediastream);
                let tracks = mediastream.get_tracks();
                debug!("Peer_1_video Tracks {:?}", tracks);

                // Send SDP offer 
                let sdp_offer = create_SDP_offer(pc1_clone).await.unwrap_throw();
                let msg =  SignallingMessage::VideoOffer(sdp_offer.into());
                let ser_msg : String  = match serde_json_wasm::to_string(&msg){
                    Ok(x) => x,
                    Err(e) => {
                        error!("Could not Seralize Video Offer {}",e);
                        "Could not Seralize Video Offer".into()
                    } 
                };

                warn!("VideoOffer {}",ser_msg);
                let x = ws_clone.clone().send_with_str(&ser_msg);
            })
        }) as Box<dyn FnMut()>

    );
    document
        .get_element_by_id("StartInitiator").expect("should have StartInitiator on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();

    ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Send Message Callback
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    let btn_cb = pc1_dc_send_message(dc1.clone(),document.clone());
    document
        .get_element_by_id("SendMessage").expect("should have SendMessage on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
    
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Start Remote Video Callback 
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    let videoelem = "peer_2_video".into();
    let state_lbl = "InitiatorState".into();
    let ice_state_change = pc1_ice_state_change(pc1.clone(),document.clone(),videoelem,state_lbl);
    pc1.set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
    ice_state_change.forget();


    Ok(())
}




//  __  __           _         
// |  \/  |         (_)        
// | \  / |   __ _   _   _ __  
// | |\/| |  / _` | | | | '_ \ 
// | |  | | | (_| | | | | | | |
// |_|  |_|  \__,_| |_| |_| |_|
                            
                            
#[wasm_bindgen(start)]
pub async fn start(){
    wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    let rtc_conn = RtcPeerConnection::new().unwrap_throw();
    setup_show_state(rtc_conn.clone());

    let websocket =  open_web_socket(rtc_conn.clone()).await.unwrap_throw();
    setup_listenner(rtc_conn.clone(),websocket.clone() ).await.unwrap_throw();
    setup_initiator(rtc_conn.clone(),websocket.clone()).await.unwrap_throw();

}
