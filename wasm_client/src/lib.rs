#[allow(non_snake_case)]

mod utils;

use std::{convert::TryInto, ops::Deref};

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
    HtmlLabelElement,HtmlInputElement,
    Element, HtmlVideoElement, HtmlButtonElement,
};

use std::rc::Rc;
use std::cell::{RefCell,Cell, RefMut};

// local
mod websockets;
use websockets::*;

mod ice;
use ice::*;

mod sdp;
use sdp::*;

use shared_protocol::*;

// Functions !
async fn handle_message_reply(message:String,rtc_conn:RtcPeerConnection,ws:WebSocket,rc_state: Rc<RefCell<AppState>>) -> Result<(),JsValue> {
    
    let result = match serde_json_wasm::from_str(&message){
        Ok(x)=> x , 
        Err(_) => {
            error!("Could not deserialize Message {} ",message);
            return Ok(()); 
        }
    };
    
    // let session_id = "12345";
    match result {
        SignalEnum::VideoOffer(offer,session_id)=>{
            warn!("VideoOffer Recieved ");
            let sdp_answer = receieve_SDP_offer_send_answer(rtc_conn.clone(),offer).await?;
            let signal = SignalEnum::VideoAnswer(sdp_answer,session_id);
            let response : String  = match serde_json_wasm::to_string(&signal){
                Ok(x) => x,
                Err(e) => {
                    error!("Could not Seralize Video Offer {}",e);
                    return Err(JsValue::from_str("Could not Seralize Video Offer"));
                } 
            };

            match ws.send_with_str(&response) {
                Ok(_) => info!("Video Offer SignalEnum sent"),
                Err(err) => error!("Error sending Video Offer SignalEnum: {:?}", err),
            }
        },
        SignalEnum::VideoAnswer(answer,session_id)=>{
            info!("Video Answer Recieved! {}",answer);
            let res = receieve_SDP_answer(rtc_conn.clone(), answer).await?;
        },
        SignalEnum::IceCandidate(candidate,session_id) =>{
            let x = recieved_new_ice_candidate(candidate,rtc_conn.clone()).await?;
        },
        SignalEnum::SessionReady(session_id) => {
            info!("SessionReady Recieved ! {}",session_id);
            let mut state = rc_state.borrow_mut();
            state.set_session_id(session_id.clone());
            set_session_label(session_id);
        }
        SignalEnum::SessionJoinSuccess(session_id) => {
            info!("SessionJoinSuccess {}",session_id);
            set_session_connection_status_error("".into());
            // Initiate the videocall
            send_video_offer(rtc_conn.clone(),ws.clone(), session_id.clone()).await;
            set_session_connection_status(session_id);
        }
        SignalEnum::SessionJoinError(e) => {
            error!("SessionJoinError! {}",e);
            set_session_connection_status_error(e);
        }
        SignalEnum::SessionJoin(session_id) => {
            info!("{}",session_id)
        }
        SignalEnum::NewUser(user_id) => {
            info!("New User Received ! {}",user_id);
            let mut state = rc_state.borrow_mut();
            state.set_user_id(user_id);
        }
        SignalEnum::ICEError(err,session_id) =>{
            error!("ICEError! {}, {} ",err, session_id);
        }
/////////////////////////////////////////////////////
        remaining =>
        {
            error!("Frontend should not recieve {:?}",remaining);
        }
        // SignalEnum::SessionNew => {
        // }
        // SignalEnum::Debug => {
        //     error!("Frontend should not recieve Debug");
        // }
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
    
    // debug!("media_devices {:?}",media_devices);
    // debug!("media_devices {:?}",navigator.media_devices());
 
    let mut constraints = MediaStreamConstraints::new(); 
    constraints.audio(&JsValue::FALSE);
    constraints.video(&JsValue::TRUE);
    // info!("Constraints {:?}",constraints);

    let stream_promise: Promise = match media_devices.get_user_media_with_constraints(&constraints){ 
        Ok(s) => s,
        Err(e) => return Err(e)
    };

    let document:Document = window.document().expect("Couldn't Get Document");

    let video_element:Element =  match  document.get_element_by_id(&video_id){
        Some(ms) => ms,
        None=> return Err(JsValue::from_str("No Element video found"))
    };

    // info!("video_element {:?}",video_element);
    
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

    // info!("vid_elem {:?}",vid_elem);
    let x = vid_elem.set_src_object(Some(&media_stream));
    // info!("media_stream {:?}",media_stream);

    Ok(media_stream)
}


//  ____    _    _   _______   _______    ____    _   _              _____   ______   _______   _    _   _____  
// |  _ \  | |  | | |__   __| |__   __|  / __ \  | \ | |            / ____| |  ____| |__   __| | |  | | |  __ \ 
// | |_) | | |  | |    | |       | |    | |  | | |  \| |           | (___   | |__       | |    | |  | | | |__) |
// |  _ <  | |  | |    | |       | |    | |  | | | . ` |            \___ \  |  __|      | |    | |  | | |  ___/ 
// | |_) | | |__| |    | |       | |    | |__| | | |\  |            ____) | | |____     | |    | |__| | | |     
// |____/   \____/     |_|       |_|     \____/  |_| \_|           |_____/  |______|    |_|     \____/  |_|     

// TODO: Investigate safety of using .unchecked_ref()
// TODO: remove unwrap Statements

fn setup_show_state(rtc_conn:RtcPeerConnection, state:Rc<RefCell<AppState>>){

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Couldnt Get Document");
    
    // DEBUG BUTTONS
    let rtc_clone_external = rtc_conn.clone();
    let btn_cb = Closure::wrap( Box::new(move || {
        let rtc_clone = rtc_clone_external.clone();
        show_rtc_state(rtc_clone, state.clone());
        }) as Box<dyn FnMut()>
    );

    document
        .get_element_by_id("debug_client_state").expect("should have debug_client_state on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
}


fn show_rtc_state(rtc_conn: RtcPeerConnection, state:Rc<RefCell<AppState>>){

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




fn setup_show_signalling_server_state(ws:WebSocket){

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Couldnt Get Document");
    
    // DEBUG BUTTONS
    let btn_cb = Closure::wrap( Box::new(move || {
            let msg =  SignalEnum::Debug;
            let ser_msg : String  = serde_json_wasm::to_string(&msg).expect("Couldnt Serialize SginalEnum::Debug Message");

            match ws.clone().send_with_str(&ser_msg){
                Ok(_) =>{}
                Err(e) =>{
                    error!("Error Sending SessionNew {:?}",e);
                }
            }
        }) as Box<dyn FnMut()>
    );

    document
        .get_element_by_id("debug_signal_server_state")
        .expect("should have debug_signal_server_state on the page")
        .dyn_ref::<HtmlButtonElement>().expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();
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

pub async fn setup_listenner(pc2: RtcPeerConnection, websocket:WebSocket, rc_state: Rc<RefCell<AppState>>) -> Result<(),JsValue>{

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Couldnt Get Document");
    
    let ws_clone_external = websocket.clone();
    let pc2_clone_external = pc2.clone();
    let document_clone_external = document.clone();
    let rc_state_clone_external= rc_state.clone();

    let btn_cb = Closure::wrap( Box::new(move || {
        let ws_clone = ws_clone_external.clone();
        let pc2_clone= pc2_clone_external.clone();
        let document_clone = document_clone_external.clone();
        let rc_state_clone_interal= rc_state_clone_external.clone();

        ////////////////////////////////////////////////////////////////////////////////////////////////////////////
        // Start Remote Video Callback 
        ////////////////////////////////////////////////////////////////////////////////////////////////////////////
        let videoelem = "peer_a_video".into();

        let ice_state_change = rtc_ice_state_change(pc2_clone.clone(),document_clone.clone(),videoelem);
        pc2_clone.set_oniceconnectionstatechange(Some(ice_state_change.as_ref().unchecked_ref()));
        ice_state_change.forget();

        ////////////////////////////////////////////////////////////////////////////////////////////////////////////
        // Start Local Video Callback 
        ////////////////////////////////////////////////////////////////////////////////////////////////////////////
        let pc2_clone_media= pc2_clone_external.clone();
        wasm_bindgen_futures::spawn_local( async move {
            let mediastream= get_video(String::from("peer_b_video")).await.expect_throw("Couldnt Get Media Stream");
            // debug!("peer_b_video result {:?}", mediastream);
            pc2_clone_media.add_stream(&mediastream);
            let tracks = mediastream.get_tracks();
            // debug!("peer_b_video Tracks {:?}", tracks);
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
        let ws_clone1= ws_clone.clone();
        let rc_state_clone= rc_state_clone_interal.clone();
        wasm_bindgen_futures::spawn_local( async move {
            // Setup ICE callbacks
            let x= setup_RTCPeerConnection_ICECallbacks(pc2_clone,ws_clone1,rc_state_clone).await;
        });
        
        let x= host_session(ws_clone);

        }) as Box<dyn FnMut()>
    );

    document
        .get_element_by_id("start_session")
        .expect("should have start_session on the page")
        .dyn_ref::<HtmlButtonElement>()
        .expect("#Button should be a be an `HtmlButtonElement`")
        .set_onclick(Some(btn_cb.as_ref().unchecked_ref()));
    btn_cb.forget();

    Ok(())
}



fn host_session(ws: WebSocket){
    
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


pub async fn setup_initiator(peer_A: RtcPeerConnection,websocket : WebSocket, rc_state: Rc<RefCell<AppState>>) -> Result<(),JsValue>{

    let window = web_sys::window().expect("No window Found");
    let document:Document = window.document().expect("Couldnt Get Document");

    let ws_clone_external = websocket.clone();
    let peer_A_clone_external = peer_A.clone();
    let rc_state_clone_ext = rc_state.clone();

    /*
    * Create DataChannel on peer_A to negotiate
    * Message will be shown here after connection established
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
            let rc_state_clone = rc_state_clone_ext.clone();

            wasm_bindgen_futures::spawn_local( async move {
                // Setup ICE callbacks
                let res = setup_RTCPeerConnection_ICECallbacks(peer_A_clone.clone(),ws_clone.clone(),rc_state_clone.clone()).await;
                if res.is_err(){
                    error!("Error Setting up RTCPeerConnection ICE Callbacks {:?}",res.unwrap_err())
                }
                //

                try_connect_to_sesison(ws_clone.clone());

                // NB !!!
                // Need to setup Media Stream BEFORE sending SDP offer
                // SDP offer Contains information about the Video Streamming technologies available to this and the other broswer
                // let mediastream= get_video(String::from("peer_a_video")).await.expect_throw("Couldnt Get Media Stream");
                // debug!("peer_a_video result {:?}", mediastream);
                // peer_A_clone.add_stream(&mediastream);
                // let tracks = mediastream.get_tracks();
                // debug!("peer_a_video Tracks {:?}", tracks);

                // Send SDP offer 
                // let mut state = rc_state_clone.borrow_mut();
                // let opt_session_ID= state.get_session_id();
                // match opt_session_ID

                // let session_id = String::from("12345");

                // let sdp_offer = create_SDP_offer(peer_A_clone).await.unwrap_throw();
                // let msg =  SignalEnum::VideoOffer(sdp_offer.into(),session_id);
                // let ser_msg : String  = match serde_json_wasm::to_string(&msg){
                //     Ok(x) => x,
                //     Err(e) => {
                //         error!("Could not Seralize Video Offer {}",e);
                //         return ;
                //     } 
                // };

                // info!("SDP VideoOffer {}",ser_msg);
                // match ws_clone.clone().send_with_str(&ser_msg){
                //     Ok(_) =>{}
                //     Err(e) =>{
                //         error!("Error Sending Video Offer {:?}",e);
                //     }
                // }

            })
        }) as Box<dyn FnMut()>

    );
    document
        .get_element_by_id("connect_to_session").expect("should have connect_to_session on the page")
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


    // ////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

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

fn set_session_label(session_id: String) {

    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document:Document = window.document().expect("Couldnt Get Document");
    let ws_conn_lbl = "sessionid_lbl";

    document
        .get_element_by_id(ws_conn_lbl)
        .expect(&format!("Should have {} on the page",ws_conn_lbl))
        .dyn_ref::<HtmlLabelElement>()
        .expect("#Button should be a be an `HtmlLabelElement`")
        .set_text_content(Some(&format!("{}",session_id)));
}


// inner_text(&self)

fn get_session_id_from_input() -> String {

    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document:Document = window.document().expect("Couldnt Get Document");
    let sid_input = "sid_input";

    let sid_input = document
        .get_element_by_id(sid_input)
        .expect(&format!("Should have {} on the page",sid_input))
        .dyn_ref::<HtmlInputElement>()
        .expect("#HtmlInputElement should be a be an `HtmlInputElement`")
        .value();
    info!("sid_inputs {}", sid_input);
    sid_input
}


fn set_session_connection_status_error(error :String) {
    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document:Document = window.document().expect("Couldnt Get Document");
    let ws_conn_lbl = "session_connection_status_error";

    let e_string;
    if error.len()==0{
        e_string = format!("")
    } else{
        e_string = format!("Could not connect: {} ", error)
    } 

    document
        .get_element_by_id(ws_conn_lbl)
        .expect(&format!("Should have {} on the page",ws_conn_lbl))
        .dyn_ref::<HtmlLabelElement>()
        .expect("#Button should be a be an `HtmlLabelElement`")
        .set_text_content(Some(&e_string));
}


fn set_session_connection_status(id :String) {
    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document:Document = window.document().expect("Couldnt Get Document");
    let ws_conn_lbl = "session_connection_status";

    document
        .get_element_by_id(ws_conn_lbl)
        .expect(&format!("Should have {} on the page",ws_conn_lbl))
        .dyn_ref::<HtmlLabelElement>()
        .expect("#Button should be a be an `HtmlLabelElement`")
        .set_text_content(Some(&id));
}



fn try_connect_to_sesison(ws: WebSocket){
    let session_id = get_session_id_from_input();
    let msg =  SignalEnum::SessionJoin(session_id);
    let ser_msg : String  = match serde_json_wasm::to_string(&msg){
        Ok(x) => x,
        Err(e) => {
            error!("Could not Seralize SessionJoin {}",e);
            return ;
        } 
    };
    match ws.clone().send_with_str(&ser_msg){
        Ok(_) =>{}
        Err(e) =>{
            error!("Error Sending SessionJoin {:?}",e);
        }
    }
}



async fn send_video_offer(rtc_conn:RtcPeerConnection,ws: WebSocket, session_id:String){
    //  NB !!!
    // Need to setup Media Stream BEFORE sending SDP offer
    // SDP offer Contains information about the Video Streamming technologies available to this and the other broswer
    let mediastream= get_video(String::from("peer_a_video")).await.expect_throw("Couldnt Get Media Stream");
    debug!("peer_a_video result {:?}", mediastream);
    rtc_conn.add_stream(&mediastream);
    let tracks = mediastream.get_tracks();
    debug!("peer_a_video Tracks {:?}", tracks);

    // Send SDP offer
    let sdp_offer = create_SDP_offer(rtc_conn).await.unwrap_throw();
    let msg =  SignalEnum::VideoOffer(sdp_offer.into(),session_id);
    let ser_msg : String  = match serde_json_wasm::to_string(&msg){
        Ok(x) => x,
        Err(e) => {
            error!("Could not Seralize Video Offer {}",e);
            return ;
        } 
    };

    info!("SDP VideoOffer {}",ser_msg);
    match ws.clone().send_with_str(&ser_msg){
        Ok(_) =>{}
        Err(e) =>{
            error!("Error Sending Video Offer {:?}",e);
        }
    }
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
    let mut state = AppState { counter: 0, session_id:None, user_id:None };
    let rc_state: Rc<RefCell<AppState>> = Rc::new(RefCell::new(state));
    
    let rtc_conn = RtcPeerConnection::new().unwrap_throw();
    setup_show_state(rtc_conn.clone(), rc_state.clone());
    let websocket =  open_web_socket(rtc_conn.clone(), rc_state.clone()).await.unwrap_throw();
    setup_show_signalling_server_state(websocket.clone());

    setup_listenner(rtc_conn.clone(), websocket.clone() , rc_state.clone()).await.unwrap_throw();
    info!("Setup Listenner");
    setup_initiator(rtc_conn.clone(), websocket.clone() , rc_state.clone()).await.unwrap_throw();
    info!("Setup Initiator");

}



#[derive(Debug)]
pub struct AppState {
    counter: i32,
    session_id:Option<String>,
    user_id:Option<String>
}

impl AppState {
    fn increment(&mut self) {
        self.counter = self.counter + 1;
    }

    fn decrement(&mut self) {
        self.counter = self.counter - 1;
    }
  
    fn set_session_id(&mut self, s_id: String) {
        self.session_id= Some(s_id)
    }

    fn get_session_id(&mut self) -> Option<String>{
        self.session_id.clone()
    }

    fn set_user_id(&mut self, s_id: String) {
        self.user_id= Some(s_id)
    }

    fn get_user_id(&mut self) -> Option<String>{
        self.user_id.clone()
    }

}
