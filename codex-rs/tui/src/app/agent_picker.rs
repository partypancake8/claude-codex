//! Root-scoped background refresh for the agent picker.

use super::agent_navigation::AgentPickerThreadVisibility;
use super::app_server_event_targets::ServerNotificationThreadTarget;
use super::app_server_event_targets::server_notification_thread_target;
use super::*;
use crate::app_event::AgentPickerThreadRefresh;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use ratatui::style::Stylize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;
use std::time::Instant;

pub(super) const AGENT_PICKER_VIEW_ID: &str = "agent-picker";
const AGENT_PICKER_PAGE_SIZE: u32 = 100;
const AGENT_PICKER_MAX_THREADS: usize = 1_000;
/// How long a Finished subagent row stays in the bottom panel before it dismisses itself.
pub(super) const FINISHED_AGENT_ROW_TTL: Duration = Duration::from_secs(30);

/// Finished-row lifetime for the bottom subagent panel. Rows record when they were first seen
/// Finished; once dismissed (expiry, next main-agent submit, or `/tasks clear`) a thread stays
/// hidden until it becomes active again, so re-emitted subscription updates cannot revive it.
#[derive(Debug, Default)]
pub(super) struct FinishedAgentRows {
    pub(super) finished_at: HashMap<ThreadId, Instant>,
    pub(super) dismissed: HashSet<ThreadId>,
}

impl FinishedAgentRows {
    /// Returns whether the row should be hidden, plus whether it just entered Finished.
    fn observe(
        &mut self,
        thread_id: ThreadId,
        status: super::agent_status_feed::AgentTaskStatus,
        is_active: bool,
        now: Instant,
        dismiss_finished: bool,
    ) -> (bool, bool) {
        if is_active {
            self.finished_at.remove(&thread_id);
            self.dismissed.remove(&thread_id);
            return (false, false);
        }
        if status != super::agent_status_feed::AgentTaskStatus::Finished {
            self.finished_at.remove(&thread_id);
            return (self.dismissed.contains(&thread_id), false);
        }
        let mut newly_finished = false;
        let finished_at = *self.finished_at.entry(thread_id).or_insert_with(|| {
            newly_finished = true;
            now
        });
        if dismiss_finished || now.saturating_duration_since(finished_at) >= FINISHED_AGENT_ROW_TTL
        {
            self.dismissed.insert(thread_id);
        }
        let hidden = self.dismissed.contains(&thread_id);
        (hidden, newly_finished && !hidden)
    }
}

impl App {
    pub(super) fn sync_task_summary(&mut self) {
        self.sync_task_summary_at(Instant::now(), /*dismiss_finished*/ false);
    }

    /// Hides every Finished row now (`/tasks clear`, or the next message to the main agent).
    pub(super) fn dismiss_finished_agent_rows(&mut self) {
        self.sync_task_summary_at(Instant::now(), /*dismiss_finished*/ true);
    }

    /// Submitting to the main agent acknowledges finished children; other threads do not.
    pub(super) fn note_main_agent_user_turn(&mut self) {
        if self.active_thread_id.is_none() || self.active_thread_id == self.primary_thread_id {
            self.dismiss_finished_agent_rows();
        }
    }

    pub(super) fn sync_task_summary_at(&mut self, now: Instant, dismiss_finished: bool) {
        let mut schedule_expiry = false;
        let mut total = 0;
        let mut working = 0;
        let mut panel_rows = Vec::new();
        for (thread_id, entry) in self.agent_navigation.visible_threads() {
            if self.primary_thread_id == Some(thread_id)
                || self.side_threads.contains_key(&thread_id)
            {
                continue;
            }
            total += 1;
            let store = self
                .thread_event_channels
                .get(&thread_id)
                .and_then(|channel| channel.store.try_lock().ok());
            let status =
                super::agent_status_feed::AgentTaskStatus::from_entry(entry, store.as_deref());
            if entry.is_running && status == super::agent_status_feed::AgentTaskStatus::Running {
                working += 1;
            }
            let name = entry
                .agent_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(|path| path.strip_prefix("/root/").unwrap_or(path).to_owned())
                .unwrap_or_else(|| {
                    format_agent_picker_item_name(
                        entry.agent_nickname.as_deref(),
                        entry.agent_role.as_deref(),
                        /*is_primary*/ false,
                    )
                });
            let is_active = matches!(
                status,
                super::agent_status_feed::AgentTaskStatus::Running
                    | super::agent_status_feed::AgentTaskStatus::NeedsInput
                    | super::agent_status_feed::AgentTaskStatus::NeedsApproval
            );
            let (hidden, newly_finished) = self.finished_agent_rows.observe(
                thread_id,
                status,
                is_active,
                now,
                dismiss_finished,
            );
            schedule_expiry |= newly_finished;
            if hidden {
                continue;
            }
            let mut label = status.label().to_string();
            if entry.is_closed && status != super::agent_status_feed::AgentTaskStatus::Closed {
                label.push_str(" · offline");
            }
            panel_rows.push(crate::bottom_pane::LiveAgentRow {
                name,
                status: label,
                marker: status.marker(),
                activity: store
                    .as_deref()
                    .and_then(super::agent_status_feed::latest_activity),
                is_active,
            });
        }
        self.chat_widget.set_task_summary(total, working);
        self.chat_widget.set_live_agent_panel(panel_rows);
        if schedule_expiry {
            self.schedule_finished_agent_row_expiry();
        }
    }

    /// One-shot wake-up for the 30 s expiry; no polling. Without a runtime (sync unit tests)
    /// the next notification-driven sync still applies the expiry.
    fn schedule_finished_agent_row_expiry(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let app_event_tx = self.app_event_tx.clone();
        runtime.spawn(async move {
            tokio::time::sleep(FINISHED_AGENT_ROW_TTL).await;
            app_event_tx.send(AppEvent::RefreshLiveAgentPanel);
        });
    }

    /// Rebuilds only an already-visible task picker, retaining the user's selected row.
    /// No network request or blocking store lock is needed on the notification path.
    pub(super) fn refresh_visible_agent_picker(&mut self) {
        self.sync_task_summary();
        let Some(selected) = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID)
        else {
            return;
        };
        let params = self.agent_picker_selection_view_params(Some(selected));
        self.chat_widget
            .replace_selection_view_if_present(AGENT_PICKER_VIEW_ID, params);
    }

    pub(super) fn agent_picker_selection_view_params(
        &self,
        selected: Option<usize>,
    ) -> SelectionViewParams {
        let mut items = vec![SelectionItem {
            name: "Background terminals".to_string(),
            name_prefix_spans: vec!["›".dim(), " ".into()],
            description: Some("Commands in the displayed conversation".to_string()),
            actions: vec![Box::new(|tx| tx.send(AppEvent::ShowBackgroundTerminals))],
            dismiss_on_select: true,
            ..Default::default()
        }];

        if !self.config.features.enabled(Feature::Collab) {
            items.push(SelectionItem {
                name: "Enable subagents".to_string(),
                description: Some("Review the setting for future sessions".to_string()),
                actions: vec![Box::new(|tx| tx.send(AppEvent::OpenMultiAgentEnablePrompt))],
                dismiss_on_select: true,
                ..Default::default()
            });
        }

        let action_count = items.len();
        let mut initial_selected_idx = selected;
        let mut running = 0;
        let agent_items = self
            .agent_navigation
            .visible_threads()
            .into_iter()
            .enumerate()
            .map(|(idx, (thread_id, entry))| {
                if initial_selected_idx.is_none() && self.active_thread_id == Some(thread_id) {
                    initial_selected_idx = Some(action_count + idx);
                }
                let is_primary = self.primary_thread_id == Some(thread_id);
                let name = if is_primary {
                    "Main conversation".to_string()
                } else {
                    entry
                        .agent_path
                        .as_deref()
                        .map(str::trim)
                        .filter(|agent_path| !agent_path.is_empty())
                        .map(|path| path.strip_prefix("/root/").unwrap_or(path).to_owned())
                        .unwrap_or_else(|| {
                            format_agent_picker_item_name(
                                entry.agent_nickname.as_deref(),
                                entry.agent_role.as_deref(),
                                is_primary,
                            )
                        })
                };
                let store = self
                    .thread_event_channels
                    .get(&thread_id)
                    .and_then(|channel| channel.store.try_lock().ok());
                let status =
                    super::agent_status_feed::AgentTaskStatus::from_entry(entry, store.as_deref());
                if status == super::agent_status_feed::AgentTaskStatus::Running {
                    running += 1;
                }
                let preview = store
                    .as_deref()
                    .and_then(super::agent_status_feed::latest_activity);
                let mut description = status.label().to_string();
                if entry.is_closed && status != super::agent_status_feed::AgentTaskStatus::Closed {
                    description.push_str(" · offline");
                }
                if self.agent_navigation.is_parent_owned(thread_id) {
                    description.push_str(" · view only");
                }
                if let Some(preview) = preview {
                    description.push_str(" · ");
                    description.push_str(&preview);
                }
                let uuid = thread_id.to_string();
                SelectionItem {
                    name: name.clone(),
                    name_prefix_spans: vec![status.marker(), " ".into()],
                    description: Some(description),
                    is_current: self.active_thread_id == Some(thread_id),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::SelectAgentThread(thread_id));
                    })],
                    dismiss_on_select: true,
                    search_value: Some(format!(
                        "{name} {} {uuid}",
                        entry.agent_path.as_deref().unwrap_or_default()
                    )),
                    ..Default::default()
                }
            })
            .collect::<Vec<_>>();
        if initial_selected_idx.is_none() && !agent_items.is_empty() {
            initial_selected_idx = Some(action_count);
        }
        items.extend(agent_items);

        SelectionViewParams {
            view_id: Some(AGENT_PICKER_VIEW_ID),
            title: Some(format!("Tasks · {running} running")),
            subtitle: Some(if self.agent_navigation.is_empty() {
                "No agents yet. Background terminals are available below.".to_string()
            } else {
                AgentNavigationState::picker_subtitle()
            }),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            initial_selected_idx,
            ..Default::default()
        }
    }

    pub(super) fn refresh_agent_picker_threads(
        &mut self,
        app_server: &AppServerSession,
        root: ThreadId,
    ) {
        let Some(request_id) = self.agent_navigation.begin_picker_refresh(root) else {
            return;
        };
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = async {
                let mut refresh = AgentPickerThreadRefresh::default();
                for archived in [false, true] {
                    let mut cursor = None;
                    let mut seen_cursors = HashSet::new();
                    let mut listed = 0;
                    while listed < AGENT_PICKER_MAX_THREADS && seen_cursors.insert(cursor.clone()) {
                        let page = match request_handle
                            .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                                request_id: RequestId::String(Uuid::new_v4().to_string()),
                                params: ThreadListParams {
                                    excluded_thread_ids: None,
                                    attached_to_thread_id: None,
                                    originators: None,
                                    cursor,
                                    limit: Some(AGENT_PICKER_PAGE_SIZE),
                                    sort_key: None,
                                    sort_direction: Some(SortDirection::Desc),
                                    model_providers: Some(vec![]),
                                    source_kinds: Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
                                    archived: Some(archived),
                                    section_id: None,
                                    project_id: None,
                                    cwd: None,
                                    use_state_db_only: true,
                                    search_term: None,
                                    parent_thread_id: None,
                                    ancestor_thread_id: Some(root.to_string()),
                                },
                            })
                            .await
                        {
                            Ok(page) => page,
                            Err(err) if !archived && listed == 0 => return Err(err.to_string()),
                            Err(err) => {
                                tracing::warn!(%err, archived, "incomplete agent picker refresh");
                                break;
                            }
                        };
                        let page_threads: Vec<_> = page
                            .data
                            .into_iter()
                            .take(AGENT_PICKER_MAX_THREADS - listed)
                            .collect();
                        listed += page_threads.len();
                        if archived {
                            refresh.archived_thread_ids.extend(
                                page_threads
                                    .into_iter()
                                    .filter_map(|thread| ThreadId::from_string(&thread.id).ok()),
                            );
                        } else {
                            refresh.threads.extend(page_threads);
                        }
                        let Some(next_cursor) = page.next_cursor else {
                            break;
                        };
                        cursor = Some(next_cursor);
                    }
                }
                refresh.threads.reverse();
                Ok(refresh)
            }
            .await;

            app_event_tx.send(AppEvent::AgentPickerThreadsLoaded {
                primary_thread_id: root,
                request_id,
                result,
            });
        });
    }

    pub(super) fn apply_agent_picker_thread_refresh(
        &mut self,
        app_server: &AppServerSession,
        root: ThreadId,
        request_id: Uuid,
        result: Result<AgentPickerThreadRefresh, String>,
    ) {
        let Some(completion) = self
            .agent_navigation
            .finish_picker_refresh(root, request_id)
        else {
            return;
        };
        if self.primary_thread_id != Some(root) {
            return;
        }
        let refresh = match result {
            Ok(refresh) => refresh,
            Err(err) => {
                tracing::warn!(%err, "failed to refresh agent picker descendants");
                self.agent_navigation.prune_untracked_picker_exclusions();
                if completion.follow_up_requested {
                    self.refresh_agent_picker_threads(app_server, root);
                }
                return;
            }
        };
        let selected_thread_id = self.selected_agent_picker_thread_id();
        for thread in refresh.threads {
            let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
                continue;
            };
            if !completion.visibility_changed_threads.contains(&thread_id) {
                self.update_agent_picker_thread_visibility(
                    thread_id,
                    AgentPickerThreadVisibility::Visible,
                );
            }
            let live = self
                .thread_event_channels
                .get(&thread_id)
                .is_some_and(|channel| channel.attachment() == ThreadEventAttachment::Live);
            let previous = self.agent_navigation.get(&thread_id);
            let is_running = matches!(thread.status, ThreadStatus::Active { .. });
            let update_liveness = previous.is_none() || !is_running;
            let is_closed = !live && matches!(thread.status, ThreadStatus::NotLoaded);
            if !is_closed && previous.is_some_and(|entry| entry.is_closed) {
                continue;
            }
            let agent_path = crate::app_server_session::source_agent_path(&thread.source);
            let agent_nickname = thread
                .agent_nickname
                .or_else(|| previous.and_then(|entry| entry.agent_nickname.clone()));
            let agent_role = thread
                .agent_role
                .or_else(|| previous.and_then(|entry| entry.agent_role.clone()));
            if thread.can_accept_direct_input == Some(false) {
                self.agent_navigation.mark_parent_owned(thread_id);
            }
            self.upsert_agent_picker_thread(thread_id, agent_nickname, agent_role, is_closed);
            self.agent_navigation.set_agent_path(thread_id, agent_path);
            if !live && update_liveness {
                self.agent_navigation.set_running(thread_id, is_running);
            }
        }
        for thread_id in refresh.archived_thread_ids {
            if !completion.visibility_changed_threads.contains(&thread_id) {
                self.update_agent_picker_thread_visibility(
                    thread_id,
                    AgentPickerThreadVisibility::Hidden,
                );
            }
        }
        self.agent_navigation.prune_untracked_picker_exclusions();

        self.sync_task_summary();
        self.replace_agent_picker_view(selected_thread_id);
        if completion.follow_up_requested {
            self.refresh_agent_picker_threads(app_server, root);
        }
    }

    pub(super) fn set_agent_picker_thread_visibility(
        &mut self,
        thread_id: ThreadId,
        visibility: AgentPickerThreadVisibility,
    ) {
        let selected_thread_id = self.selected_agent_picker_thread_id();
        self.update_agent_picker_thread_visibility(thread_id, visibility);
        self.replace_agent_picker_view(selected_thread_id);
    }

    fn update_agent_picker_thread_visibility(
        &mut self,
        thread_id: ThreadId,
        visibility: AgentPickerThreadVisibility,
    ) {
        if matches!(visibility, AgentPickerThreadVisibility::Hidden) {
            self.agent_navigation.mark_stopped(thread_id);
            if let Some(channel) = self.thread_event_channels.get_mut(&thread_id) {
                channel.mark_replay_only();
            }
        }
        self.agent_navigation
            .set_picker_thread_visibility(thread_id, visibility);
    }

    pub(super) fn handle_agent_picker_visibility_notification(
        &mut self,
        app_server: &AppServerSession,
        notification: &ServerNotification,
    ) {
        let visibility = match notification {
            ServerNotification::ThreadArchived(_) => AgentPickerThreadVisibility::Hidden,
            ServerNotification::ThreadUnarchived(_) => AgentPickerThreadVisibility::Visible,
            _ => return,
        };
        let ServerNotificationThreadTarget::Thread(thread_id) =
            server_notification_thread_target(notification)
        else {
            return;
        };
        let refresh_uncached_thread = matches!(visibility, AgentPickerThreadVisibility::Visible)
            && self.agent_navigation.get(&thread_id).is_none();
        self.set_agent_picker_thread_visibility(thread_id, visibility);
        if refresh_uncached_thread
            && let Some(primary_thread_id) = self.primary_thread_id
            && !self
                .agent_navigation
                .queue_picker_refresh(primary_thread_id)
        {
            self.refresh_agent_picker_threads(app_server, primary_thread_id);
        }
    }

    fn selected_agent_picker_thread_id(&self) -> Option<ThreadId> {
        let selected = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID)?;
        let visible = self.agent_navigation.visible_threads();
        let action_count = self
            .agent_picker_selection_view_params(None)
            .items
            .len()
            .saturating_sub(visible.len());
        visible
            .get(selected.checked_sub(action_count)?)
            .map(|(thread_id, _)| *thread_id)
    }

    fn replace_agent_picker_view(&mut self, selected_thread_id: Option<ThreadId>) {
        let action_count = self
            .agent_picker_selection_view_params(None)
            .items
            .len()
            .saturating_sub(self.agent_navigation.visible_threads().len());
        let selected = selected_thread_id.and_then(|selected_thread_id| {
            self.agent_navigation
                .visible_threads()
                .iter()
                .position(|(thread_id, _)| *thread_id == selected_thread_id)
                .map(|index| index + action_count)
        });
        let params = self.agent_picker_selection_view_params(selected);
        self.chat_widget
            .replace_selection_view_if_present(AGENT_PICKER_VIEW_ID, params);
    }
}
