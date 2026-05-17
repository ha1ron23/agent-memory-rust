use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use reqwest::Client;
use urlencoding::encode;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = BufReader::new(std::io::stdin());
    let client = Client::new();
    let base_url = "http://127.0.0.1:8080";

    for line in stdin.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                let _ = writeln!(std::io::stdout(), "{}", error_response);
                continue;
            }
        };
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "0.1.0",
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "agent-memory", "version": "0.1.0" }
                    }
                })
            }
            "tools/list" => {
                let tools = vec![
                    json!({
                        "name": "add_memory",
                        "description": "Add a fact to long-term memory",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "content": {"type": "string"},
                                "mem_type": {"type": "string"}
                            },
                            "required": ["content", "mem_type"]
                        }
                    }),
                    json!({
                        "name": "query_memory",
                        "description": "Search long-term memory by keyword",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"keyword": {"type": "string"}},
                            "required": ["keyword"]
                        }
                    }),
                    json!({
                        "name": "semantic_search",
                        "description": "Semantic search in long-term memory",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"q": {"type": "string"}},
                            "required": ["q"]
                        }
                    }),
                    json!({
                        "name": "add_short_term",
                        "description": "Add an event to short-term memory (conversation)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "role": {"type": "string"},
                                "content": {"type": "string"}
                            },
                            "required": ["role", "content"]
                        }
                    }),
                    json!({
                        "name": "get_short_term_context",
                        "description": "Get recent short-term memory events",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"max": {"type": "integer"}}
                        }
                    }),
                    json!({
                        "name": "add_reasoning_step",
                        "description": "Add a reasoning step (thought, action, observation)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "step_type": {"type": "string"},
                                "content": {"type": "string"}
                            },
                            "required": ["step_type", "content"]
                        }
                    }),
                    json!({
                        "name": "get_reasoning_trace",
                        "description": "Get reasoning trace",
                        "inputSchema": {
                            "type": "object",
                            "properties": {"limit": {"type": "integer"}}
                        }
                    }),
                ];
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                })
            }
            "tools/call" => {
                let tool_name = request["params"]["name"].as_str().unwrap_or("");
                let arguments = request["params"]["arguments"].clone();
                let result = match tool_name {
                    "add_memory" => {
                        let content = arguments["content"].as_str().unwrap_or("");
                        let mem_type = arguments["mem_type"].as_str().unwrap_or("fact");
                        let resp = client
                            .post(format!("{}/memory", base_url))
                            .json(&json!({ "content": content, "mem_type": mem_type }))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    "query_memory" => {
                        let keyword = arguments["keyword"].as_str().unwrap_or("");
                        let resp = client
                            .get(format!("{}/query?keyword={}", base_url, encode(keyword)))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    "semantic_search" => {
                        let q = arguments["q"].as_str().unwrap_or("");
                        let resp = client
                            .get(format!("{}/semantic_search?q={}", base_url, encode(q)))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    "add_short_term" => {
                        let role = arguments["role"].as_str().unwrap_or("");
                        let content = arguments["content"].as_str().unwrap_or("");
                        let resp = client
                            .post(format!("{}/short_term", base_url))
                            .json(&json!({ "role": role, "content": content }))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    "get_short_term_context" => {
                        let max = arguments.get("max").and_then(|v| v.as_u64()).unwrap_or(10);
                        let resp = client
                            .get(format!("{}/short_term/context?max={}", base_url, max))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    "add_reasoning_step" => {
                        let step_type = arguments["step_type"].as_str().unwrap_or("");
                        let content = arguments["content"].as_str().unwrap_or("");
                        let resp = client
                            .post(format!("{}/reasoning", base_url))
                            .json(&json!({ "step_type": step_type, "content": content }))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    "get_reasoning_trace" => {
                        let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(20);
                        let resp = client
                            .get(format!("{}/reasoning/trace?max={}", base_url, limit))
                            .send()
                            .await;
                        match resp {
                            Ok(r) => r.json::<Value>().await.unwrap_or_default(),
                            Err(e) => json!({ "error": e.to_string() })
                        }
                    }
                    _ => json!({ "error": "Unknown tool" })
                };
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [ { "type": "text", "text": serde_json::to_string(&result).unwrap_or_default() } ]
                    }
                })
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                })
            }
        };
        let _ = writeln!(std::io::stdout(), "{}", response);
    }
    Ok(())
}