# Remote Commit P2P

A Rust-based peer-to-peer (P2P) system for remotely committing changes to a Git repository using libp2p and Gossipsub. This project enables mobile-to-desktop/server workflows, allowing remote Git commits from mobile or other clients to a trusted daemon.

## Project Status

- **Active Development:** The protocol, daemon, and mobile integration are functional and evolving. Direct address-based connection and persistent pairing are now supported.
- **Mobile/Android:** Kotlin bindings are supported via UniFFI, using a JNA workaround for Android (see below).

## How It Works

- The **daemon** listens for commit and pairing requests from clients.
- The **client** (or mobile app) sends commit or pairing requests to the daemon using a full libp2p Multiaddr (address + PeerId).
- Pairing is required for trust: the daemon operator must approve new clients.
- After the first connection, future connections are faster thanks to DHT address caching.

## Quick Start

### Prerequisites
- Rust (2021 edition or later)
- Git
- For Android: Android Studio, NDK, and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk)

### Build (Desktop)
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

## Mobile/Android Integration (JNA Workaround)

**UniFFI 0.29.3 only generates JNA-based Kotlin bindings, which are not natively supported on Android.**
For side projects and prototyping, you can use the following workaround:

### Android Setup Steps
1. **Add JNA as an AAR dependency in your `build.gradle`:**
   ```gradle
   implementation "net.java.dev.jna:jna:5.14.0@aar"
   ```
2. **Download JNA Android JARs for each ABI:**
   - [jna-5.14.0-android-aarch64.jar](https://repo1.maven.org/maven2/net/java/dev/jna/jna/5.14.0/jna-5.14.0-android-aarch64.jar)
   - [jna-5.14.0-android-armv7.jar](https://repo1.maven.org/maven2/net/java/dev/jna/jna/5.14.0/jna-5.14.0-android-armv7.jar)
   - [jna-5.14.0-android-x86-64.jar](https://repo1.maven.org/maven2/net/java/dev/jna/jna/5.14.0-android-x86-64.jar)
3. **Extract `libjnidispatch.so` from each JAR and place in:**
   ```
   app/src/main/jniLibs/arm64-v8a/libjnidispatch.so
   app/src/main/jniLibs/armeabi-v7a/libjnidispatch.so
   app/src/main/jniLibs/x86_64/libjnidispatch.so
   ```
4. **Build your Rust library for each ABI:**
   ```sh
   cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ./app/src/main/jniLibs build --release
   ```
   This will place `libmobile_core.so` in the correct folders.
5. **Configure your `build.gradle`:**
   ```gradle
   android {
     sourceSets {
       main {
         jniLibs.srcDirs += ['src/main/jniLibs']
       }
     }
   }
   ```
6. **In your Kotlin code, load the library:**
   ```kotlin
   companion object {
       init {
           System.loadLibrary("mobile_core")
       }
   }
   ```
7. **Rebuild and run your app.**

**Note:** This is a workaround for side projects. For production, wait for UniFFI JNI support or use a JNI-based FFI solution.

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
- [JNA (for Android workaround)](https://github.com/java-native-access/jna)

## Next Steps / Development Needed
- **Dynamic repo selection:** Allow the client to specify any repo path, not just a hardcoded one.
- **Better mobile UX:** Improve error handling, address entry, and feedback in the mobile app.
- **Switch to JNI UniFFI bindings when available:** Remove JNA workaround and use official UniFFI Android support.
- **Security:** Add authentication, encryption, and more robust trust management.
- **Multi-platform support:** Expand to iOS, desktop, and more.

## License
MIT 