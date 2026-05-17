use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTermEvent {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: u64,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub id: String,
    pub step_type: String,
    pub content: String,
    pub timestamp: u64,
    pub parent_id: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub input: serde_json::Value,
    pub output: Option<String>,
}

pub struct MemoryLayers {
    short_term: VecDeque<ShortTermEvent>,
    max_short_term: usize,
    reasoning: Vec<ReasoningStep>,
}

impl MemoryLayers {
    pub fn new(max_short_term: usize) -> Self {
        Self {
            short_term: VecDeque::with_capacity(max_short_term),
            max_short_term,
            reasoning: Vec::new(),
        }
    }

    pub fn add_short_term_event(&mut self, event: ShortTermEvent) {
        if self.short_term.len() == self.max_short_term {
            self.short_term.pop_front();
        }
        self.short_term.push_back(event);
    }

    pub fn get_short_term_context(&self, n: usize) -> Vec<&ShortTermEvent> {
        self.short_term.iter().rev().take(n).collect()
    }

    pub fn add_reasoning_step(&mut self, step: ReasoningStep) {
        self.reasoning.push(step);
    }

    pub fn get_reasoning_trace(&self, limit: usize) -> Vec<&ReasoningStep> {
        self.reasoning.iter().rev().take(limit).collect()
    }

    pub fn restore_short_term(&mut self, events: Vec<ShortTermEvent>) {
        for ev in events {
            self.add_short_term_event(ev);
        }
    }

    pub fn restore_reasoning(&mut self, steps: Vec<ReasoningStep>) {
        self.reasoning = steps;
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl ShortTermEvent {
    pub fn new(role: String, content: String, metadata: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: now_secs(),
            metadata,
        }
    }
}

impl ReasoningStep {
    pub fn new(
        step_type: String,
        content: String,
        parent_id: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            step_type,
            content,
            timestamp: now_secs(),
            parent_id,
            tool_calls,
        }
    }
}