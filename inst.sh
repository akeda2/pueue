#!/bin/bash
cont() {
    local prompt="${1:-Continue?} (y/n): "
    read -p "$prompt" -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]]
}
# Look for cargo:
command -v cargo \
	|| { cont "No cargo/rust. Install?" \
		&& curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh ;\
		echo "Logout, login and run again" ;\
		exit 0;}
# Build pueue:
pushd pueue
	cargo build --release --locked && echo "Build success!" || { echo "Build failed" ; exit 1;}
popd
# Install pueue and pueued, add bash-completions:
cont "(re-)Install to /usr/bin/ and (re)add bash-completions to /usr/share/bash-completions/completions/?" \
	&& { sudo install -m 755 target/release/pueue target/release/pueued /usr/bin \
	&& sudo pueue completions bash /usr/share/bash-completion/completions \
	&& echo "Done! If you want additional shell completions, run \"pueue completions <shell> <target-path>\"";}
# Enable service, start with "systemctl start --user pueued.service":
cont "(re-)Copy service to /etc/systemd/user and enable+(re)start service?" \
	&& { sudo cp utils/pueued.service /etc/systemd/user/ \
		&& systemctl enable --user pueued.service \
		&& systemctl restart --user pueued.service \
		&& echo "Service (re)start - success!" || echo "FAIL!";} \
	|| echo "Start service with: \"systemctl start --user pueued.service\""
# Enable linger for user:
cont "(re-)loginctl enable-linger for user?" && { loginctl enable-linger && echo "loginctl enable-linger - success!" || echo "FAIL!";}