#!/bin/bash

function cont {
        # Should we continue? Supply a string to be printed, or use default.
    local ASK="${1:-Continue?}"
    #[[ -z $1 ]] && ASK="Continue? (y/n): " || ASK="$1"
        read -p "$ASK (y/n): " -n 1 -r
        echo    # (optional) move to a new line
        if [[ $REPLY =~ ^[Yy]$ ]]; then
                return
        else
                        false
                fi
}

command -v cargo || { cont "No cargo/rust. Install?" && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh ; echo "logout, login and run again" ; exit 0;}
pushd pueue
	cargo build --release --locked || { echo "FAIL" ; exit 1;}
popd
sudo cp target/release/pueue target/release/pueued /usr/local/bin && \
sudo pueue completions bash /usr/share/bash-completion/completions
sudo cp utils/pueued.service /etc/systemd/user/ && systemctl enable --user pueued.service
