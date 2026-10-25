use super::*;
use codex_app_server_protocol::FileUpdateChange;
use pretty_assertions::assert_eq;

#[test]
fn counts_successful_edits_once_across_replay() {
    let mut counts = SessionLines::default();
    let mut item = ThreadItem::FileChange {
        id: "edit-1".into(),
        status: PatchApplyStatus::Completed,
        changes: vec![
            FileUpdateChange {
                path: "new.rs".into(),
                kind: PatchChangeKind::Add,
                diff: "a\nb".into(),
            },
            FileUpdateChange {
                path: "old.rs".into(),
                kind: PatchChangeKind::Delete,
                diff: "a\nb\n".into(),
            },
            FileUpdateChange {
                path: "edit.rs".into(),
                kind: PatchChangeKind::Update { move_path: None },
                diff: "--- a/edit.rs\n+++ b/edit.rs\n@@ -1 +1,2 @@\n-old\n+++new\n+\n".into(),
            },
        ],
    };
    counts.record("turn-1", &item);
    counts.record("turn-1", &item);
    assert_eq!(counts.display(), "loc +4/-3");
    if let ThreadItem::FileChange { status, .. } = &mut item {
        *status = PatchApplyStatus::Failed;
    }
    counts.record("turn-2", &item);
    assert_eq!(counts.display(), "loc +4/-3");
    if let ThreadItem::FileChange { status, .. } = &mut item {
        *status = PatchApplyStatus::Completed;
    }
    counts.record("turn-2", &item);
    assert_eq!(counts.display(), "loc +8/-6");
    insta::assert_snapshot!(format!(
        "{} · 5h 11% used · ctx 1% · {}",
        short_model("gpt-5.6-sol medium"),
        counts.display()
    ));
}

#[test]
fn preserves_model_identity_when_abbreviating() {
    assert_eq!(short_model("gpt-5.6-sol medium"), "5.6-sol med");
    assert_eq!(short_model("gpt-6-astra xhigh"), "6-astra xhigh");
    assert_eq!(short_model("custom-model high"), "custom-model high");
}

#[test]
fn counts_survive_restart_and_do_not_leak_between_threads() {
    let home = tempfile::tempdir().unwrap();
    let item = ThreadItem::FileChange {
        id: "edit".into(),
        status: PatchApplyStatus::Completed,
        changes: vec![FileUpdateChange {
            path: "new.rs".into(),
            kind: PatchChangeKind::Add,
            diff: "first\nsecond".into(),
        }],
    };
    let mut counts = SessionLines::default();
    counts.attach(home.path(), "thread-a");
    counts.record("turn", &item);
    let mut resumed = SessionLines::default();
    resumed.attach(home.path(), "thread-a");
    resumed.record("turn", &item);
    assert_eq!(resumed.display(), "loc +2/-0");
    resumed.attach(home.path(), "thread-b");
    assert_eq!(resumed.display(), "loc +0/-0");
    resumed.attach(home.path(), "thread-a");
    assert_eq!(resumed.display(), "loc +2/-0");
}
