use std::collections::VecDeque;
use std::time::Instant;

use ploke_core::rag_types::AssembledContext;

use crate::llm::manager::events::ContextPlan;

const DEFAULT_HISTORY_MAX: usize = 32;

#[derive(Clone, Debug)]
pub struct ContextPlanSnapshot {
    pub plan: ContextPlan,
    pub rag_context: Option<AssembledContext>,
    pub created_at: Instant,
}

impl ContextPlanSnapshot {
    pub fn new(plan: ContextPlan, rag_context: Option<AssembledContext>) -> Self {
        Self {
            plan,
            rag_context,
            created_at: Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct ContextPlanHistory {
    entries: VecDeque<ContextPlanSnapshot>,
    max_entries: usize,
}

impl Default for ContextPlanHistory {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_MAX)
    }
}

impl ContextPlanHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: max_entries.max(1),
        }
    }

    pub fn push(&mut self, snapshot: ContextPlanSnapshot) {
        self.entries.push_back(snapshot);
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, idx: usize) -> Option<&ContextPlanSnapshot> {
        self.entries.get(idx)
    }
}
