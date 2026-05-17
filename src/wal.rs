use crate::memory::{AgentMemory, MemoryNode, MemoryEdge};
use crate::layers::{ShortTermEvent, ReasoningStep};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use tokio::task::spawn_blocking;
use tracing::info;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum WalCommand {
    AddNode { node: MemoryNode },
    AddEdge { from_id: String, to_id: String, edge: MemoryEdge },
    AddShortTermEvent { event: ShortTermEvent },
    AddReasoningStep { step: ReasoningStep },
}

const WAL_PATH: &str = "memory.wal";

pub async fn append_command(cmd: WalCommand) -> std::io::Result<()> {
    spawn_blocking(move || {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(WAL_PATH)?;
        let json = serde_json::to_string(&cmd)?;
        writeln!(file, "{}", json)?;
        file.sync_all()?;
        Ok(())
    })
    .await
    .unwrap()
}

pub fn recover_graph(memory: &mut AgentMemory) -> std::io::Result<()> {
    let file = match File::open(WAL_PATH) {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    let reader = BufReader::new(file);
    let mut count = 0;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let cmd: WalCommand = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("WAL parse error: {} (line: {})", e, line);
                continue;
            }
        };
        if let WalCommand::AddNode { node } = cmd {
            let _ = memory.add_node(node);
            count += 1;
        } else if let WalCommand::AddEdge { from_id, to_id, edge } = cmd {
            let _ = memory.add_edge(&from_id, &to_id, edge);
            count += 1;
        }
    }
    if count > 0 {
        info!("WAL recovered: {} graph operations", count);
    }
    Ok(())
}

pub fn recover_layers() -> std::io::Result<(Vec<ShortTermEvent>, Vec<ReasoningStep>)> {
    let file = match File::open(WAL_PATH) {
        Ok(f) => f,
        Err(_) => return Ok((vec![], vec![])),
    };
    let reader = BufReader::new(file);
    let mut stm_events = Vec::new();
    let mut reasoning_steps = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let cmd: WalCommand = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("WAL parse error: {} (line: {})", e, line);
                continue;
            }
        };
        match cmd {
            WalCommand::AddShortTermEvent { event } => stm_events.push(event),
            WalCommand::AddReasoningStep { step } => reasoning_steps.push(step),
            _ => {}
        }
    }
    info!("WAL recovered: {} STM events, {} reasoning steps", stm_events.len(), reasoning_steps.len());
    Ok((stm_events, reasoning_steps))
}
