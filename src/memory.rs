use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    pub id: String,
    pub content: String,
    pub mem_type: String,
    pub created_at: u64,
    pub metadata: HashMap<String, String>,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdge {
    pub relation: String,
    pub weight: f32,
    pub created_at: u64,
}

pub struct AgentMemory {
    graph: DiGraph<MemoryNode, MemoryEdge>,
    node_indices: HashMap<String, NodeIndex>,
    content_index: HashMap<(String, String), String>, // (content, mem_type) -> id
}

impl AgentMemory {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            content_index: HashMap::new(),
        }
    }

    // Добавляет узел, избегая дублирования по content+mem_type
    pub fn add_node(&mut self, node: MemoryNode) -> Result<(), String> {
        let key = (node.content.clone(), node.mem_type.clone());
        if self.content_index.contains_key(&key) {
            return Err(format!("Duplicate node: content '{}' type '{}' already exists", node.content, node.mem_type));
        }
        if self.node_indices.contains_key(&node.id) {
            return Err(format!("Node with id {} already exists", node.id));
        }
        let idx = self.graph.add_node(node.clone());
        self.node_indices.insert(node.id.clone(), idx);
        self.content_index.insert(key, node.id);
        Ok(())
    }

    pub fn add_edge(&mut self, from_id: &str, to_id: &str, edge: MemoryEdge) -> Result<(), String> {
        let from = *self.node_indices.get(from_id).ok_or("From node not found")?;
        let to = *self.node_indices.get(to_id).ok_or("To node not found")?;
        self.graph.add_edge(from, to, edge);
        Ok(())
    }

    pub fn search_by_keyword(&self, keyword: &str) -> Vec<&MemoryNode> {
        let keyword_lower = keyword.to_lowercase();
        self.graph
            .node_weights()
            .filter(|node| node.content.to_lowercase().contains(&keyword_lower))
            .collect()
    }

    // Возвращает последние N узлов (по created_at, от новых к старым)
    pub fn get_recent(&self, limit: usize) -> Vec<&MemoryNode> {
        let mut nodes: Vec<&MemoryNode> = self.graph.node_weights().collect();
        nodes.sort_by_key(|n| std::cmp::Reverse(n.created_at));
        nodes.truncate(limit);
        nodes
    }

    pub fn semantic_search(&self, query_embed: &[f32], top_k: usize) -> Vec<(&MemoryNode, f32)> {
        let mut scores: Vec<(&MemoryNode, f32)> = self
            .graph
            .node_weights()
            .filter_map(|node| node.embedding.as_ref().map(|emb| {
                let sim = cosine_similarity(query_embed, emb);
                (node, sim)
            }))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.truncate(top_k);
        scores
    }

    pub fn len(&self) -> usize {
        self.graph.node_count()
    }
}

impl Default for AgentMemory {
    fn default() -> Self {
        Self::new()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}