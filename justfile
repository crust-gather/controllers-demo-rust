generate:
  cargo run --bin crdgen > yaml/crd.yaml

run:
  cargo run --bin plan

compile features="":
  #!/usr/bin/env bash
  docker run --rm \
    -v cargo-cache:/root/.cargo \
    -v $PWD:/volume \
    -w /volume \
    -t clux/muslrust:stable \
    cargo build --release --features={{features}} --bin controller
  mkdir _out -p
  cp ./target/{{arch()}}-unknown-linux-musl/release/controller _out/controller

docker-build: compile
  docker build -t ttl.sh/rust-controller:5m .

install-kopium:
  cargo install kopium

generate-api: install-kopium
  curl -sSL https://raw.githubusercontent.com/crust-gather/controllers-demo-rust/refs/heads/main/yaml/crd.yaml | kopium -D Default -D PartialEq -A -d -f - > src/api.rs