//! Compact footer formatting and replay-safe counts of successful file edits.
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::ThreadItem;
use sha2::Digest;
use sha2::Sha256;

pub(super) fn short_model(value: &str) -> String {
    value
        .strip_prefix("gpt-")
        .unwrap_or(value)
        .split_whitespace()
        .map(|part| match part {
            "medium" => "med",
            "minimal" => "min",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Default)]
pub(super) struct SessionLines {
    edits: BTreeMap<(String, String), (usize, usize)>,
    cache_dir: Option<PathBuf>,
}

impl SessionLines {
    // One atomic record per edit avoids lost updates when two clients view the same thread.
    // Only identifiers and counts are stored, never file contents.
    pub(super) fn attach(&mut self, codex_home: &Path, thread_id: &str) {
        let dir = codex_home.join("session-line-counts").join(thread_id);
        if self.cache_dir.as_ref() == Some(&dir) {
            return;
        }
        self.edits.clear();
        if let Ok(files) = std::fs::read_dir(&dir) {
            for file in files.flatten() {
                if file.path().extension().is_some_and(|ext| ext == "json")
                    && let Ok(bytes) = std::fs::read(file.path())
                    && let Ok((key, counts)) =
                        serde_json::from_slice::<((String, String), (usize, usize))>(&bytes)
                {
                    self.edits.insert(key, counts);
                }
            }
        }
        self.cache_dir = Some(dir);
    }

    pub(super) fn record(&mut self, turn_id: &str, item: &ThreadItem) {
        let ThreadItem::FileChange {
            id,
            changes,
            status: PatchApplyStatus::Completed,
            ..
        } = item
        else {
            return;
        };
        let mut added = 0;
        let mut removed = 0;
        for change in changes {
            match &change.kind {
                PatchChangeKind::Add => added += change.diff.lines().count(),
                PatchChangeKind::Delete => removed += change.diff.lines().count(),
                PatchChangeKind::Update { .. } => {
                    let mut in_hunk = false;
                    for line in change.diff.lines() {
                        if line.starts_with("@@") {
                            in_hunk = true;
                            continue;
                        }
                        if in_hunk && line.starts_with('+') {
                            added += 1;
                        }
                        if in_hunk && line.starts_with('-') {
                            removed += 1;
                        }
                    }
                }
            }
        }
        let key = (turn_id.to_string(), id.clone());
        let counts = (added, removed);
        if self.edits.get(&key) == Some(&counts) {
            return;
        }
        if let Some(dir) = &self.cache_dir {
            let save = || -> std::io::Result<()> {
                std::fs::create_dir_all(dir)?;
                let key_bytes = serde_json::to_vec(&key)?;
                let filename = format!("{:x}.json", Sha256::digest(key_bytes));
                let mut file = tempfile::NamedTempFile::new_in(dir)?;
                serde_json::to_writer(file.as_file_mut(), &(&key, counts))?;
                file.persist(dir.join(filename)).map_err(|err| err.error)?;
                Ok(())
            };
            if let Err(error) = save() {
                tracing::warn!(%error, "could not save session line counts");
            }
        }
        self.edits.insert(key, counts);
    }

    pub(super) fn display(&self) -> String {
        let (added, removed) = self
            .edits
            .values()
            .fold((0, 0), |(a, r), (plus, minus)| (a + plus, r + minus));
        format!("loc +{added}/-{removed}")
    }
}

#[cfg(test)]
#[path = "compact_status_tests.rs"]
mod tests;
