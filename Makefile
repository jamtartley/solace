CLIENT=solace-client-term
SERVER=solace-server

.PHONY: build
build:
	cargo build --all

.PHONY: build-release
build-release:
	cargo build --all --release

.PHONY: client
client:
	cargo run -p $(CLIENT)

.PHONY: client-release
client-release:
	./target/release/$(CLIENT)

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

.PHONY: server-release
server-release:
	./target/release/$(SERVER)
