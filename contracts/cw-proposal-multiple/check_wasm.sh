#!bin/bash

RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw3_multiple_choice.wasm .
check_contract cw3_multiple_choice.wasm