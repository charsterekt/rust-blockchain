// Imports
use super::{App, Block};
use libp2p::{
    floodsub::{Floodsub, FloodsubEvent, Topic},
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, Swarm},
    NetworkBehaviour, PeerId,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;


// ------------------- DATA STRUCTURES ------------------------

// Key value pair and derived peer id for libp2p's intrinsics to identify clients on the network
pub static KEYS: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
pub static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
// Using FloodSub, a simple publish/subscribe protocol to communicate between nodes
// These topics are channels to subscribe to. We can subscribe to chains and use them to send local blockchain to other nodes
// and receive theirs. Similarly we can subscribe to blocks to send and receive new blocks
pub static CHAIN_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("chains"));
pub static BLOCK_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("blockchain"));

// ChainResponse holds a list of blocks and receiver. This is expected if someone sends their local blockchain
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainResponse {
    pub blockchain: Vec<Block>,
    pub receiver: String
}

// This is what triggers the above interaction. Sending this with peer_id of another node will make them send us their chain back
#[derive(Debug, Serialize, Deserialize)]
pub struct LocalChainRequest {
    pub from_peer_id: String
}

// This is to handle incoming messages, lazy initialization, and keyboard input
pub enum EventType {
    LocalChainResponse(ChainResponse),
    Input(String),
    Init
}

// Holds the FloodSub instance and Mdns instance
#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub response_sender: mpsc::UnboundedSender<ChainResponse>,
    #[behaviour(ignore)]
    pub init_sender: mpsc::UnboundedSender<bool>,
    #[behaviour(ignore)]
    pub app: App
}


// ---------------------------- IMPLEMENTATIONS -----------------------\
impl AppBehaviour {
    pub async fn new(
        app: App,
        response_sender: mpsc::UnboundedSender<ChainResponse>,
        init_sender: mpsc::UnboundedSender<bool>,
    ) -> Self {
        let mut behaviour = Self {
            app,
            floodsub: Floodsub::new(*PEER_ID),
            mdns: Mdns::new(Default::default()).await.expect("Can create mdns"),
            response_sender,
            init_sender
        };

        behaviour.floodsub.subscribe(CHAIN_TOPIC.clone());
        behaviour.floodsub.subscribe(BLOCK_TOPIC.clone());

        behaviour
    }
}

// Implement handlers for data incoming from other nodes

// Mdns events
// If a new node is discovered we add it to our FloodSub list of nodes so we can communicate
// Once it expires, remove it again

impl NetworkBehaviourEventProcess<MdnsEvent> for AppBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

// FloodSub events
// Incoming event handler

// For incoming events (FloodsubEvent::Message) we check whether the payload fits any of our expected data structures
// If it's a ChainResponse, we got sent a local blockchain by another node
// if it's a LocalChainRequest, check the peer id to see if they're the one we want the chain from and send them a JSON of our blockchain
// If it's a Block, someone else mined a block and wants us to add it to local. Check validity and add
impl NetworkBehaviourEventProcess<FloodsubEvent> for AppBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice::<ChainResponse>(&msg.data) {
                if resp.receiver == PEER_ID.to_string() {
                    info!("Response from {}:", msg.source);
                    resp.blockchain.iter().for_each(|r| info!("{:?}", r));

                    self.app.blockchain = self.app.choose_chain(self.app.blockchain.clone(), resp.blockchain);
                }
            } else if let Ok(resp) = serde_json::from_slice::<LocalChainRequest>(&msg.data) {
                info!("Sending local chain to {}", msg.source.to_string());
                let peer_id = resp.from_peer_id;

                if PEER_ID.to_string() == peer_id {
                    if let Err(e) = self.response_sender.send(ChainResponse {
                        blockchain: self.app.blockchain.clone(),
                        receiver: msg.source.to_string()
                    }) {
                        error!("Error sending response via channel: {}", e);
                    }
                }
            } else if let Ok(block) = serde_json::from_slice::<Block>(&msg.data) {
                info!("Received new block from {}", msg.source.to_string());
                self.app.try_add_block(block);
            }
        }
    }
}

// -------------------------- HELPER FUNCTIONS --------------------------

pub fn get_list_peers(swarm: &Swarm<AppBehaviour>) -> Vec<String> {
    info!("Discovered Peers:");
    let nodes = swarm.behaviour().mdns.discovered_nodes();
    let mut unique_peers = HashSet::new();
    for peer in nodes {
        unique_peers.insert(peer);
    }
    unique_peers.iter().map(|p| p.to_string()).collect()
}

pub fn handle_print_peers(swarm: &Swarm<AppBehaviour>) {
    let peers = get_list_peers(swarm);
    peers.iter().for_each(|p| info!("{}", p));
}

pub fn handle_print_chain(swarm: &Swarm<AppBehaviour>) {
    info!("Local Blockchain:");
    let pretty_json =
        serde_json::to_string_pretty(&swarm.behaviour().app.blockchain).expect("can jsonify blocks");
    info!("{}", pretty_json);
}

pub fn handle_create_block(cmd: &str, swarm: &mut Swarm<AppBehaviour>) {
    if let Some(data) = cmd.strip_prefix("create b") {
        let behaviour = swarm.behaviour_mut();
        let latest_block = behaviour
            .app
            .blockchain
            .last()
            .expect("there is at least one block");
        let block = Block::new(
            latest_block.block_id + 1,
            latest_block.hash.clone(),
            data.to_owned(),
        );
        let json = serde_json::to_string(&block).expect("can jsonify request");
        behaviour.app.blockchain.push(block);
        info!("broadcasting new block");
        behaviour
            .floodsub
            .publish(BLOCK_TOPIC.clone(), json.as_bytes());
    }
}