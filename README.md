# Rust Wasm WebRTC BitTorent-like file sharing example (WIP)

## About

An example of BitTorrent-like file sharing in a browser using Rust/Wasm and WebRTC.
At the moment, the example is extremely CPU intensive, makes poor use of Peer-To-Peer interactions, and has poor block selection algorithms.

## State

- [x] Peer-To-Tracker protocol,
- [x] Tracker implementation,
- [x] Client file sharing,
- [x] Client file receiving,
- [x] Peer-To-Peer protocol
- [x] Minimal client peer-to-peer interaction,
- [x] Minimal client UI,
- [ ] Better client peer-to-peer interaction,
- [ ] Better client UI,

## Setup

* Run `bash setup.sh`

## Usage

* Run `bash watch.sh`,
* Open `localhost:8080`,
* Open the browser console to read log messages,
* If necessary, edit the server address and other parameters,
* Сlick the button `Connect to server`,
* Click the button `Choose file`,
* The default settings are convenient for debugging file transfers of about 1 MB in size.
* Now this peer will share this file
* Copy file SHA256,
* Open `localhost:8080` in another browser tab,
* Open the browser console to read log messages,
* If necessary, edit the server address and other parameters,
* Сlick the button `Connect to server`,
* Paste SHA256 and click `Receive file by sha256`,
* This peer now gets parts of the file, and also distributes the already downloaded parts to other peers,
* Wait for the file to load (progress is shown in the browser log),
* When the file is fully downloaded, a link will be added to the page to download it,
* If the seeder gets disconnected, the leechers will still continue to exchange parts of the file, and after a while all the leechers will have the same percentage of the downloaded file.

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
