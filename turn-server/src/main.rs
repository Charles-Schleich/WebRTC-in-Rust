use turn::auth::*;
use turn::relay::relay_range::RelayAddressGeneratorRanges;
use turn::relay::relay_static::RelayAddressGeneratorStatic;
use turn::server::{config::*, *};
use turn::Error;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::signal;
use tokio::time::Duration;
use util::vnet::net::*;

struct MyAuthHandler {
    cred_map: HashMap<String, Vec<u8>>,
}

impl MyAuthHandler {
    fn new(cred_map: HashMap<String, Vec<u8>>) -> Self {
        MyAuthHandler { cred_map }
    }
}

impl AuthHandler for MyAuthHandler {
    fn auth_handle(
        &self,
        username: &str,
        _realm: &str,
        src_addr: SocketAddr,
    ) -> Result<Vec<u8>, Error> {
        // println!("Attempt: username={}", username);
        println!("src_addr={}", src_addr);
        println!("username={}", username);
        if let Some(pw) = self.cred_map.get(username) {
            println!("  password={:?}", pw);
            Ok(pw.to_vec())
        } else {
            Err(Error::ErrFakeErr)
        }
    }
}

// RUST_LOG=trace cargo run --color=always

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    
    // const SHARED_SECRET: &str = "HELLO_WORLD";
    // let long_term_auth_handler= LongTermAuthHandler::new(SHARED_SECRET.to_string());
    // let (user, pass) = generate_long_term_credentials(SHARED_SECRET, Duration::from_secs(600000))?;
    

    let public_ip = "192.168.178.60";
    let port = "3478";
    let realm = "realm";

    let mut cred_map: HashMap<String, Vec<u8>> = HashMap::new();
    for user_id in 0..11 {
        let user = format!("user{}",user_id);
        let pass = format!("pass{}",user_id);
        println!("{} : {}",user,pass);
        let key = generate_auth_key(&user, realm, &pass);
        cred_map.insert(user.to_owned(), key);
    }

    let conn = Arc::new(UdpSocket::bind(format!("192.168.178.60:{}", port)).await?);
    // let conn = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", port)).await?);
    println!("listening {}...", conn.local_addr()?);


    let box_relay_adress_genenrator_range = Box::new(RelayAddressGeneratorRanges{
        relay_address: IpAddr::from_str(public_ip)?,
        min_port: 3000,
        max_port: 60000,
        max_retries: 10,
        address: "192.168.178.60".to_owned(),
        net: Arc::new(Net::new(None)),
    });


    let box_relay_adress_gen_static = Box::new(RelayAddressGeneratorStatic{
        relay_address: IpAddr::from_str(public_ip)?,
        address: "0.0.0.0".to_owned(),
        net: Arc::new(Net::new(None)),
    });

    // Box::new(RelayAddressGeneratorStatic {
    //     relay_address: IpAddr::from_str(public_ip)?,
    //     address: "0.0.0.0".to_owned(),
    //     net: Arc::new(Net::new(None)),
    // });

    let server = Server::new(ServerConfig {
        conn_configs: vec![ConnConfig {
            conn,
            relay_addr_generator: box_relay_adress_genenrator_range
            // relay_addr_generator: box_relay_adress_gen_static
        }],
        realm: realm.to_owned(),
        
        // auth_handler: Arc::new(long_term_auth_handler),
        auth_handler: Arc::new(MyAuthHandler::new(cred_map)),
        channel_bind_timeout: Duration::from_secs(600),
    })
    .await?;

    println!("Waiting for Ctrl-C...");
    signal::ctrl_c().await.expect("failed to listen for event");
    println!("\nClosing connection now...");
    server.close().await?;

    Ok(())
}