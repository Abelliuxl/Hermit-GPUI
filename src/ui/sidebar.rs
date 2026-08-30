use crate::models::{AgentSession, ConnectionState};
use crate::state::AppState;
use gpui::{
    div, prelude::*, px, AnyElement, Context, Entity, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, Render, Stateful, StatefulInteractiveElement, Styled, Window,
};

/// Sidebar: status pill, batch actions, and the session list.
pub struct SidebarView {
    state: Entity<AppState>,
    confirm_action: Option<ConfirmAction>,
}

#[derive(Clone, Copy, PartialEq)]
enum ConfirmAction {
    ArchiveAll,
    DeleteAll,
}

impl SidebarView {
    pub fn new(state: Entity<AppState>, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            confirm_action: None,
        }
    }
}

impl Render for SidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        let (connection_label, pill_color, session_count) = {
            let state = state.read(cx);
            (
                state.connection_state.pill_label().to_string(),
                pill_color(&state.connection_state),
                state.sessions.len(),
            )
        };

        let confirm = self.confirm_action;

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(gpui::px(272.0))
            .bg(crate::ui::theme::Theme::sidebar_bg())
            .border_r_1()
            .border_color(crate::ui::theme::Theme::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .bg(crate::ui::theme::Theme::surface())
                            .child(div().size(px(7.0)).rounded_full().bg(pill_color))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(crate::ui::theme::Theme::text_secondary())
                                    .child(connection_label),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        header_button("Archive all", session_count > 0).on_click(cx.listener(
                            move |this, _event, _window, cx| {
                                this.confirm_action = Some(ConfirmAction::ArchiveAll);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        header_button("Delete all", session_count > 0).on_click(cx.listener(
                            move |this, _event, _window, cx| {
                                this.confirm_action = Some(ConfirmAction::DeleteAll);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .h(px(1.0))
                    .w_full()
                    .bg(crate::ui::theme::Theme::border()),
            )
            .children(match confirm {
                Some(action) => Some(self.render_confirm(action, cx)),
                None => None,
            })
            .child(
                div()
                    .id("sidebar-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.render_session_list(cx)),
            )
    }
}

impl SidebarView {
    fn render_confirm(&self, action: ConfirmAction, cx: &mut Context<Self>) -> AnyElement {
        let count = self.state.read(cx).sessions.len();
        let (title, body, confirm_label) = match action {
            ConfirmAction::ArchiveAll => (
                format!("Archive all {count} sessions?"),
                "Sessions will be removed from the list; transcripts are kept.",
                "Archive all",
            ),
            ConfirmAction::DeleteAll => (
                format!("Delete all {count} sessions?"),
                "Sessions and their transcripts will be permanently deleted.",
                "Delete all",
            ),
        };
        div()
            .mx_3()
            .my_2()
            .p_3()
            .rounded_lg()
            .bg(crate::ui::theme::Theme::surface())
            .border_1()
            .border_color(crate::ui::theme::Theme::border())
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(crate::ui::theme::Theme::text())
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(crate::ui::theme::Theme::text_secondary())
                    .child(body),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .justify_end()
                    .child(
                        small_button("Cancel", crate::ui::theme::Theme::surface_hover()).on_click(
                            cx.listener(move |this, _event, _window, cx| {
                                this.confirm_action = None;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        small_button(
                            confirm_label,
                            if action == ConfirmAction::DeleteAll {
                                crate::ui::theme::Theme::danger()
                            } else {
                                crate::ui::theme::Theme::accent()
                            },
                        )
                        .on_click(cx.listener(
                            move |this, _event, _window, cx| {
                                this.confirm_action = None;
                                let state = this.state.clone();
                                match action {
                                    ConfirmAction::ArchiveAll => {
                                        state.update(cx, |state, cx| state.archive_all(cx))
                                    }
                                    ConfirmAction::DeleteAll => {
                                        state.update(cx, |state, cx| state.delete_all(cx))
                                    }
                                }
                            },
                        )),
                    ),
            )
            .into_any()
    }

    fn render_session_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.clone();
        let (selected_id, sessions) = {
            let state = state.read(cx);
            (
                state
                    .selected_session
                    .as_ref()
                    .map(|session| session.id.clone()),
                state.sessions.clone(),
            )
        };

        if sessions.is_empty() {
            return div()
                .p_4()
                .text_size(px(11.0))
                .text_color(crate::ui::theme::Theme::text_tertiary())
                .child("No sessions yet. Send a message to start one.")
                .into_any();
        }

        let rows = sessions
            .into_iter()
            .map(|session| {
                let is_selected = selected_id.as_deref() == Some(session.id.as_str());
                div()
                    .id(gpui::ElementId::NamedInteger(
                        "session-row".into(),
                        hash_id(&session.id),
                    ))
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(is_selected, |this| {
                        this.bg(crate::ui::theme::Theme::accent_soft())
                    })
                    .hover(|style| style.bg(crate::ui::theme::Theme::surface_hover()))
                    .cursor_pointer()
                    .on_click({
                        let state = state.clone();
                        let session_for_click = session.clone();
                        move |_event, _window, cx| {
                            state.update(cx, |state, cx| {
                                state.resume_session(session_for_click.clone(), cx)
                            });
                        }
                    })
                    .child(render_row_title(&session))
                    .when(!session.subtitle().is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(crate::ui::theme::Theme::text_tertiary())
                                .max_w_full()
                                .text_ellipsis()
                                .child(session.subtitle()),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .text_size(px(10.0))
                            .text_color(crate::ui::theme::Theme::text_tertiary())
                            .children(
                                session
                                    .message_count
                                    .map(|count| div().child(format!("{count} messages"))),
                            )
                            .when(
                                !session.profile.clone().unwrap_or_default().is_empty(),
                                |this| {
                                    this.child(
                                        div().child(session.profile.clone().unwrap_or_default()),
                                    )
                                },
                            ),
                    )
            })
            .collect::<Vec<_>>();

        div().flex().flex_col().children(rows).into_any()
    }
}

fn render_row_title(session: &AgentSession) -> gpui::AnyElement {
    let active = session.is_active == Some(true);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(crate::ui::theme::Theme::text())
                .max_w_full()
                .text_ellipsis()
                .child(session.display_title()),
        )
        .when(active, |this| {
            this.child(
                div()
                    .size(px(6.0))
                    .rounded_full()
                    .bg(crate::ui::theme::Theme::ok()),
            )
        })
        .into_any()
}

fn header_button(label: &'static str, enabled: bool) -> Stateful<gpui::Div> {
    let base = div()
        .id(ElementIdName(label))
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(11.0))
        .cursor_pointer();
    if enabled {
        base.text_color(crate::ui::theme::Theme::text_secondary())
            .hover(|style| {
                style
                    .bg(crate::ui::theme::Theme::surface_hover())
                    .text_color(crate::ui::theme::Theme::text())
            })
    } else {
        base.text_color(crate::ui::theme::Theme::text_tertiary())
    }
}

fn small_button(label: &'static str, color: Hsla) -> Stateful<gpui::Div> {
    div()
        .id(ElementIdName(label))
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(11.0))
        .text_color(gpui::black())
        .bg(color)
        .cursor_pointer()
        .hover(|style| style.opacity(0.85))
}

// ElementId::Name takes a SharedString; tiny shim keeps call sites tidy.
fn ElementIdName(label: &'static str) -> gpui::ElementId {
    gpui::ElementId::Name(label.into())
}

pub fn hash_id(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn pill_color(state: &ConnectionState) -> Hsla {
    match state {
        ConnectionState::Connected => crate::ui::theme::Theme::ok(),
        ConnectionState::Connecting => crate::ui::theme::Theme::warn(),
        ConnectionState::Disconnected => crate::ui::theme::Theme::text_tertiary(),
        ConnectionState::Degraded(_) => crate::ui::theme::Theme::warn(),
        ConnectionState::Failed(_) => crate::ui::theme::Theme::danger(),
    }
}
