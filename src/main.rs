// extern crate futures;
// extern crate libp2p_core;
// extern crate libp2p_noise;
// extern crate libp2p_tcp;
// extern crate log;
// extern crate multiaddr;

// use futures::{
//     future::{self, Either},
//     prelude::*,
// };
// use libp2p_core::identity;
// use libp2p_core::transport::{ListenerEvent, Transport};
// use libp2p_core::upgrade::{self, apply_inbound, apply_outbound, Negotiated};
// use libp2p_noise::{Keypair, NoiseConfig, NoiseError, NoiseOutput, RemoteIdentity, X25519};
// use libp2p_tcp::{TcpConfig, TcpTransStream};
// use multiaddr::Multiaddr;

// fn main() {
//     env_logger::init();
//     let client_id = identity::Keypair::generate_ed25519();

//     let client_id_public = client_id.public();

//     let client_dh = Keypair::<X25519>::new().into_authentic(&client_id).unwrap();
//     let client_transport = TcpConfig::new().and_then(move |output, endpoint| {
//         upgrade::apply(
//             output,
//             NoiseConfig::xx(client_dh),
//             endpoint,
//             upgrade::Version::V1,
//         )
//     });
//     //.and_then(move |out, _| println!("Done client"));

//     futures::executor::block_on(async {
//         let server_address: Multiaddr = "/ip4/127.0.0.1/tcp/23456".parse().unwrap();
//         let client_fut = async {
//             let mut client_session = client_transport
//                 .dial(server_address.clone())
//                 .unwrap()
//                 .await
//                 .map(|(_, session)| session)
//                 .expect("no error");

//             // for m in outbound_msgs {
//             //     let n = (m.0.len() as u64).to_be_bytes();
//             //     client_session.write_all(&n[..]).await.expect("len written");
//             //     client_session.write_all(&m.0).await.expect("no error")
//             // }
//             // client_session.flush().await.expect("no error");
//         };

//         client_fut.await;

//         println!("DONE.");
//     });
// }

extern crate libp2p;

use async_std::{io, task};
use env_logger::{Builder, Env};
use futures::prelude::*;
use libp2p::gossipsub::protocol::MessageId;
use libp2p::gossipsub::{GossipsubEvent, GossipsubMessage, Topic};
use libp2p::{gossipsub, identity, PeerId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use std::{
    error::Error,
    task::{Context, Poll},
};

fn main() -> Result<(), Box<dyn Error>> {
    Builder::from_env(Env::default().default_filter_or("info")).init();

    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {:?}", local_peer_id);

    // Set up an encrypted TCP Transport over the Mplex and Yamux protocols
    let transport = libp2p::build_development_transport(local_key)?;

    // Create a Gossipsub topic
    let topic = Topic::new("test-net".into());

    // Create a Swarm to manage peers and events
    let mut swarm = {
        // to set default parameters for gossipsub use:
        // let gossipsub_config = gossipsub::GossipsubConfig::default();

        // To content-address message, we can take the hash of message and use it as an ID.
        let message_id_fn = |message: &GossipsubMessage| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            MessageId(s.finish().to_string())
        };

        // set custom gossipsub
        let gossipsub_config = gossipsub::GossipsubConfigBuilder::new()
            .heartbeat_interval(Duration::from_secs(10))
            .message_id_fn(message_id_fn) // content-address messages. No two messages of the
            //same content will be propagated.
            .build();
        // build a gossipsub network behaviour
        let mut gossipsub = gossipsub::Gossipsub::new(local_peer_id.clone(), gossipsub_config);
        gossipsub.subscribe(topic.clone());
        libp2p::Swarm::new(transport, gossipsub, local_peer_id)
    };

    // Listen on all interfaces and whatever port the OS assigns
    libp2p::Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();

    // Reach out to another node if specified
    if let Some(to_dial) = std::env::args().nth(1) {
        let dialing = to_dial.clone();
        match to_dial.parse() {
            Ok(to_dial) => match libp2p::Swarm::dial_addr(&mut swarm, to_dial) {
                Ok(_) => println!("Dialed {:?}", dialing),
                Err(e) => println!("Dial {:?} failed: {:?}", dialing, e),
            },
            Err(err) => println!("Failed to parse address to dial: {:?}", err),
        }
    }

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Kick it off
    let mut listening = false;
    task::block_on(future::poll_fn(move |cx: &mut Context| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => swarm.publish(&topic, line.as_bytes()),
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break,
            };
        }

        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(gossip_event)) => match gossip_event {
                    GossipsubEvent::Message(peer_id, id, message) => println!(
                        "Got message: {} with id: {} from peer: {:?}",
                        String::from_utf8_lossy(&message.data),
                        id,
                        peer_id
                    ),
                    _ => {}
                },
                Poll::Ready(None) | Poll::Pending => break,
            }
        }

        if !listening {
            for addr in libp2p::Swarm::listeners(&swarm) {
                println!("Listening on {:?}", addr);
                listening = true;
            }
        }

        Poll::Pending
    }))
}
