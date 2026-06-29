use std::collections::HashMap;

/// Bidirectional map between WebAtom JS-assigned node IDs and Blitz internal node IDs.
///
/// - Forward (`js_id → blitz_id`): used when applying DomOps from the JS thread.
/// - Reverse (`blitz_id → js_id`): used when translating blitz hit-test results to JS node IDs
///   for event dispatch.
pub struct NodeIdMap {
    js_to_blitz: HashMap<usize, usize>,
    blitz_to_js: HashMap<usize, usize>,
}

impl NodeIdMap {
    pub fn new() -> Self {
        Self {
            js_to_blitz: HashMap::new(),
            blitz_to_js: HashMap::new(),
        }
    }

    /// Insert a (js_id, blitz_id) pair, evicting any stale reverse entry for the old blitz_id.
    pub fn insert(&mut self, js_id: usize, blitz_id: usize) {
        if let Some(old_blitz) = self.js_to_blitz.insert(js_id, blitz_id) {
            self.blitz_to_js.remove(&old_blitz);
        }
        self.blitz_to_js.insert(blitz_id, js_id);
    }

    /// Remove by JS id; returns the corresponding blitz id if it existed.
    pub fn remove(&mut self, js_id: usize) -> Option<usize> {
        let blitz_id = self.js_to_blitz.remove(&js_id)?;
        self.blitz_to_js.remove(&blitz_id);
        Some(blitz_id)
    }

    /// Forward lookup: JS id → Blitz id (used when applying DomOps).
    pub fn blitz_id(&self, js_id: usize) -> Option<usize> {
        self.js_to_blitz.get(&js_id).copied()
    }

    /// Reverse lookup: Blitz id → JS id (used when forwarding hit-test results as events).
    pub fn js_id(&self, blitz_id: usize) -> Option<usize> {
        self.blitz_to_js.get(&blitz_id).copied()
    }

    pub fn clear(&mut self) {
        self.js_to_blitz.clear();
        self.blitz_to_js.clear();
    }
}
