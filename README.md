# Web RTC in Rust !
![](https://devopedia.org/images/article/39/4276.1518788244.png)





Starter for your WebRTC needs in Rust 🦀🕸

## Start Right away
To build project root  
Terminal 1 🛠 : `cargo make build`  
  
  
To Run  
Terminal 1 🚀 : `cargo make serve`  
Terminal 2 🔌 : `cargo make servesignal`


## Usefull Terminology
- ICE  : Interactive Connectivity Establishment
- SCTP : Stream Control Transmission Protocol (SCTP)
- SDP  : Session Description Protocol
- STUN : Session Traversal Utilities for NAT
- NAT  : Network Address Translation
- TURN : Traversal Using Relays around NAT
- Signaling: Signaling is the process of sending control information between two devices to determine the communication protocols, channels, media codecs and formats, and method of data transfer, as well as any required routing information. The most important thing to know about the signaling process for WebRTC: it is not defined in the specification.


## The signaling process:
There's a sequence of things that have to happen in order to make it possible to begin a WebRTC session:
1. Each peer creates an RTCPeerConnection object representing their end of the WebRTC session.
2. Each peer establishes a handler for icecandidate events, which handles sending those candidates to the other peer over the signaling channel.
3. Each peer establishes a handler for track event, which is received when the remote peer adds a track to the stream. This code should connect the tracks to its consumer, such as a <video> element.
4. Each peer connects to an agreed-upon signaling server, such as a WebSocket server they both know how to exchange messages with.
5. The person that starts the call Is waiting 


## NB 
Need to setup Media Stream BEFORE sending SDP offer  
SDP offer Contains information about the Video Streamming technologies available to this and the other broswer
