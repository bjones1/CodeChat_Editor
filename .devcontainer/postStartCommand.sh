#!/usr/bin/env bash

cd server
# TODO: this doesn't open a file from the codespace, unfortunately.
cargo run -- start ../README.md
