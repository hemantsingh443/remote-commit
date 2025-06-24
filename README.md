# Remote Commit P2P

> **Note:** This project is in its early stages (initialization/ongoing development). The current codebase provides the foundation for remote Git commits via a P2P protocol, with a focus on enabling mobile-to-desktop/server workflows.

A Rust-based peer-to-peer (P2P) system for remotely committing changes to a Git repository using libp2p and Gossipsub. This project demonstrates a simple protocol for sending commit requests from a client (including mobile) to a daemon, which performs the commit and responds with the result.

## Project Status

- **Ongoing/Initialization:** The project is under active development. The current implementation establishes the protocol, daemon, and a test client. Mobile integration is a primary goal and is being actively developed.

## Mobile Integration

This project is designed to support remote Git commits from **mobile devices**:

- **Mobile Client:**
  - The client logic can be compiled for Android or iOS using Rust's cross-compilation tools.
  - The `mobile_core` library (to be developed/extended) will provide a reusable interface for mobile apps.
- **Integration:**
  - Use the `mobile_core` library in your mobile app (Kotlin/Swift/Flutter/etc.).
  - Communicate with the desktop/server daemon over the local network or internet using the same P2P protocol.
- **Example Use Case:**
  - Edit files or trigger commits from your phone.
  - The daemon applies the commit to your repository and responds with the result.

**See the `mobile_core` crate (coming soon) for integration details.**

## Persistent Client Identity & Pairing (NEW)

- The client now generates and saves a persistent identity keypair in `client_identity.key` on first run.
- This identity is reused for all future client operations, ensuring a stable PeerId.
- The daemon only accepts commit requests from trusted PeerIds.
- To trust a new client, run the daemon in pairing mode (`cargo run -- --pair`) and run the client with `--pair` to initiate pairing. The daemon operator must approve the request interactively.
- Once paired, the client can send commit requests and will be recognized even after restarting.

## Project Structure

- `daemon/` - The P2P daemon that listens for commit requests and performs Git operations.
- `client/` - The client (or test harness) that sends commit requests to the daemon.
- `git-actor/` - Library for performing Git operations.
- `test-harness/` - (Optional) Additional test utilities.
- `src/` - Shared or root-level code.

## Protocol

All messages are sent as JSON over libp2p Gossipsub. The protocol supports two message types:

- `CommitRequest`: Sent by the client to request a commit.
- `CommitResponse`: Sent by the daemon in response, indicating success or failure.
- `PairRequest`: Sent by the client to request pairing/trust.
- `PairSuccess`: Sent by the daemon to confirm pairing.

See `daemon/src/protocol.rs` and `client/src/protocol.rs` for details.

## Getting Started

### Prerequisites
- Rust (edition 2021 or later)
- Git

### Build

From the project root:

```sh
cargo build --release
```

### Pairing a Client (NEW)

1. **Start the daemon in pairing mode:**
   ```sh
   cd daemon
   cargo run -- --pair
   ```
2. **Start the client in pairing mode (in another terminal):**
   ```sh
   cd client
   cargo run -- --pair
   ```
3. **Approve the pairing request in the daemon terminal when prompted.**

Once paired, you can run the client in commit mode as before:

```sh
cargo run
```

### Run the Daemon (Standard Mode)

In one terminal:

```sh
cd daemon
cargo run
```

### Run the Client

In another terminal:

```sh
cd client
cargo run
```

The client will send a commit request to the daemon. The daemon will perform the commit and send a response back, provided the client is trusted.

## Dependencies
- [libp2p](https://libp2p.io/)
- [anyhow](https://docs.rs/anyhow/)
- [serde](https://serde.rs/)
- [tokio](https://tokio.rs/)

## License

MIT 