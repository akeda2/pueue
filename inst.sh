#!/bin/bash
cd pueue
cargo build --release --locked || { echo "FAIL" ; exit 1;}
sudo cp target/release/pueue target/release/pueued /usr/local/bin && \
sudo pueue completions bash /usr/share/bash-completion/completions
