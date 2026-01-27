//! Command history for interactive mode.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const MAX_HISTORY_SIZE: usize = 1000;

pub struct History {
    entries: Vec<String>,
    index: Option<usize>,
    draft: String,
}

impl History {
    pub fn load() -> Self {
        let entries = history_path()
            .and_then(|path| File::open(path).ok())
            .map(|file| BufReader::new(file).lines().map_while(Result::ok).collect())
            .unwrap_or_default();

        Self {
            entries,
            index: None,
            draft: String::new(),
        }
    }

    pub fn save(&self) {
        let Some(path) = history_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(mut file) = File::create(&path) else {
            return;
        };
        let start = self.entries.len().saturating_sub(MAX_HISTORY_SIZE);
        for entry in &self.entries[start..] {
            let _ = writeln!(file, "{}", entry);
        }
    }

    pub fn add(&mut self, programme: &str) {
        let programme = programme.trim();
        if programme.is_empty() {
            return;
        }
        if self.entries.last().map(String::as_str) != Some(programme) {
            self.entries.push(programme.to_string());
        }
    }

    pub fn up(&mut self, current: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }

        let new_index = match self.index {
            None => {
                self.draft = current.to_string();
                self.entries.len() - 1
            }
            Some(0) => return None,
            Some(i) => i - 1,
        };

        self.index = Some(new_index);
        Some(&self.entries[new_index])
    }

    pub fn down(&mut self, _current: &str) -> Option<&str> {
        let i = self.index?;

        if i + 1 >= self.entries.len() {
            self.index = None;
            return Some(&self.draft);
        }

        self.index = Some(i + 1);
        Some(&self.entries[i + 1])
    }

    pub fn reset(&mut self) {
        self.index = None;
        self.draft.clear();
    }
}

fn history_path() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("t").join("history"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_up_down_navigation() {
        let mut history = History {
            entries: vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
            ],
            index: None,
            draft: String::new(),
        };

        assert_eq!(history.up("current"), Some("third"));
        assert_eq!(history.index, Some(2));

        assert_eq!(history.up("current"), Some("second"));
        assert_eq!(history.index, Some(1));

        assert_eq!(history.up("current"), Some("first"));
        assert_eq!(history.index, Some(0));

        assert_eq!(history.up("current"), None);
        assert_eq!(history.index, Some(0));

        assert_eq!(history.down("current"), Some("second"));
        assert_eq!(history.index, Some(1));

        assert_eq!(history.down("current"), Some("third"));
        assert_eq!(history.index, Some(2));

        assert_eq!(history.down("current"), Some("current"));
        assert_eq!(history.index, None);
    }

    #[test]
    fn test_draft_preserved() {
        let mut history = History {
            entries: vec!["old".to_string()],
            index: None,
            draft: String::new(),
        };

        assert_eq!(history.up("my draft"), Some("old"));
        assert_eq!(history.draft, "my draft");

        assert_eq!(history.down("old"), Some("my draft"));
    }

    #[test]
    fn test_add_deduplicates() {
        let mut history = History {
            entries: vec!["first".to_string()],
            index: None,
            draft: String::new(),
        };

        history.add("first");
        assert_eq!(history.entries.len(), 1);

        history.add("second");
        assert_eq!(history.entries.len(), 2);

        history.add("second");
        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn test_add_ignores_empty() {
        let mut history = History {
            entries: vec![],
            index: None,
            draft: String::new(),
        };

        history.add("");
        history.add("  ");
        assert!(history.entries.is_empty());
    }

    #[test]
    fn test_empty_history_navigation() {
        let mut history = History {
            entries: vec![],
            index: None,
            draft: String::new(),
        };

        assert_eq!(history.up("current"), None);
        assert_eq!(history.down("current"), None);
    }
}
