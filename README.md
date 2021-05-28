# Web RTC in Rust !
![Ferris](Crustacean_over_ip.png)



Starter for your WebRTC+WASM in Rust 🦀🕸
## Start Right away
To build the wasm, from `/wasm_client/` run  
Terminal 1 🛠 : `cargo make build` or `cargo make watch` (if you plan on tinkering)    
  
To run the servers (Can be done from root directory)  
Terminal 1 🚀 : `cargo make serve`  
Terminal 2 🔌 : `cargo make servesignal`  

Dont forget to set your own ip address for your web-sockets signalling server inside `/wasm_client/src/websockets.rs`

## Useful Terminology
- ICE  : Interactive Connectivity Establishment
- SCTP : Stream Control Transmission Protocol (SCTP)
- SDP  : Session Description Protocol
- STUN : Session Traversal Utilities for NAT
- NAT  : Network Address Translation
- TURN : Traversal Using Relays around NAT
- Signaling: Signaling is the process of sending control information between two devices to determine the communication protocols, channels, media codecs and formats, and method of data transfer, as well as any required routing information. The most important thing to know about the signaling process for WebRTC: it is not defined in the specification.


This is to be read with the following Medium Article
