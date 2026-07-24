#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODE="user"
PREFIX="/usr/local"
ASSUME_YES=0
INSTALL_COMPLETIONS="ask"
SETUP_SERVICE="ask"
ENABLE_LINGER="ask"
INSTALL_GFFF_CONFIG="ask"
FORCE_GFFF_CONFIG=0

cont() {
	local prompt="${1:-Continue?} (y/n): "
	read -rp "$prompt" -n 1 -r
	echo
	[[ $REPLY =~ ^[Yy]$ ]]
}

install_cargo() {
	if [[ $ASSUME_YES -eq 1 || ! -t 0 ]]; then
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
	else
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
	fi
}

print_usage() {
	cat <<'EOF'
Usage: ./inst.sh [options]

Options:
  --mode user|system      Install scope. Default: user
  --prefix PATH           Install prefix for system mode. Default: /usr/local
  --install-completions   Install bash completions
  --no-completions        Skip completion installation
  --setup-service         Install + enable user systemd service
  --no-service            Skip service setup
  --install-gfff-config   Copy pueue.yaml to ~/.config/gfff if directory exists
  --no-gfff-config        Skip gfff-buildbot config installation
  --force-gfff-config     Overwrite existing ~/.config/gfff/pueue.yaml
  --enable-linger         Enable systemd linger for current user
  --no-linger             Skip linger setup
  --yes, -y               Non-interactive mode
  --install-cargo         Install rustup/cargo and exit
  --help, -h              Show this help
EOF
}

while (($#)); do
	case "$1" in
		--mode)
			MODE="${2:-}"
			shift 2
			;;
		--prefix)
			PREFIX="${2:-}"
			shift 2
			;;
		--install-completions)
			INSTALL_COMPLETIONS="yes"
			shift
			;;
		--no-completions)
			INSTALL_COMPLETIONS="no"
			shift
			;;
		--setup-service)
			SETUP_SERVICE="yes"
			shift
			;;
		--no-service)
			SETUP_SERVICE="no"
			shift
			;;
		--install-gfff-config)
			INSTALL_GFFF_CONFIG="yes"
			shift
			;;
		--no-gfff-config)
			INSTALL_GFFF_CONFIG="no"
			shift
			;;
		--force-gfff-config)
			FORCE_GFFF_CONFIG=1
			shift
			;;
		--enable-linger)
			ENABLE_LINGER="yes"
			shift
			;;
		--no-linger)
			ENABLE_LINGER="no"
			shift
			;;
		--yes|-y)
			ASSUME_YES=1
			shift
			;;
		--install-cargo)
			install_cargo
			exit 0
			;;
		--help|-h)
			print_usage
			exit 0
			;;
		*)
			echo "Unknown option: $1"
			print_usage
			exit 1
			;;
	esac
done

if [[ "$MODE" != "user" && "$MODE" != "system" ]]; then
	echo "Invalid mode: $MODE (expected: user|system)"
	exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
	if [[ $ASSUME_YES -eq 1 ]]; then
		echo "cargo not found. Cannot continue in --yes mode. Run with --install-cargo first."
		exit 1
	fi

	if cont "No cargo/rust found. Install rustup/cargo?"; then
		install_cargo
		echo "Please log out/in and rerun this script."
		exit 0
	fi

	exit 1
fi

pushd "$ROOT_DIR/pueue" >/dev/null
cargo build --release --locked
popd >/dev/null

SOURCE_BIN_DIR="$ROOT_DIR/target/release"
SOURCE_PUEUE="$SOURCE_BIN_DIR/pueue"
SOURCE_PUEUED="$SOURCE_BIN_DIR/pueued"
SOURCE_HELPER="$ROOT_DIR/pueue-status-compact.sh"

if [[ ! -x "$SOURCE_PUEUE" || ! -x "$SOURCE_PUEUED" ]]; then
	echo "Build artifacts not found in $SOURCE_BIN_DIR"
	exit 1
fi

if [[ "$MODE" == "user" ]]; then
	BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
	COMPLETION_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions"
	UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
	UNIT_TARGET="$UNIT_DIR/pueued.service"
	SUDO_CMD=""
else
	BIN_DIR="$PREFIX/bin"
	COMPLETION_DIR="/usr/share/bash-completion/completions"
	UNIT_DIR="/etc/systemd/user"
	UNIT_TARGET="$UNIT_DIR/pueued.service"
	if [[ $EUID -eq 0 ]]; then
		SUDO_CMD=""
	else
		SUDO_CMD="sudo"
	fi
fi

if [[ $ASSUME_YES -eq 1 ]]; then
	DO_INSTALL=1
else
	if [[ "$MODE" == "user" ]]; then
		cont "Install binaries into $BIN_DIR?" && DO_INSTALL=1 || DO_INSTALL=0
	else
		cont "Install binaries into $BIN_DIR (requires privileges)?" && DO_INSTALL=1 || DO_INSTALL=0
	fi
fi

if [[ $DO_INSTALL -eq 1 ]]; then
	if [[ "$MODE" == "user" ]]; then
		mkdir -p "$BIN_DIR"
	else
		$SUDO_CMD mkdir -p "$BIN_DIR"
	fi

	$SUDO_CMD install -m 755 "$SOURCE_PUEUE" "$SOURCE_PUEUED" "$BIN_DIR"
	$SUDO_CMD install -m 755 "$SOURCE_HELPER" "$BIN_DIR"
fi

if [[ "$INSTALL_COMPLETIONS" == "ask" ]]; then
	if [[ $ASSUME_YES -eq 1 ]]; then
		INSTALL_COMPLETIONS="yes"
	elif cont "Install bash completions into $COMPLETION_DIR?"; then
		INSTALL_COMPLETIONS="yes"
	else
		INSTALL_COMPLETIONS="no"
	fi
fi

if [[ "$INSTALL_COMPLETIONS" == "yes" ]]; then
	if [[ "$MODE" == "user" ]]; then
		mkdir -p "$COMPLETION_DIR"
	else
		$SUDO_CMD mkdir -p "$COMPLETION_DIR"
	fi
	if [[ "$MODE" == "user" ]]; then
		"$SOURCE_PUEUE" completions bash "$COMPLETION_DIR"
	else
		$SUDO_CMD "$SOURCE_PUEUE" completions bash "$COMPLETION_DIR"
	fi
fi

if [[ "$SETUP_SERVICE" == "ask" ]]; then
	if [[ $ASSUME_YES -eq 1 ]]; then
		SETUP_SERVICE="yes"
	elif cont "Install and enable user service (pueued.service) in $UNIT_DIR?"; then
		SETUP_SERVICE="yes"
	else
		SETUP_SERVICE="no"
	fi
fi

if [[ "$SETUP_SERVICE" == "yes" ]]; then
	if ! command -v systemctl >/dev/null 2>&1; then
		echo "systemctl not found. Skipping service setup."
		SETUP_SERVICE="no"
	fi
fi

if [[ "$SETUP_SERVICE" == "yes" ]]; then
	if [[ "$MODE" == "user" ]]; then
		mkdir -p "$UNIT_DIR"
	else
		$SUDO_CMD mkdir -p "$UNIT_DIR"
	fi

	temp_unit="$(mktemp)"
	cp "$ROOT_DIR/utils/pueued.service" "$temp_unit"
	sed -i "s|^ExecStart=.*|ExecStart=$BIN_DIR/pueued -vv|" "$temp_unit"
	$SUDO_CMD install -m 644 "$temp_unit" "$UNIT_TARGET"
	rm -f "$temp_unit"

	systemctl --user daemon-reload
	systemctl --user enable --now pueued.service
fi

if [[ "$ENABLE_LINGER" == "ask" ]]; then
	if [[ $ASSUME_YES -eq 1 ]]; then
		ENABLE_LINGER="no"
	elif cont "Enable linger for $USER (requires privileges)?"; then
		ENABLE_LINGER="yes"
	else
		ENABLE_LINGER="no"
	fi
fi

if [[ "$ENABLE_LINGER" == "yes" ]]; then
	if [[ $EUID -eq 0 ]]; then
		loginctl enable-linger "$USER"
	else
		sudo loginctl enable-linger "$USER"
	fi
fi

if [[ "$INSTALL_GFFF_CONFIG" == "ask" ]]; then
	if [[ "$MODE" == "user" ]]; then
		INSTALL_GFFF_CONFIG="yes"
	elif [[ $ASSUME_YES -eq 1 ]]; then
		INSTALL_GFFF_CONFIG="no"
	elif cont "Install gfff-buildbot config to ~/.config/gfff if present?"; then
		INSTALL_GFFF_CONFIG="yes"
	else
		INSTALL_GFFF_CONFIG="no"
	fi
fi

GFFF_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/gfff"
GFFF_TARGET="$GFFF_DIR/pueue.yaml"

if [[ "$INSTALL_GFFF_CONFIG" == "yes" ]]; then
	if [[ -d "$GFFF_DIR" ]]; then
		if [[ -e "$GFFF_TARGET" ]]; then
			if [[ $FORCE_GFFF_CONFIG -eq 1 ]]; then
				install -m 644 "$ROOT_DIR/pueue.yaml" "$GFFF_TARGET"
				echo "Overwrote gfff-buildbot config at $GFFF_TARGET"
			else
				echo "Skipping gfff-buildbot config install: $GFFF_TARGET already exists"
			fi
		else
			install -m 644 "$ROOT_DIR/pueue.yaml" "$GFFF_TARGET"
			echo "Installed gfff-buildbot config to $GFFF_TARGET"
		fi
	else
		echo "Skipping gfff-buildbot config install: $GFFF_DIR not found"
	fi
fi

echo
echo "Install summary"
echo "  Mode:               $MODE"
echo "  Binaries:           $BIN_DIR"
echo "  Completions:        $INSTALL_COMPLETIONS ($COMPLETION_DIR)"
echo "  User service setup: $SETUP_SERVICE ($UNIT_TARGET)"
echo "  Linger:             $ENABLE_LINGER"
echo "  gfff config:        $INSTALL_GFFF_CONFIG ($GFFF_TARGET)"
echo "  gfff force:         $FORCE_GFFF_CONFIG"
echo
echo "If $BIN_DIR is not in your PATH, add it and restart your shell."