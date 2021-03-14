#[allow(non_snake_case)]


mod utils;

use std::convert::TryInto;

use js_sys::{JSON, Promise, Reflect};

use log::{info,warn,error,debug};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture};
use wasm_bindgen::JsCast;

use web_sys::{
    MessageEvent, 
    RtcDataChannelEvent, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcSdpType,RtcSessionDescriptionInit,
    RtcDataChannel, RtcIceCandidate, RtcIceCandidateInit,  RtcIceConnectionState,
    MediaStream, MediaStreamConstraints,
    Document, 
    WebSocket,
    HtmlLabelElement,
    Element, HtmlVideoElement, HtmlButtonElement,
};

// local
mod websockets;
use websockets::*;

// Functions !
async fn handle_message_reply(message:String,rtc_conn:RtcPeerConnection,ws:WebSocket) -> Result<(),JsValue> {
    let result = match serde_json_wasm::from_str(&message){
        Ok(x)=> x , 
        Err(_) => {
            error!("Could not deserialize Message {} ",message);
            return Ok(()); 
        }
    };

    match result {
        SignallingMessage::VideoOffer(offer)=>{
            warn!("VideoOffer Recieved ");
            let sdp_answer = receieve_SDP_offer_send_answer(rtc_conn.clone(),offer).await?;
            let signal = SignallingMessage::VideoAnswer(sdp_answer);
            let response : String  = match serde_json_wasm::to_string(&signal){
                Ok(x) => x,
                Err(e) => {
                    error!("Could not Seralize Video Offer {}",e);
                    return Err(JsValue::from_str("Could not Seralize Video Offer"));
                } 
            };

            match ws.send_with_str(&response) {
                Ok(_) => info!("Video Offer SignallingMessage sent"),
                Err(err) => error!("Error sending Video Offer SignallingMessage: {:?}", err),
            }
        },
        SignallingMessage::VideoAnswer(answer)=>{
            info!("Video Answer Recieved! {}",answer);
            let res = receieve_SDP_answer(rtc_conn.clone(), answer).await?;
        },
        SignallingMessage::IceCandidate(candidate) =>{
            let x = recieved_new_ice_candidate(candidate,rtc_conn.clone()).await?;
        },
        SignallingMessage::ICEError(err) =>{
            error!("ICEError! {}",err);
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

    let document:Document = window.document().expect("Couldn't Get Document");

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

// As soon as This peer has an ICE Candidate then send it over the websocket connection
#[allow(non_snake_case)]
pub async fn setup_RTCPeerConnection_ICECallbacks(rtc_conn: RtcPeerConnection, ws:WebSocket ) -> Result<RtcPeerConnection,JsValue> {

    let onicecandidate_callback1 = 
        Closure::wrap(
        Box::new(move |ev:RtcPeerConnectionIceEvent| match ev.candidate() {
                Some(candidate) => {
                    let json_obj_candidate = candidate.to_json();
                    let res = JSON::stringify(&json_obj_candidate).unwrap_throw();

                    let js_ob = String::from(res.clone());
                    let signal = SignallingMessage::IceCandidate(js_ob);
                    let ice_candidate : String  = serde_json_wasm::to_string(&signal).unwrap();
                    
                    info!("Sending IceCandidate to Other peer {:?}",res);
                    match ws.send_with_str(&ice_candidate) {
                        Ok(_) => 
                            info!("IceCandidate sent {}",ice_candidate),
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
    warn!("ICECandidate Recieved! {}",candidate);

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

        let mut rtc_ice_init = RtcIceCandidateInit::new(&"");
        rtc_ice_init.candidate(&icecandidate.candidate);
        rtc_ice_init.sdp_m_line_index(Some(icecandidate.sdpMLineIndex));
        rtc_ice_init.sdp_mid(Some(&icecandidate.sdpMid));

        match RtcIceCandidate::new(&rtc_ice_init){
            Ok(x)=>{
                let promise = rtc_conn.clone().add_ice_candidate_with_opt_rtc_ice_candidate(Some(&x));
                let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
                info!("Added other peer's Ice Candidate !");
            }
            Err(e) => {
                info!("Ice Candidate Addition error, {} | {:?}",candidate,e);
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


//  ____    _    _   _______   _______    ____    _   _              _____   ______   _______   _    _   _____  
// |  _ \  | |  | | |__   __| |__   __|  / __ \  | \ | |            / ____| |  ____| |__   __| | |  | | |  __ \ 
// | |_) | | |  | |    | |       | |    | |  | | |  \| |           | (___   | |__       | |    | |  | | | |__) |
// |  _ <  | |  | |    | |       | |    | |  | | | . ` |            \___ \  |  __|      | |    | |  | | |  ___/ 
// | |_) | | |__| |    | |       | |    | |__| | | |\  |            ____) | | |____     | |    | |__| | | |     
// |____/   \____/     |_|       |_|     \____/  |_| \_|           |_____/  |______|    |_|     \____/  |_|     

// TODO: Investigate safety of using .unchecked_ref()
// TODO: remove unwrap Statements

fn setup_show_state(rtc_conn:RtcPeerConnection){

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Couldnt Get Document");
    
    // DEBUG BUTTONS
    let rtc_clone_external = rtc_conn.clone();
    let btn_cb = Closure::wrap( Box::new(move || {
        let rtc_clone = rtc_clone_external.clone();
        show_rtc_state(rtc_clone);
        
        }) as Box<dyn FnMut()>
    );

    document
        .get_element_by_id("print RTC State").expect("should have print RTC State on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
}

fn show_rtc_state(rtc_conn: RtcPeerConnection){

    debug!("===========================");
    debug!("Signalling State : {:?}", rtc_conn.signaling_state());
    debug!("Ice Conn State : {:?}", rtc_conn.ice_connection_state());
    debug!("ice gathering_state : {:?}", rtc_conn.ice_gathering_state());
    debug!("local_description : {:?}", rtc_conn.local_description());
    debug!("remote_description : {:?}", rtc_conn.remote_description());
    debug!("get_senders : {:?}", rtc_conn.get_senders());
    debug!("get_receivers : {:?}", rtc_conn.get_receivers());
    debug!("===========================");

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
    let document:Document = window.document().expect("Couldnt Get Document");
    
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
            let videoelem = "peer_a_video".into();

            // let state_lbl = "ListennerState".into();
            let ice_state_change = rtc_ice_state_change(pc2_clone.clone(),document_clone.clone(),videoelem);
            pc2_clone.set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
            ice_state_change.forget();
            info!("pc2 State 1: {:?}",pc2_clone.signaling_state());

            ////////////////////////////////////////////////////////////////////////////////////////////////////////////
            // Start Local Video Callback 
            ////////////////////////////////////////////////////////////////////////////////////////////////////////////
            let pc2_clone_media= pc2_clone_external.clone();
            wasm_bindgen_futures::spawn_local( async move {
                let mediastream= get_video(String::from("peer_b_video")).await.expect_throw("Couldnt Get Media Stream");
                debug!("peer_b_video result {:?}", mediastream);
                pc2_clone_media.add_stream(&mediastream);
                let tracks = mediastream.get_tracks();
                debug!("peer_b_video Tracks {:?}", tracks);
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
            let x= setup_RTCPeerConnection_ICECallbacks(pc2_clone,ws_clone).await;
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

    Ok(())
}

//  _____           _   _     _           _     _                
// |_   _|         (_) | |   (_)         | |   (_)               
//   | |    _ __    _  | |_   _    __ _  | |_   _    ___    _ __ 
//   | |   | '_ \  | | | __| | |  / _` | | __| | |  / _ \  | '__|
//  _| |_  | | | | | | | |_  | | | (_| | | |_  | | | (_) | | |   
// |_____| |_| |_| |_|  \__| |_|  \__,_|  \__| |_|  \___/  |_|   
                
fn peer_A_dc_on_message(dc:RtcDataChannel) -> Closure<dyn FnMut(MessageEvent)>{
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


pub async fn setup_initiator(peer_A: RtcPeerConnection,websocket : WebSocket) -> Result<(),JsValue>{

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Couldnt Get Document");

    let ws_clone_external = websocket.clone();
    let peer_A_clone_external = peer_A.clone();

    /*
    * Create DataChannel on peer_A to negotiate
    * Message will be shown here after connection established
    *
    */
    info!("peer_A State 1: {:?}",peer_A.signaling_state());
    let dc1 = peer_A.clone().create_data_channel("my-data-channel");
    info!("dc1 created: label {:?}", dc1.label());

    let dc1_clone = dc1.clone();
    let onmessage_callback =  peer_A_dc_on_message(dc1_clone);
    dc1.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let btn_cb = Closure::wrap( Box::new(move || {
            let ws_clone = ws_clone_external.clone();
            let peer_A_clone= peer_A_clone_external.clone();
            wasm_bindgen_futures::spawn_local( async move {
                // Setup ICE callbacks
                let res = setup_RTCPeerConnection_ICECallbacks(peer_A_clone.clone(),ws_clone.clone()).await;
                if res.is_err(){
                    error!("Error Setting up RTCPeerConnection ICE Callbacks {:?}",res.unwrap_err())
                }

                // NB !!!
                // Need to setup Media Stream BEFORE sending SDP offer
                // SDP offer Contains information about the Video Streamming technologies available to this and the other broswer
                let mediastream= get_video(String::from("peer_a_video")).await.expect_throw("Couldnt Get Media Stream");
                // debug!("peer_a_video result {:?}", mediastream);
                peer_A_clone.add_stream(&mediastream);
                // let tracks = mediastream.get_tracks();
                // debug!("peer_a_video Tracks {:?}", tracks);

                // Send SDP offer 
                let sdp_offer = create_SDP_offer(peer_A_clone).await.unwrap_throw();
                let msg =  SignallingMessage::VideoOffer(sdp_offer.into());
                let ser_msg : String  = match serde_json_wasm::to_string(&msg){
                    Ok(x) => x,
                    Err(e) => {
                        error!("Could not Seralize Video Offer {}",e);
                        return ;
                    } 
                };

                info!("SDP VideoOffer {}",ser_msg);
                match ws_clone.clone().send_with_str(&ser_msg){
                    Ok(_) =>{}
                    Err(e) =>{
                        error!("Error Sending Video Offer {:?}",e);
                    }
                }

            })
        }) as Box<dyn FnMut()>

    );
    document
        .get_element_by_id("StartInitiator").expect("should have StartInitiator on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
    
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Start Remote Video Callback 
    ////////////////////////////////////////////////////////////////////////////////////////////////////////////
    let videoelem = "peer_b_video".into();
    // let state_lbl = "InitiatorState".into();
    let ice_state_change = rtc_ice_state_change(peer_A.clone(),document.clone(),videoelem);
    peer_A.set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
    ice_state_change.forget();

    Ok(())
}


//    _____                                                     ______                          _     _                         
//   / ____|                                                   |  ____|                        | |   (_)                        
//  | |        ___    _ __ ___    _ __ ___     ___    _ __     | |__     _   _   _ __     ___  | |_   _    ___    _ __    ___   
//  | |       / _ \  | '_ ` _ \  | '_ ` _ \   / _ \  | '_ \    |  __|   | | | | | '_ \   / __| | __| | |  / _ \  | '_ \  / __|  
//  | |____  | (_) | | | | | | | | | | | | | | (_) | | | | |   | |      | |_| | | | | | | (__  | |_  | | | (_) | | | | | \__ \  
//   \_____|  \___/  |_| |_| |_| |_| |_| |_|  \___/  |_| |_|   |_|       \__,_| |_| |_|  \___|  \__| |_|  \___/  |_| |_| |___/  

fn rtc_ice_state_change(rtc_conn:RtcPeerConnection, document:Document, videoelem: String)-> Closure<dyn FnMut()>{
    
    Closure::wrap( Box::new(move || {
        // document
        //         .get_element_by_id(&state_lbl)
        //         .expect(&format!("Should have {} on the page",state_lbl))
        //         .dyn_ref::<HtmlLabelElement>()
        //         .expect("#Button should be a be an `HtmlLabelElement`")
        //         .set_text_content(Some(&format!("{:?}",rtc_conn.ice_connection_state())));

        ///////////////////////////////////////////////////////////////
        /////// Start Video When connected  
        ///////////////////////////////////////////////////////////////
        match rtc_conn.ice_connection_state(){
            RtcIceConnectionState::Connected=> {
                // TODO:  Add Audio track here
            
                // let remote_streams = rtc_conn.get_senders().to_vec();
                let remote_streams = rtc_conn.get_remote_streams().to_vec();
                debug!("remote_streams {:?}",remote_streams);
                // remote_streams
                if remote_streams.len() == 1 {
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
