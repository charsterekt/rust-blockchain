<a href="https://blog.logrocket.com/how-to-build-a-blockchain-in-rust/">Reference</a>

## Dependencies used
- chrono = "0.4" - Date and Time equivalent
- sha2 = "0.9.8" - SHA256 encryption
- serde = {version = "1.0", features = ["derive"] } - For JSON
- serde_json = "1.0" - For JSON
- libp2p = { version = "0.39", features = ["tcp-tokio", "mdns"] } - Peer to Peer server
- tokio = { version = "1.0", features = ["io-util", "io-std", "macros", "rt", "rt-multi-thread", "sync", "time"] } - Async Runtime for Rust
- hex = "0.4" - Cryptography
- once_cell = "1.5" - Static Initialization
- log = "0.4" - Logging
- pretty_env_logger = "0.4" - Prettyier Logging
<hr>

## Notes:

- The Blockchain is not persistent, it only exists as long as the code is running
- There is no error handling, there is a case where race conditions between nodes will result in an invalid state and broken nodes
- FloodSub protocol is simple to set up and use for P2P but is less efficient as we have to broadcast every single bit of info
- GossipSub or request/response models can be used but the setup is far more complex for the sake of the demo

<hr>

## Testing Flow:

We can start the application using `RUST_LOG=info cargo run` on Linux machines. On Windows setups, add `RUST_LOG` as an environment variable to the `user` or `system` and the value as `info`, and then restart before running the code for the changes to take place. It’s best to actually start multiple instances of it in different terminal windows.

For example, we can start two nodes:

```
INFO  rust_blockchain_example > Peer Id: 12D3KooWJWbGzpdakrDroXuCKPRBqmDW8wYc1U3WzWEydVr2qZNv
```

and

```
INFO  rust_blockchain_example > Peer Id: 12D3KooWSXGZJJEnh3tndGEVm6ACQ5pdaPKL34ktmCsUqkqSVTWX
```

Using `ls peers` in the second app shows the connection to the first one:

```
INFO  rust_blockchain_example::p2p > Discovered Peers:
INFO  rust_blockchain_example::p2p > 12D3KooWJWbGzpdakrDroXuCKPRBqmDW8wYc1U3WzWEydVr2qZNv
```

Then we can use `ls chain` to print the genesis block:

```
INFO  rust_blockchain_example::p2p > Local Blockchain:
INFO  rust_blockchain_example::p2p > [
    {
    "id": 0,
    "hash": "Genesis Hash",
    "previous_hash": "---",
    "timestamp": 1636664658,
    "data": "Genesis Block",
    "nonce": 2108
    }
]
```

Let's create a block

```
create b hello
INFO  rust_blockchain_example      > mining block...
INFO  rust_blockchain_example      > nonce: 0
INFO  rust_blockchain_example      > mined! nonce: 62235, hash: 00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922, binary hash: 0010001100111101101000110110101001111110011111000101010101000101111110101010110110010011111110111000010100001011110001111000000110110111101100010111111100001011011110001111110100011111011000101111111001111110101001100010
INFO  rust_blockchain_example::p2p > broadcasting new block
```

On the first node we see this 

```
INFO  rust_blockchain_example::p2p > received new block from 12D3KooWSXGZJJEnh3tndGEVm6ACQ5pdaPKL34ktmCsUqkqSVTWX
```

And calling `ls chain`

```
INFO  rust_blockchain_example::p2p > Local Blockchain:
INFO  rust_blockchain_example::p2p > [
    {
    "id": 0,
    "hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "previous_hash": "genesis",
    "timestamp": 1636664655,
    "data": "genesis!",
    "nonce": 2836
    },
    {
    "id": 1,
    "hash": "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922",
    "previous_hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "timestamp": 1636664772,
    "data": " hello",
    "nonce": 62235
    }
]
```

The block got added. Add a third node. It should automatically receive the updated chain because it's longer than its own

```
INFO  rust_blockchain_example > Peer Id: 12D3KooWSDyn83pJD4eEg9dvYffceAEcbUkioQvSPY7aCi7J598q

INFO  rust_blockchain_example > sending init event
INFO  rust_blockchain_example::p2p > Discovered Peers:
INFO  rust_blockchain_example      > connected nodes: 2
INFO  rust_blockchain_example::p2p > Response from 12D3KooWSXGZJJEnh3tndGEVm6ACQ5pdaPKL34ktmCsUqkqSVTWX:
INFO  rust_blockchain_example::p2p > Block { id: 0, hash: "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43", previous_hash: "genesis", timestamp: 1636664658, data: "genesis!", nonce: 2836 }
INFO  rust_blockchain_example::p2p > Block { id: 1, hash: "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922", previous_hash: "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43", timestamp: 1636664772, data: " hello", nonce: 62235 }
```

Calling `ls chain` here will show the same chain

```
INFO  rust_blockchain_example::p2p > Local Blockchain:
INFO  rust_blockchain_example::p2p > [
    {
    "id": 0,
    "hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "previous_hash": "genesis",
    "timestamp": 1636664658,
    "data": "genesis!",
    "nonce": 2836
    },
    {
    "id": 1,
    "hash": "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922",
    "previous_hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "timestamp": 1636664772,
    "data": " hello",
    "nonce": 62235
    }
]
```

Creating a block should also work

```
create block alsoworks
INFO  rust_blockchain_example      > mining block...
INFO  rust_blockchain_example      > nonce: 0
INFO  rust_blockchain_example      > mined! nonce: 34855, hash: 0000e0bddf4e603da675b92b88e86e25692eaaa8ad20db6ecab5940bdee1fdfd, binary hash: 001110000010111101110111111001110110000011110110100110111010110111001101011100010001110100011011101001011101001101110101010101010100010101101100000110110111101110110010101011010110010100101111011110111000011111110111111101
INFO  rust_blockchain_example::p2p > broadcasting new block
```

Node 1:

```
INFO  rust_blockchain_example::p2p > received new block from 12D3KooWSDyn83pJD4eEg9dvYffceAEcbUkioQvSPY7aCi7J598q

ls chain
 INFO  rust_blockchain_example::p2p > Local Blockchain:
 INFO  rust_blockchain_example::p2p > [
  {
    "id": 0,
    "hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "previous_hash": "genesis",
    "timestamp": 1636664658,
    "data": "genesis!",
    "nonce": 2836
  },
  {
    "id": 1,
    "hash": "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922",
    "previous_hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "timestamp": 1636664772,
    "data": " hello",
    "nonce": 62235
  },
  {
    "id": 2,
    "hash": "0000e0bddf4e603da675b92b88e86e25692eaaa8ad20db6ecab5940bdee1fdfd",
    "previous_hash": "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922",
    "timestamp": 1636664920,
    "data": " alsoworks",
    "nonce": 34855
  }
]
```

Node 2: 

```
INFO  rust_blockchain_example::p2p > received new block from 12D3KooWSDyn83pJD4eEg9dvYffceAEcbUkioQvSPY7aCi7J598q
ls chain
INFO  rust_blockchain_example::p2p > Local Blockchain:
INFO  rust_blockchain_example::p2p > [
    {
    "id": 0,
    "hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "previous_hash": "genesis",
    "timestamp": 1636664655,
    "data": "genesis!",
    "nonce": 2836
    },
    {
    "id": 1,
    "hash": "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922",
    "previous_hash": "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43",
    "timestamp": 1636664772,
    "data": " hello",
    "nonce": 62235
    },
    {
    "id": 2,
    "hash": "0000e0bddf4e603da675b92b88e86e25692eaaa8ad20db6ecab5940bdee1fdfd",
    "previous_hash": "00008cf68da9f978aa080b7aad93fb4285e3c0dbd85fc21bc7e83e623f9fa922",
    "timestamp": 1636664920,
    "data": " alsoworks",
    "nonce": 34855
    }
]
```