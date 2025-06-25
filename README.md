# Remote Commit P2P

A Rust-based peer-to-peer (P2P) system for remotely committing changes to a Git repository using libp2p and Gossipsub. This project enables mobile-to-desktop/server workflows, allowing remote Git commits from mobile or other clients to a trusted daemon.

## Project Status

- **Active Development:** The protocol, daemon, and mobile integration are functional and evolving. Direct address-based connection and persistent pairing are now supported.

## How It Works

- The **daemon** listens for commit requests and pairing requests from clients.
- The **client** (or mobile app) sends commit or pairing requests to the daemon using a full libp2p Multiaddr (address + PeerId).
- Pairing is required for trust: the daemon operator must approve new clients.
- After the first connection, future connections are faster thanks to DHT address caching.

## Quick Start

### Prerequisites
- Rust (2021 edition or later)
- Git

### Build
```sh
cargo build --release
```

### 1. Start the Daemon
In one terminal:
```sh
cd daemon
cargo run -- --pair
```
- The daemon will print several public addresses (Multiaddrs) like:
  ```
  /ip4/127.0.0.1/tcp/35281/p2p/12D3KooWFGPBb5BaYyCmCEZ6UmaneuVTMhe518HB1Psqtvpgy1JK
  ```
- **Copy one of these full addresses.**

### 2. Pair the Client
In another terminal:
```sh
cd client
# Edit client/src/main.rs and paste the daemon's full address into the variable `daemon_full_addr_str`.
cargo run -- --pair
```
- Approve the pairing request in the daemon terminal when prompted.

### 3. Commit from the Client
After pairing, you can run the client in commit mode:
```sh
cargo run
```
- The client will use the same full address to connect and send commit requests.

## Mobile Integration
- The `mobile_core` library provides FFI bindings for use in mobile apps (Kotlin/Swift/etc.).
- Use the same full Multiaddr for initial connection from mobile.
- See the `mobile_core` crate for details.

## Protocol
- All messages are JSON over libp2p Gossipsub.
- Types: `CommitRequest`, `CommitResponse`, `PairRequest`, `PairSuccess`.
- See `protocol.rs` for details.

## Project Structure
- `daemon/` - The P2P daemon
- `client/` - The test client
- `mobile-core/` - FFI/mobile library
- `git-actor/` - Git operations

## Dependencies
- [libp2p](https://libp2p.io/)
- [tokio](https://tokio.rs/)
- [serde](https://serde.rs/)
- [anyhow](https://docs.rs/anyhow/)

## License
MIT 