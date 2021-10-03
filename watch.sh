#!/bin/bash

trap "kill 0" EXIT

TRACKER_ADDRESS=$(hostname -I | awk '{print $1}')
TRACKER_PORT="9010"

(
    cd client
    mkdir -p target
    echo $"window.tracker_address = \"ws://$TRACKER_ADDRESS:$TRACKER_PORT\";" > target/params.js
    trunk serve --release -d dist -w . ../peer ../tracker-protocol
) &
CLIENT_PID=$!

(
    cd server
    cargo watch -s "RUST_LOG=warn,tracker=debug cargo run -- -a $TRACKER_ADDRESS -p $TRACKER_PORT"
) &
TRACKER_PID=$!

wait
exit
