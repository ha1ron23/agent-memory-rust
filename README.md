# Agent Memory

Rust-powered memory system for AI agents. Three layers: long-term (graph + embeddings), short-term (conversation buffer), reasoning (chain of thought). HTTP API + MCP server for Claude Desktop.

## Features

- Long-term memory: facts, preferences, observations as graph nodes with semantic search (Ollama embeddings)
- Short-term memory: rolling buffer of last N user/assistant messages
- Reasoning memory: logs of thoughts, actions, tool calls, observations
- Write-Ahead Log (WAL) – data survives restart
- Duplicate prevention: same content + type cannot be added twice
- Concurrent reads (RwLock), async with tokio
- Graceful shutdown (Ctrl+C)
- No external database – everything embedded

## Quick Start

### Prerequisites
- Rust 1.70+
- Ollama with `nomic-embed-text` (for semantic search)
  ```bash
  ollama pull nomic-embed-text
  ```
  ### Build & run
  ```bash
  git clone https://github.com/ha1ron23/agent-memory-rust.git
  cd agent-memory-rust
  cargo build --release
  cargo run --release 
  ```
  HTTP server starts at http://127.0.0.1:8080

## API Reference

### Long-Term Memory (LTM)

#### `POST /memory` – Add a memory node

#### Request body:
```json
{
  "content": "string (required)",
  "mem_type": "string (required, e.g. 'fact', 'preference', 'observation')",
  "id": "string (optional, auto-generated)",
  "metadata": "object (optional)"
}
```
#### Response data: { "id": "uuid" }

#### Example:
```bash
   curl -X POST http://127.0.0.1:8080/memory \
  -H "Content-Type: application/json" \
  -d '{"content":"Cats love milk","mem_type":"fact"}'
```
GET /query – Keyword search

Query: ?keyword=...

#### Response data: { "results": [{"id":"...","content":"...","type":"..."}] }

#### Example:
```bash
curl "http://127.0.0.1:8080/query?keyword=milk"
```
### Short-Term Memory (STM)
POST /short_term – Add conversation event

#### Request body:
```json
{
  "role": "string (user/assistant/system)",
  "content": "string",
  "metadata": "object (optional)"
}
```
#### Response: {}

#### Example:
```bash
curl -X POST http://127.0.0.1:8080/short_term \
  -H "Content-Type: application/json" \
  -d '{"role":"user","content":"Hello!"}'
  ```
GET /short_term/context – Recent messages

Query parameter: ?max=N (default 10)

#### Response:
```json
{
  "context": [
    {
      "id": "uuid",
      "role": "string",
      "content": "string",
      "timestamp": 1234567890,
      "metadata": {}
    }
  ]
}
```
#### Example:
```bash
curl "http://127.0.0.1:8080/short_term/context?max=20"
```

### Reasoning Memory (RM)

POST /reasoning – Add reasoning step

#### Request body:
```json
{
  "step_type": "string (thought/action/observation/final)",
  "content": "string",
  "parent_id": "uuid (optional)",
  "tool_calls": [
    {
      "tool": "string",
      "input": {},
      "output": "string (optional)"
    }
  ]
}
```
#### Response: {}

#### Example:
```bash
curl -X POST http://127.0.0.1:8080/reasoning \
  -H "Content-Type: application/json" \
  -d '{"step_type":"thought","content":"User asked about weather"}'
```
GET /reasoning/trace – Get reasoning chain

Query parameter: ?limit=N (default 20)

#### Response:
```json
{
  "trace": [
    {
      "id": "uuid",
      "type": "string",
      "content": "string",
      "timestamp": 1234567890,
      "parent_id": "uuid or null",
      "tool_calls": [...]
    }
  ]
}
```

#### Example:
```bash
curl "http://127.0.0.1:8080/reasoning/trace?limit=10"
```

### MCP Server (Claude Desktop Integration)

Run the MCP server in a separate terminal:

```bash
cargo run --bin mcp_server --release
```

#### Configure Claude Desktop

#### Edit claude_desktop_config.json:

Linux: ~/.config/Claude/claude_desktop_config.json
macOS: ~/Library/Application Support/Claude/claude_desktop_config.json
Windows: %APPDATA%\Claude\claude_desktop_config.json

#### Add:
```json
{
  "mcpServers": {
    "agent-memory": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/ABSOLUTE/PATH/TO/agent-memory/Cargo.toml", "--bin", "mcp_server"]
    }
  }
}
```

**Restart Claude Desktop. The agent will have access to tools: add_memory, query_memory, semantic_search, add_short_term, get_short_term_context, add_reasoning_step, get_reasoning_trace.**

### Manual MCP test
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run --bin mcp_server
```

### License
**MIT License**
