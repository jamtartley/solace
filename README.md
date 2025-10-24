# solace

A simple TCP-based chat server and terminal client written in Rust.

## Overview

Solace is a multi-user chat system consisting of a server and terminal-based client. Users can connect to the server, send messages, change nicknames, set topics, and query other users in the channel.

## Project Structure

This is a Rust workspace containing four crates:

```
solace/
├── solace-server/           # TCP chat server
├── solace-client-term/      # Terminal UI client (uses crossterm)
├── solace-protocol/         # Shared protocol definitions (requests/responses)
└── solace-message-parser/   # Message parsing and formatting
```

### Crate Descriptions

- **solace-server**: Asynchronous TCP server that handles multiple client connections, broadcasts messages, and manages chat state (topic, nicknames, user list)
- **solace-client-term**: Terminal user interface client with vim-like rendering and editing capabilities
- **solace-protocol**: Defines the binary protocol for client-server communication using bincode serialization
- **solace-message-parser**: Parses and handles message formatting with Unicode grapheme support

## Building

Build all crates:
```bash
make build
```

Build release binaries:
```bash
make build-release
```

## Running

### Start the Server

```bash
make server
```

The server listens on `0.0.0.0:7878` by default.

### Start the Client

In a separate terminal:
```bash
make client
```

## Features

### Commands

- `ping` - Test server connection
- `nick <nickname>` - Change your nickname
- `topic <text>` - Set the channel topic
- `whois <nickname>` - Query information about a user
- `disconnect` - Disconnect from the server

### Client UI

- Enter key: Send message
- Ctrl+C: Quit client

### Server Features

- Automatic nickname generation for new clients
- Real-time message broadcasting
- User join/leave notifications
- Channel topic management
- User list tracking

## Technical Details

- Built with Tokio for async I/O
- Uses framed codecs for reliable message transmission
- Terminal rendering with differential updates for efficiency
- Unicode-aware text handling
