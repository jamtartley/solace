CLIENT=wangerz-client-term
SERVER=wangerz-server

.PHONY: build
build:
	cargo build --all

.PHONY: client
client:
	cargo run -p $(CLIENT)

.PHONY: clippy
clippy:
	cargo clippy -- -D warnings

.PHONY: doc
doc:
	cargo doc
	rustup docs --std --path

.PHONY: server
server:
	cargo run -p $(SERVER)
