# Rust Wasm WebRTC BitTorent-like file sharing example (WIP)

## About

An example of BitTorrent-like file sharing in a browser using Rust/Wasm and WebRTC.
At the moment, the example is extremely CPU intensive, makes poor use of Peer-To-Peer interactions, and has poor block selection algorithms.

## WIP Branch

This is a work in progress branch.
For a more stable version, use the master branch.

## Crates

- client: file sharing example ui,
- peer: file sharing library,
- server: tracker implementation binary,
- tracker: tracker implementation library,
- tracker-protocol: peer to tracker protocol.

## State

TODO

## Setup

* Run `bash setup.sh`

## Usage

* Run `bash watch.sh`,
* Open `localhost:8080`,
* Open the browser console to read log messages,
* If necessary, edit the server address and other parameters,
* Сlick the button `Connect to server`,
* To share files:
    * Select files with `Choose file` and click `Send file`,
    * Copy the magnet link in magnet input,
* To receive files:
    * Paste the magnet link in magnet input,
    * Click the button `Receive file by magnet`.
    * Once the file is fully downloaded, it can be downloaded to the computer using the `Download` button.

## License

Licensed under either of

* Apache License, Version 2.0,
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any
additional terms or conditions.
