use agent_cli::{CliThread, CliThreadEvent};
use editor::{Editor, EditorEvent};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Subscription, Window, div,
};
use terminal_view::TerminalView;
use ui::{IconButton, IconButtonShape, IconName, IconSize, Label, LabelSize, Tooltip, prelude::*};

pub struct CliConversationView {
    thread: Entity<CliThread>,
    terminal_view: Entity<TerminalView>,
    title_editor: Option<Entity<Editor>>,
    title_editor_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone)]
pub enum CliConversationViewEvent {
    CloseRequested,
    RestartRequested,
    TitleChanged,
}

impl EventEmitter<CliConversationViewEvent> for CliConversationView {}

impl CliConversationView {
    pub fn new(
        thread: Entity<CliThread>,
        workspace: gpui::WeakEntity<workspace::Workspace>,
        project: Entity<project::Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let terminal = thread.read(cx).terminal().clone();
        let terminal_view = cx.new(|cx| {
            TerminalView::new(terminal, workspace, None, project.downgrade(), window, cx)
        });

        let subscription = cx.subscribe(&thread, |_this, _thread, _event: &CliThreadEvent, cx| {
            cx.notify();
        });

        Self {
            thread,
            terminal_view,
            title_editor: None,
            title_editor_subscription: None,
            _subscriptions: vec![subscription],
        }
    }

    pub fn thread(&self) -> &Entity<CliThread> {
        &self.thread
    }

    pub fn title(&self, cx: &App) -> SharedString {
        self.thread.read(cx).title()
    }

    pub fn is_alive(&self, cx: &App) -> bool {
        self.thread.read(cx).is_alive(cx)
    }

    pub fn title_editor(&self) -> Option<&Entity<Editor>> {
        self.title_editor.as_ref()
    }

    pub fn begin_editing_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(title_editor) = self.title_editor.as_ref() {
            title_editor.focus_handle(cx).focus(window, cx);
            return;
        }
        let title = self.title(cx).to_string();
        let title_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text(title, window, cx);
            editor
        });
        let subscription = cx.subscribe_in(&title_editor, window, Self::handle_title_editor_event);
        title_editor.update(cx, |editor, cx| {
            editor.select_all(&editor::actions::SelectAll, window, cx);
            editor.focus_handle(cx).focus(window, cx);
        });
        self.title_editor = Some(title_editor);
        self.title_editor_subscription = Some(subscription);
        cx.notify();
    }

    pub fn stop_editing_title(
        &mut self,
        focus_terminal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.title_editor = None;
        self.title_editor_subscription = None;
        if focus_terminal {
            self.terminal_view
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        }
        cx.notify();
    }

    fn handle_title_editor_event(
        &mut self,
        title_editor: &Entity<Editor>,
        event: &EditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            EditorEvent::BufferEdited => {
                if !title_editor.read(cx).is_focused(window) {
                    return;
                }
                let new_title = title_editor.read(cx).text(cx).trim().to_string();
                if new_title.is_empty() {
                    return;
                }
                let title = SharedString::from(new_title);
                self.thread.update(cx, |thread, cx| {
                    thread.set_title(title, cx);
                });
                cx.emit(CliConversationViewEvent::TitleChanged);
            }
            EditorEvent::Blurred => {
                self.stop_editing_title(false, window, cx);
            }
            _ => {}
        }
    }

    fn render_title_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.title(cx);
        let is_alive = self.is_alive(cx);

        let title_element = if let Some(title_editor) = self.title_editor.as_ref() {
            title_editor.clone().into_any_element()
        } else {
            Label::new(title)
                .size(LabelSize::Small)
                .single_line()
                .into_any_element()
        };

        h_flex()
            .w_full()
            .py_1p5()
            .px_2()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(
                div()
                    .id("cli-thread-title")
                    .flex_1()
                    .cursor_text()
                    .overflow_x_scroll()
                    .child(title_element)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.begin_editing_title(window, cx);
                    })),
            )
            .when(!is_alive, |row| {
                row.child(
                    IconButton::new("restart", IconName::RotateCw)
                        .shape(IconButtonShape::Square)
                        .icon_size(IconSize::Small)
                        .tooltip(Tooltip::text("Restart Agent"))
                        .on_click(cx.listener(|_this, _, _window, cx| {
                            cx.emit(CliConversationViewEvent::RestartRequested);
                        })),
                )
            })
            .child(
                IconButton::new("close", IconName::Close)
                    .shape(IconButtonShape::Square)
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text("Close Agent"))
                    .on_click(cx.listener(|_this, _, _window, cx| {
                        cx.emit(CliConversationViewEvent::CloseRequested);
                    })),
            )
    }
}

impl Render for CliConversationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let terminal_focus = self.terminal_view.read(cx).focus_handle(cx);
        v_flex()
            .size_full()
            .track_focus(&terminal_focus)
            .child(self.render_title_strip(cx))
            .child(div().flex_1().size_full().child(self.terminal_view.clone()))
    }
}

impl Focusable for CliConversationView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if let Some(title_editor) = self.title_editor.as_ref() {
            title_editor.read(cx).focus_handle(cx)
        } else {
            self.terminal_view.read(cx).focus_handle(cx)
        }
    }
}
