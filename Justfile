build:
  cargo build --release

install-local: build
  sudo systemctl stop rpcpu.service
  sudo cp target/release/rpcpu /usr/local/bin
  sudo systemctl start rpcpu.service
