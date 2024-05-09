#!/usr/bin/env bash

echo "Build NPM packages"
cd client
npm update
npm run build
echo "Build server."
cd ../server
cargo build
