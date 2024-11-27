#!/bin/bash
pushd pueue
cargo build --release --locked || { echo "FAIL" ; exit 1;}
popd
sudo cp target/release/pueue target/release/pueued /usr/local/bin && \
sudo pueue completions bash /usr/share/bash-completion/completions
sudo cp utils/pueued.service /etc/systemd/user/
systemctl enable --user pueued.service
