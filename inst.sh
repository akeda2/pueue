#!/bin/bash
cont() {
    local prompt="${1:-Continue?} (y/n): "
    read -p "$prompt" -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]]
}

command -v cargo || { cont "No cargo/rust. Install?" && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh ; echo "logout, login and run again" ; exit 0;}
pushd pueue
	cargo build --release --locked || { echo "FAIL" ; exit 1;}
popd
sudo cp target/release/pueue target/release/pueued /usr/local/bin && \
sudo pueue completions bash /usr/share/bash-completion/completions
sudo cp utils/pueued.service /etc/systemd/user/ && systemctl enable --user pueued.service
