//! Register this once with the existing Rhai engine. Each invocation resolves
//! its backend-owned immutable snapshot by context_id. No stale single-snapshot
//! service is kept forever and no script can replace another context's data.
use crate::rhai_agent_bridge::{AgentServices, PolicyView, QuotedPlan, RequirementReceipt};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

pub struct ContextRouter {
    contexts: RwLock<BTreeMap<String, Arc<dyn AgentServices>>>,
    max_contexts: usize,
}
impl ContextRouter {
    /// Capacity supplied by the host's resource policy; no env/config writes.
    pub fn new(max_contexts: usize) -> Result<Self, String> {
        if max_contexts == 0 {
            return Err("zero_context_capacity".into());
        }
        Ok(Self {
            contexts: RwLock::new(BTreeMap::new()),
            max_contexts,
        })
    }
    pub fn insert(
        &self,
        context_id: String,
        service: Arc<dyn AgentServices>,
    ) -> Result<(), String> {
        if context_id.is_empty() {
            return Err("empty_context_id".into());
        }
        let mut m = self
            .contexts
            .write()
            .map_err(|_| "context_registry_poisoned")?;
        if m.contains_key(&context_id) {
            return Err("context_replacement_forbidden_use_new_revision_id".into());
        }
        if m.len() >= self.max_contexts {
            return Err("context_capacity_reached_evict_expired_contexts".into());
        }
        m.insert(context_id, service);
        Ok(())
    }
    /// Owner invokes this after expiry/terminal persistence or reorg invalidation.
    /// SnapshotServices' own time/revision guards still reject stale cached Arcs.
    pub fn remove(&self, context_id: &str) -> Result<bool, String> {
        Ok(self
            .contexts
            .write()
            .map_err(|_| "context_registry_poisoned")?
            .remove(context_id)
            .is_some())
    }
    fn resolve(&self, c: &Value) -> Result<Arc<dyn AgentServices>, String> {
        let id = c["context_id"].as_str().ok_or("context_id_required")?;
        self.contexts
            .read()
            .map_err(|_| "context_registry_poisoned")?
            .get(id)
            .cloned()
            .ok_or_else(|| "backend_context_not_found".into())
    }
}
impl AgentServices for ContextRouter {
    fn discover(&self, c: &Value, s: &Value) -> Result<Value, String> {
        self.resolve(c)?.discover(c, s)
    }
    fn quote(&self, c: &Value, s: &Value, p: &Value) -> Result<QuotedPlan, String> {
        self.resolve(c)?.quote(c, s, p)
    }
    fn operators(&self, c: &Value, s: &Value, p: &Value) -> Result<Value, String> {
        self.resolve(c)?.operators(c, s, p)
    }
    fn policy(&self, c: &Value, s: &Value) -> Result<PolicyView, String> {
        self.resolve(c)?.policy(c, s)
    }
    fn verify_requirements(
        &self,
        c: &Value,
        s: &Value,
        p: &Value,
        n: &[String],
    ) -> Result<Vec<RequirementReceipt>, String> {
        self.resolve(c)?.verify_requirements(c, s, p, n)
    }
    fn build_payload(&self, o: &Value, s: &Value) -> Result<Value, String> {
        self.resolve(o)?.build_payload(o, s)
    }
}
