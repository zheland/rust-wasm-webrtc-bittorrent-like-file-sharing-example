# Rust Wasm WebRTC BitTorent-like file sharing example

## About

An example of BitTorrent-like file sharing in a browser using Rust/Wasm and WebRTC.
This example uses good algorithm for selecting chunks and peers.
The file chunk retrieval process is visualized using canvas.

## Crates

* client: file sharing example ui,
* peer: file sharing library,
* server: tracker implementation binary,
* tracker: tracker implementation library,
* tracker-protocol: peer to tracker protocol.

## State

* [x] Peer-To-Tracker protocol,
* [x] Tracker implementation,
* [x] Client file sharing,
* [x] Client file receiving,
* [x] Peer-To-Peer protocol
* [x] Minimal client peer-to-peer interaction,
* [x] Minimal client UI,
* [x] Better client peer-to-peer interaction,
* [x] Better client UI,
* [x] Visualization for file retrieval process,
* [x] Improved logging,

## Known bugs

If you use a large number of peers or using a high transfer rate then it sometimes crashes.

## Setup

* Run `bash setup.sh`

## Usage example

* Run `bash watch.sh`;
* Open 3 windows with `localhost:8080`;
* Connect to server in all windows;
* In 1st: Set upload speed limit to 50000;
* In 2nd: Set upload speed limit to 0;
* In 3rd: Set upload speed limit to 0;
* In 1st: Click "Choose File" and choose file with size ~1MB..4MB;
* In 1st: Click "Send file";
* In 1st: Copy magnet from "File: magnet" input;
* In 2nd: Paste magnet in "Receive: magnet" input;
* In 3rd: Paste magnet in "Receive: magnet" input;
* In 2nd: Click "Receive file by magnet";
* In 3rd: Click "Receive file by magnet";
* ![1](https://user-images.githubusercontent.com/4979738/166099619-ffc7a282-9122-4108-818e-f367e40e8465.png)
* Wait a bit;
* ![2](https://user-images.githubusercontent.com/4979738/166099650-f9430cae-d204-4777-9599-94a79e4715a4.png)
* Wait until half of the file has been received by both receivers;
* In 1st: Set upload speed limit to 0;
* On a canvas, you can find out that the receivers visually received different halves of the file:
  ![3](https://user-images.githubusercontent.com/4979738/166099654-79aff748-0ba6-4f33-b57c-79dced8ebd45.png)
* In 2nd: Set upload speed limit to 30000;
* In 3rd: Set upload speed limit to 50000;
* Note that files are downloaded at different speeds:
  ![4](https://user-images.githubusercontent.com/4979738/166099657-ed998b86-6598-45bc-8eed-51ba42bc51ee.png)
* Wait for the file to be downloaded by the first agent.
  The second peer gets all the pieces of the file and the file is ready to be downloaded:
  ![5](https://user-images.githubusercontent.com/4979738/166099671-030ea028-c1ea-48a3-aac8-55f5b961e057.png)
* The third peer gets all the pieces of the file and the file is ready to be downloaded:
  ![6](https://user-images.githubusercontent.com/4979738/166099676-45498ad9-86ee-4415-8a7d-406e13ee91fa.png)

* If you set the upload speed on all peers to 50000, you can see that both leech peers constantly have approximately the same parts of the file:
![7](https://user-images.githubusercontent.com/4979738/166099678-2c33a99a-9ed4-423e-babe-885d29322e54.png)
This happens because they are actively exchanging with each other during the receiving from the seeder peer.

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
