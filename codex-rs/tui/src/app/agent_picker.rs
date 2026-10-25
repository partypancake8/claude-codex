//! Root-scoped background refresh for the agent picker.

use super::*;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use ratatui::style::Stylize;
use std::collections::HashSet;

pub(super) const AGENT_PICKER_VIEW_ID: &str = "agent-picker";
const AGENT_PICKER_PAGE_SIZE: u32 = 100;
const AGENT_PICKER_MAX_THREADS: usize = 1_000;

impl App {
    pub(super) fn sync_task_summary(&mut self) {
        let mut total = 0;
        let mut working = 0;
        let mut panel_rows = Vec::new();
        for (thread_id, entry) in self.agent_navigation.ordered_threads() {
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
                is_active: matches!(
                    status,
                    super::agent_status_feed::AgentTaskStatus::Running
                        | super::agent_status_feed::AgentTaskStatus::NeedsInput
                        | super::agent_status_feed::AgentTaskStatus::NeedsApproval
                ),
            });
        }
        self.chat_widget.set_task_summary(total, working);
        self.chat_widget.set_live_agent_panel(panel_rows);
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
            .ordered_threads()
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
                let mut threads = Vec::new();
                let mut cursor = None;
                let mut seen_cursors = HashSet::new();
                while threads.len() < AGENT_PICKER_MAX_THREADS
                    && seen_cursors.insert(cursor.clone())
                {
                    let page = match request_handle
                        .request_typed::<ThreadListResponse>(ClientRequest::ThreadList {
                            request_id: RequestId::String(Uuid::new_v4().to_string()),
                            params: ThreadListParams {
                                cursor,
                                limit: Some(AGENT_PICKER_PAGE_SIZE),
                                sort_key: None,
                                sort_direction: Some(SortDirection::Desc),
                                model_providers: Some(vec![]),
                                source_kinds: Some(vec![ThreadSourceKind::SubAgentThreadSpawn]),
                                archived: None,
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
                        Err(err) if threads.is_empty() => return Err(err.to_string()),
                        Err(err) => {
                            tracing::warn!(%err, "failed to refresh remaining agent picker descendants");
                            break;
                        }
                    };
                    threads.extend(
                        page.data
                            .into_iter()
                            .take(AGENT_PICKER_MAX_THREADS - threads.len()),
                    );
                    let Some(next_cursor) = page.next_cursor else {
                        break;
                    };
                    cursor = Some(next_cursor);
                }
                threads.reverse();
                Ok(threads)
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
        root: ThreadId,
        request_id: Uuid,
        result: Result<Vec<Thread>, String>,
    ) {
        if !self
            .agent_navigation
            .finish_picker_refresh(root, request_id)
            || self.primary_thread_id != Some(root)
        {
            return;
        }
        let threads = match result {
            Ok(threads) => threads,
            Err(err) => {
                tracing::warn!(%err, "failed to refresh agent picker descendants");
                return;
            }
        };
        let selected = self
            .chat_widget
            .selected_index_for_present_view(AGENT_PICKER_VIEW_ID);
        for thread in threads {
            let Ok(thread_id) = ThreadId::from_string(&thread.id) else {
                continue;
            };
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

        self.sync_task_summary();
        let params = self.agent_picker_selection_view_params(selected);
        self.chat_widget
            .replace_selection_view_if_present(AGENT_PICKER_VIEW_ID, params);
    }
}
