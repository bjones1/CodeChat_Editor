#!/usr/bin/env bash

echo "Install pytest"
sudo apt update
sudo apt install -y python3-pip
pip3 install pytest
echo "Build NPM packages"
cd client
npm update
npm run build
echo "Build server."
cd ../server
cargo build
