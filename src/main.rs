// Imports
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use log::{error, warn, info};
use sha2::{Digest, Sha256};
use std::time::Duration;
use libp2p::{
    core::upgrade,
    futures::StreamExt,
    mplex,
    noise::{Keypair, NoiseConfig, X25519Spec},
    swarm::{Swarm, SwarmBuilder},
    tcp::TokioTcpConfig,
    Transport,
};
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    select, spawn,
    sync::mpsc,
    time::sleep,
};


// ----------------------------- STRUCTS ----------------------------------
pub struct App {  // Non persistent blockchain
    pub blockchain: Vec<Block>,  // The blockchain will be a vector of Blocks
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {  // Attributes in each Block
    pub block_id: u64,
    pub hash: String,
    pub prev_hash: String,
    pub timestamp: i64,
    pub data: String,
    pub nonce: u64, 
}

// ----------------------------- HELPERS ----------------------------------

// When mining a block the person has to hash the data with SHA256 and find a hash in binary which starts with "00"
// This denotes our difficulty level
// Increasing the number of zeroes increases the difficulty
const DIFFICULTY_PREFIX: &str = "00";
mod p2p;

fn hash_to_binary_representation(hash: &[u8]) -> String {
    let mut result: String = String::default();
    for c in hash {
        result.push_str(&format!("{:b}", c));
    }
    result
}

// The mining function will return a nonce and a hash
// We can create a new block with the timestamp, given data, block id, previous hash and new hash and nonce
// After announcing that we're about to mine a block we set the nonce to 0
// Then start an endless loop that increments the nonce each step, and calculate the hash
fn mine_block(block_id: u64, timestamp: i64, prev_hash: &str, data: &str) -> (u64, String) {
    info!("Mining block...");
    let mut nonce = 0;

    loop {
        if nonce % 100000 == 0 {
            info!("Trying nonce {}", nonce);
        }

        let hash = calculate_hash(block_id, timestamp, prev_hash, data, nonce);
        let bin_hash = hash_to_binary_representation(&hash);
        if bin_hash.starts_with(DIFFICULTY_PREFIX) {
            info!("Block mined with nonce {}, hash: {}, binary hash: {}", nonce, hex::encode(&hash), bin_hash);
            return (nonce, hex::encode(hash));
        }
        nonce += 1;  // Increment the nonce if it meets the condition
    }
}

// Create a JSON representation of the block and pass it through the SHA256 hasher
fn calculate_hash(block_id: u64, timestamp: i64, prev_hash: &str, data: &str, nonce: u64) -> Vec<u8> {
    let data = serde_json::json!({
        "block_id": block_id,
        "prev_hash": prev_hash,
        "data": data,
        "timestamp": timestamp,
        "nonce": nonce
    });

    let mut hash_func = Sha256::new();
    hash_func.update(data.to_string().as_bytes());
    hash_func.finalize().as_slice().to_owned()
}


// ----------------------------- IMPLEMENTATIONS ----------------------------------

impl App {
    fn new() -> Self {  // Initialization
        Self { blockchain: vec![] }
    }

    fn genesis(&mut self) {  // Genesis block logic
        let genesis_block = Block {
            block_id: 0,
            hash: String::from("Genesis Hash"),
            prev_hash: String::from("---"),
            timestamp: Utc::now().timestamp(),
            data: String::from("Genesis Block"),
            nonce: 2108,
        };

        self.blockchain.push(genesis_block); // Add genesis block to the blockchain
        // Initialize the application with an empty chain and use longest chain rule later
    }

    // Function to add new blocks
    fn try_add_block(&mut self, block: Block) {
        let latest_block = self.blockchain.last().expect("There is at least one block in the chain");

        if self.is_block_valid(&block, latest_block) {
            self.blockchain.push(block);
        } else {
            error!("Block is not valid, not added to chain");
        }
    }

    // Function to check if a block is valid by checking all the validity cases
    // 1. The previous hash needs to match the last block in the chain's hash
    // 2. The hash needs to start with "00" -> DIFFICULTY_PREFIX to indicate it was mined correctly
    // 3. The block_id needs to be the latest ID incremented by 1
    // 4. The hash itself needs to be correct, hashing the data of the block should give the block hash
    fn is_block_valid(&self, block: &Block, prev_block: &Block) -> bool {
        if block.prev_hash != prev_block.hash {
            warn!("Block with id {} has the wrong previous hash reference", block.block_id);
            return false;
        } else if !hash_to_binary_representation(
            &hex::decode(&block.hash).expect("Can't decode from Hex")
        ).starts_with(DIFFICULTY_PREFIX) {
            warn!("Block with id {} has invalid difficulty", block.block_id);
            return false;
        } else if block.block_id != prev_block.block_id + 1 {
            warn!("Block with id {} is not the next block. The latest is {}", block.block_id, prev_block.block_id);
            return false;
        } else if hex::encode(calculate_hash(
            block.block_id,
            block.timestamp,
            &block.prev_hash,
            &block.data,
            block.nonce
        )) != block.hash {
            warn!("Block with id {} has invalid hash", block.block_id);
            return false;
        }
        true
    }

    // Function to validate chain using longest chain rule
    // Ignoring the genesis, we validate every block in the chain
    // If one block fails, the chain is invalid
    fn is_chain_valid(&self, chain: &[Block]) -> bool {
        for i in 0..chain.len() {
            if i == 0 {
                continue;
            }

            let first = chain.get(i - 1).expect("It has to exist");
            let second = chain.get(i).expect("It has to exist");
            if !self.is_block_valid(second, first) {
                return false;
            }
        }
        true
    }

    // Actually choose the longest chain
    fn choose_chain(&mut self, local: Vec<Block>, remote: Vec<Block>) -> Vec<Block> {
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);

        if is_local_valid && is_remote_valid {
            if local.len() >= remote.len() {
                local
            } else {
                remote
            }
        } else if is_remote_valid && !is_local_valid {
            remote
        } else if is_local_valid && !is_remote_valid {
            local
        } else {
            panic!("Local and remote chains are both invalid");
        }
    }

}

// The mining scheme will be implemented in Block
impl Block {
    pub fn new(block_id: u64, prev_hash: String, data: String) -> Self {
        let now = Utc::now();
        let (nonce, hash) = mine_block(block_id, now.timestamp(), &prev_hash, &data);
        Self {
            block_id,
            hash,
            timestamp: now.timestamp(),
            prev_hash,
            data,
            nonce
        }
    }
}

// ------------------------- ASYNC RUNTIME -------------------------------

/*
First initialize the key pair, libp2p transport, behaviour, and libp2p Swarm
Swarm is the entity that runs the network stack
Initialize a buffered reader on stdin so we can read incoming commands from the user and start the Swarm
Spawn an async coroutine which waits a second and then sends an initialization trigger on the init channel
This is the signal we'll use after starting a node to wait for a bit untilt he node is up and connected
We then ask another node for their current blockchain to get us up to speed
*/

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    info!("Peer Id: {}", p2p::PEER_ID.clone());
    let (response_sender, mut response_receiver) = mpsc::unbounded_channel();
    let (init_sender, mut init_receiver) = mpsc::unbounded_channel();

    let auth_keys = Keypair::<X25519Spec>::new()
    .into_authentic(&p2p::KEYS).expect("Can create auth keys"); // Generate a new keypair

    let transp = TokioTcpConfig::new()
    .upgrade(upgrade::Version::V1)
    .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
    .multiplex(mplex::MplexConfig::new())
    .boxed();

    let behaviour = p2p::AppBehaviour::new(App::new(), response_sender, init_sender.clone()).await;

    let mut swarm = SwarmBuilder::new(transp, behaviour, *p2p::PEER_ID)
    .executor(Box::new(|fut| {
        spawn(fut);
    })).build();

    let mut stdin = BufReader::new(stdin()).lines();

    Swarm::listen_on(
        &mut swarm,"/ip4/0.0.0.0/tcp/0".parse().expect("can get a local socket"),
    ).expect("Swarm can be started");

    spawn(async move {
        sleep(Duration::from_secs(1)).await;
        info!("Sending init event");
        init_sender.send(true).expect("Can send init event");
    });

    /*
    Here on in, we handle keyboard events from the user, incoming, and outgoing data
    We start an endless loop and use Tokio's select! macro to race multiple async functions
    This means whichever one of these finishes first will get handled first and then we start anew
    The first event emitter is the buffered reader which will give input lines from the user
    If we get one we create an EventType::Input with the line
    Then we listen to the response and init channel, creating their events respectively
    If the events come in on the swarm itself this means they are events that are neither handled
    by our Mdns behaviour nor FloodSub behaviour and we just log them. Mostly noise but helpful debugging tools
    With corresponding events created (or not), we go about handling them
    For our init event we call genesis() on our app to create the genesis block
    If connected to nodes, trigger a LocalChainRequest to the last one in the list
    For simplicity we just ask one node and accept whatever they send us
    If we get a LocalChainResponse Event then something was sent on the response channel
    Broadcast the incoming JSON on the network to the correct FloodSub topic
    For user input we have 3 commands:
    ls peers: lists peers
    ls chain: prints local blockchain
    create block $data: create a new block with $data as the string content
    */

    loop {
        let evt = {
            select! {
                line = stdin.next_line() => Some(p2p::EventType::Input(line.expect("can get line").expect("can read line from stdin"))),
                response = response_receiver.recv() => {
                    Some(p2p::EventType::LocalChainResponse(response.expect("response exists")))
                },
                _init = init_receiver.recv() => {
                    Some(p2p::EventType::Init)
                }
                event = swarm.select_next_some() => {
                    info!("Unhandled Swarm Event: {:?}", event);
                    None
                },
            }
        };

        if let Some(event) = evt {
            match event {
                p2p::EventType::Init => {
                    let peers = p2p::get_list_peers(&swarm);
                    swarm.behaviour_mut().app.genesis();

                    info!("connected nodes: {}", peers.len());
                    if !peers.is_empty() {
                        let req = p2p::LocalChainRequest {
                            from_peer_id: peers
                            .iter()
                            .last()
                            .expect("at least one peer")
                            .to_string(),
                        };

                        let json = serde_json::to_string(&req).expect("can jsonify request");
                        swarm.behaviour_mut().floodsub.publish(p2p::CHAIN_TOPIC.clone(), json.as_bytes());
                    }
                }

                p2p::EventType::LocalChainResponse(resp) => {
                    let json = serde_json::to_string(&resp).expect("can jsonify response");
                    swarm.behaviour_mut().floodsub.publish(p2p::CHAIN_TOPIC.clone(), json.as_bytes());
                }

                p2p::EventType::Input(line) => match line.as_str() {
                    "ls peers" => p2p::handle_print_peers(&swarm),
                    cmd if cmd.starts_with("ls chain") => p2p::handle_print_chain(&swarm),
                    cmd if cmd.starts_with("create block") => p2p::handle_create_block(cmd, &mut swarm),
                    _ => error!("Unknown command"),
                },
            }
        }
    }

}
