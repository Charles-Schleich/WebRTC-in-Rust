use std::cell::RefCell;
use std::rc::Rc;

use log::{debug, error, info};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, ErrorEvent, HtmlLabelElement, MessageEvent, RtcPeerConnection, WebSocket};

use crate::common::{handle_message_reply, AppState};

// From Workspace

// __          __         _          _____                  _             _
// \ \        / /        | |        / ____|                | |           | |
//  \ \  /\  / /    ___  | |__     | (___     ___     ___  | | __   ___  | |_
//   \ \/  \/ /    / _ \ | '_ \     \___ \   / _ \   / __| | |/ /  / _ \ | __|
//    \  /\  /    |  __/ | |_) |    ____) | | (_) | | (__  |   <  |  __/ | |_
//     \/  \/      \___| |_.__/    |_____/   \___/   \___| |_|\_\  \___|  \__|

const WS_IP_PORT: &str = "ws://0.0.0.0:2794";

pub async fn open_web_socket(
    rtc_conn: RtcPeerConnection,
    rc_state: Rc<RefCell<AppState>>,
) -> Result<WebSocket, JsValue> {
    info!("Opening WS Connection");

    let ws = WebSocket::new(WS_IP_PORT)?;

    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
    let cloned_ws_ext = ws.clone();
    let cloned_state_ext = rc_state;
    //  ON MESSAGE CALLBACK
    let onmessage_callback = Closure::wrap(Box::new(move |ev: MessageEvent| {
        if let Ok(array_buffer) = ev.data().dyn_into::<js_sys::ArrayBuffer>() {
            info!(
                "WS: message event, received arraybuffer: {:?}",
                array_buffer
            );
        } else if let Ok(blob) = ev.data().dyn_into::<web_sys::Blob>() {
            info!("WS: message event, received blob: {:?}", blob);
        } else if let Ok(txt) = ev.data().dyn_into::<js_sys::JsString>() {
            info!("WS: message event, received string: {:?}", txt);
            let rust_string = String::from(txt);
            // put the below line in an async
            let rtc_conn_clone = rtc_conn.clone();
            let cloned_ws = cloned_ws_ext.clone();
            let cloned_state = cloned_state_ext.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let result = handle_message_reply(
                    rust_string,
                    rtc_conn_clone.clone(),
                    cloned_ws.clone(),
                    cloned_state,
                )
                .await;
                match result {
                    Err(x) => error!("{:?}", x),
                    _ => {
                        debug!("Handle Signalling message done")
                    }
                }
            });
        } else {
            info!("message event, received Unknown: {:?}", ev.data());
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let window = web_sys::window().expect("No window Found, We've got bigger problems here");
    let document: Document = window.document().expect("Couldn't Get Document");
    let ws_conn_lbl = "ws_conn_lbl";
    let ws_conn_lbl_err = "ws_conn_lbl_err";

    //  ON ERROR
    let document_clone: Document = document.clone();
    let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
        error!("WS: onerror_callback error event: {:?}", e);

        document_clone
            .get_element_by_id(ws_conn_lbl_err)
            .unwrap_or_else(|| panic!("Should have {} on the page", ws_conn_lbl_err))
            .dyn_ref::<HtmlLabelElement>()
            .expect("#Button should be a be an `HtmlLabelElement`")
            .set_text_content(Some(&format!(
                "{} {} ?",
                "Could not make Websocket Connection, Is the Signalling Server running on: ",
                WS_IP_PORT
            )));
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();

    //  ON OPEN
    let document_clone: Document = document;
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        document_clone
            .get_element_by_id(ws_conn_lbl)
            .unwrap_or_else(|| panic!("Should have {} on the page", ws_conn_lbl))
            .dyn_ref::<HtmlLabelElement>()
            .expect("#Button should be a be an `HtmlLabelElement`")
            .set_text_content(Some(&"Websocket Connected !".to_string()));

        document_clone
            .get_element_by_id(ws_conn_lbl_err)
            .unwrap_or_else(|| panic!("Should have {} on the page", ws_conn_lbl_err))
            .dyn_ref::<HtmlLabelElement>()
            .expect("#Button should be a be an `HtmlLabelElement`")
            .set_text_content(Some(&"".to_string()));
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    // input
    Ok(ws)
}
