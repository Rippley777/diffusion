use serde::{Deserialize, Serialize};

const RECENT_LIMIT: usize = 20;
const BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scratchpad {
    pub names: [String; 2],
    pub texts: [String; 2],
}

impl Scratchpad {
    pub fn blank() -> Self {
        Self {
            names: ["Scratchpad A.txt".into(), "Scratchpad B.txt".into()],
            texts: Default::default(),
        }
    }

    fn bytes(&self) -> usize {
        self.names
            .iter()
            .chain(self.texts.iter())
            .map(String::len)
            .sum()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub pinned: bool,
    pub content: Scratchpad,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub entries: Vec<Entry>,
    next_id: u64,
}

impl History {
    pub fn record(&mut self, content: Scratchpad, editing: Option<u64>) -> Result<u64, String> {
        // Pinned entries are immutable snapshots; edits branch into a new entry.
        let replace = editing.filter(|id| self.entries.iter().any(|e| e.id == *id && !e.pinned));
        let duplicate = self
            .entries
            .iter()
            .find(|e| e.content == content)
            .map(|e| e.id);
        let id = duplicate.or(replace).unwrap_or(self.next_id);
        let pinned =
            duplicate.is_some_and(|id| self.entries.iter().any(|e| e.id == id && e.pinned));
        let pinned_bytes: usize = self
            .entries
            .iter()
            .filter(|e| e.pinned && e.id != id)
            .map(|e| e.content.bytes())
            .sum();
        if pinned_bytes.saturating_add(content.bytes()) > BYTE_LIMIT {
            return Err(
                "History is full. Unpin or delete a saved comparison to make room (64 MiB limit)."
                    .into(),
            );
        }
        self.next_id = self.next_id.max(id.saturating_add(1));
        self.entries.retain(|e| e.id != id);
        self.entries.insert(
            0,
            Entry {
                id,
                pinned,
                content,
            },
        );
        self.trim();
        Ok(id)
    }

    pub fn trim(&mut self) {
        let mut recent = 0;
        self.entries.retain(|e| {
            if e.pinned {
                return true;
            }
            recent += 1;
            recent <= RECENT_LIMIT
        });
        let mut bytes: usize = self.entries.iter().map(|e| e.content.bytes()).sum();
        while bytes > BYTE_LIMIT {
            let Some(index) = self.entries.iter().rposition(|e| !e.pinned) else {
                break;
            };
            bytes -= self.entries.remove(index).content.bytes();
        }
    }

    pub fn toggle_pin(&mut self, id: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.pinned = !entry.pinned;
        }
        self.trim();
    }

    pub fn remove(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    pub fn clear_recent(&mut self) {
        self.entries.retain(|e| e.pinned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(text: &str) -> Scratchpad {
        Scratchpad {
            texts: [text.into(), "right".into()],
            ..Scratchpad::blank()
        }
    }

    #[test]
    fn pins_survive_eviction_and_edits_branch() {
        let mut history = History::default();
        let original = history.record(content("original"), None).unwrap();
        history.toggle_pin(original);
        let branch = history.record(content("edited"), Some(original)).unwrap();
        assert_ne!(original, branch);
        for i in 0..30 {
            history.record(content(&i.to_string()), None).unwrap();
        }
        assert_eq!(history.entries.len(), 21);
        assert_eq!(
            history
                .entries
                .iter()
                .find(|e| e.id == original)
                .unwrap()
                .content,
            content("original")
        );
        history.clear_recent();
        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn deduplicates_and_updates_one_editing_session() {
        let mut history = History::default();
        let id = history.record(content("first"), None).unwrap();
        assert_eq!(history.record(content("first"), None).unwrap(), id);
        assert_eq!(history.record(content("second"), Some(id)).unwrap(), id);
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].content, content("second"));
    }

    #[test]
    fn full_pins_do_not_evict_or_destroy_existing_entries() {
        let mut history = History::default();
        let large = Scratchpad {
            names: Default::default(),
            texts: ["x".repeat(BYTE_LIMIT), String::new()],
        };
        let id = history.record(large, None).unwrap();
        history.toggle_pin(id);
        assert!(history.record(content("new"), None).is_err());
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].id, id);
    }
}
