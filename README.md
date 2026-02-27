# Rs Fast MCP 🦀

**High-performance, async-first Rust implementation of the Model Context Protocol (MCP)**

A Rust native port of the [Ox Fast MCP](https://github.com/neuro-ng/ox-fast-mcp) library (originally based on [FastMCP Python](https://github.com/jlowin/fastmcp)), providing a high-level, efficient interface for building MCP servers and clients using modern Rust practices.

## Features ✨

- **🚀 High Performance**: Built on `tokio` and `actix-web` for non-blocking I/O.
- **�️ Memory Safe**: Leverages Rust's ownership model for crash-free execution.
- **🔌 Flexible Transports**: Full support for Stdio (Pipe) and SSE (HTTP) transports.
- **🔒 Secure**: Built-in authentication (OAuth, OIDC, JWT) and rate limiting.
- **� Zero-Cost Abstractions**: Type-safe interfaces with minimal runtime overhead.
- **� Comprehensive Examples**: Ready-to-run examples for tools, resources, and auth.

## Architecture

```
rs-fast-mcp/
├── src/
│   ├── utilities/          # Common helpers (LruCache, JSON Schema)
│   ├── tools/              # Tool management (Execution, Validation)
│   ├── prompts/            # Prompt templates and execution
│   ├── resources/          # Resource management (URI templates, Reading)
│   ├── client/             # MCP Client SDK (Transport, Session)
│   ├── server/             # MCP Server implementation
│   │   ├── core.rs         # Core server logic & Builder
│   │   ├── middleware/     # Auth, Rate Limiting, Caching
│   │   ├── transport/      # Stdio and HTTP/SSE transports
│   │   └── auth/           # Authentication providers (OIDC, OAuth, etc.)
│   ├── mcp/                # Protocol types and message definitions
│   ├── cli/                # Command-line interface definitions
│   └── lib.rs              # Library entry point
├── examples/               # Usage examples (demos, integrations)
├── tests/                  # Integration tests
├── specs/                  # Specifications (Gemini)
├── Cargo.toml              # Project configuration
└── README.md               # This file
```

## Test Results Summary

**132 tests** across unit, integration, and doc-tests — all passing:

**✅ Server Module**: 43 tests passing
- Core server lifecycle, Stdio/SSE transports
- Authentication (Google, GitHub, OIDC, Auth0, AWS Cognito)
- Middleware (Rate limiting, Caching, Logging)
- Request handling, Mounting, Proxy, Lifespan hooks
- Context state management, Change reporting flags
- DuplicateStrategy serialization

**✅ Client Module**: 7 tests passing
- Client-server integration, Error handling, Timeouts
- Transport instantiation, Authentication injection
- Proxy and sampling support

**✅ Tools Module**: 20 tests passing
- Registration, Execution, Validation
- Duplicate handling (Error, Ignore, Replace), Fuzzy matching, Stats
- Tool builder (new, add_parameter, with_handler, schema structure)

**✅ Resources Module**: 14 tests passing
- Management, Reading, Templates
- Subscriptions, Subscribe/Unsubscribe, Stats, Filtering
- Duplicate strategies (Error, Ignore)

**✅ Prompts & Validation**: 14 tests passing
- Prompt management, Schema validation
- Input validation (required fields, types, URIs)
- Duplicate strategies (Error, Ignore, Replace)
- Prompt execution (not-found error path)

**✅ Core & Utilities**: 16 tests passing
- Settings loading, JSON Schema, Configuration
- LogLevel parsing (valid/invalid), Tag loading from env
- Error variant construction and display
- Image/Audio content block generation

**✅ MCP Protocol Types**: 15 tests passing
- ContentBlock serialization/deserialization (Text, Image, Audio)
- Role, Notification, Response round-trips
- ResourceContents (text/blob), RequestId variants
- MCPConfig CRUD, Transport inference, ServerCapabilities

**✅ Doc-tests**: 3 tests passing
- Library entry point, ServerBuilder, RateLimitMiddleware

## Core Components

### 1. Protocol Types (`src/mcp/`)

Provides strong, serde-compatible definitions for all MCP protocol messages, structures, and capabilities.

### 2. Server Module (`src/server/`)

The heart of `rs-fast-mcp`. Manages the server lifecycle, handles incoming requests via `Stdio` or `Http` transports, and orchestrates Tools, Resources, and Prompts. Includes middleware support for:
- **Authentication**: Generic `AuthProvider` trait with implementations for Google, Azure, OCI, Supabase, WorkOS, Scalekit.
- **Rate Limiting**: Token bucket algorithm.
- **Caching**: Automated result caching.

### 3. Client Module (`src/client/`)

A robust async client for connecting to MCP servers. Supports:
- **Discovery**: List tools, prompts, resources.
- **Execution**: Call tools, get prompts, read resources.
- **Transports**: Seamless switching between Stdio and SSE.

### 4. Managers (`src/tools`, `src/resources`, `src/prompts`)

Dedicated managers for registration, lookup, and execution of MCP primitives with built-in validation (JSON Schema).

## Installation 📦

Add this to your `Cargo.toml`:

```toml
[dependencies]
rs-fast-mcp = { git = "https://github.com/neuron-ng/rs-fast-mcp.git" }
```

## Building and Running

### Build the project
```bash
cargo build --release
```

### Run tests
```bash
cargo test
```

### Use the CLI
`rs-fast-mcp` includes a CLI for serving and inspecting.

```bash
# Start a server (echo example)
cargo run -- serve --transport stdio

# Connect as a client
cargo run -- client --server-url http://localhost:8000/sse

# Inspect a server
cargo run -- inspect
```

## Examples

We provide a comprehensive suite of examples to get you started:

| Example | Description | Command |
|---------|-------------|---------|
| `simple_server` | Basic Echo server | `cargo run --example simple_server` |
| `demo` | Comprehensive Stdio demo | `cargo run --example demo` |
| `filesystem` | File operations tool | `cargo run --example filesystem` |
| `calculator_server` | Math tools | `cargo run --example calculator_server` |
| `http_demo` | HTTP/SSE Transport | `cargo run --example http_demo` |
| `auth_demo` | Auth Middleware Integration | `cargo run --example auth_demo` |
| `smart_home` | WebSocket / Stateful Mock | `cargo run --example smart_home` |
| `complex_inputs` | Nested Schema Validation | `cargo run --example complex_inputs` |
| `atproto_server` | AT Protocol Client | `cargo run --example atproto_server` |
| `google_oauth_demo` | Google OAuth | `cargo run --example google_oauth_demo` |
| `github_oauth_demo` | GitHub OAuth | `cargo run --example github_oauth_demo` |
| `authkit_dcr` | Custom Routes & DCR | `cargo run --example authkit_dcr` |

For detailed specifications, see `specs/gemini/`.

## Dependencies

- **tokio**: Async runtime
- **actix-web**: HTTP server framework
- **serde**: Serialization/Deserialization
- **reqwest**: HTTP client
- **async-trait**: Async trait support
- **clap**: CLI argument parsing

## Comparison 🐍 🐫 🦀

| Feature | Ox Fast MCP (OCaml) | Rs Fast MCP (Rust) | FastMCP (Python) |
|---------|---------------------|--------------------|------------------|
| Type Safety | ✅ Strong Static | ✅ Strong Static | ❌ Runtime |
| Async Model | ✅ Lwt | ✅ Tokio | ✅ asyncio |
| Performance | ✅ Native | 🚀 Native (Highest) | ❌ Interpreted |
| Ecosystem | 🐫 OCaml | 🦀 Crates.io | 🐍 PyPI |
| Error Handling| ✅ Result Types | ✅ Result Types | ❌ Exceptions |

## Rust Implementation Benefits 🦀

- **Zero-Cost Async**: `tokio` provides industry-standard, high-performance async execution.
- **Ecosystem**: direct access to the vast crates.io ecosystem (e.g., `reqwest` for OAuth, `sqlx` for DBs).
- **Tooling**: Cargo provides best-in-class build, test, and dependency management.
- **Interop**: Easy to embed in other applications or compile to WASM (future potential).

## Contributing 🤝

We welcome contributions!

1.  **Fork** the repository.
2.  **Create** a feature branch.
3.  **Implement** your changes with tests.
4.  **Submit** a Pull Request.

Please refer to the `specs/` directory for architectural guidance.

## Version History 📈

- **v0.1.0** (Current):
    - Full OCaml port completed.
    - Stdio and HTTP transports.
    - All Auth providers implemented.
    - Comprehensive example suite.

## License 📄

Apache-2.0 License - see [LICENSE](LICENSE) file for details.

---

*Ported with ❤️ from OCaml to Rust*
