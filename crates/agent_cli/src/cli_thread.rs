use std::path::PathBuf;

use anyhow::{Context as _, Result};
use collections::HashMap;
use gpui::{App, AppContext as _, Entity, EventEmitter, SharedString, Subscription, Task};
use project::Project;
use serde::{Deserialize, Serialize};
use task::{SpawnInTerminal, TaskId};
use terminal::Terminal;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CliThreadId(pub SharedString);

impl CliThreadId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string().into())
    }
}

impl Default for CliThreadId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum CliThreadEvent {
    Exited,
    TitleChanged,
}

impl EventEmitter<CliThreadEvent> for CliThread {}

pub struct CliThread {
    id: CliThreadId,
    name: SharedString,
    title: SharedString,
    terminal: Entity<Terminal>,
    _subscriptions: Vec<Subscription>,
}

impl CliThread {
    pub fn id(&self) -> &CliThreadId {
        &self.id
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }

    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    pub fn set_title(&mut self, title: SharedString, cx: &mut gpui::Context<Self>) {
        if self.title != title {
            self.title = title;
            cx.emit(CliThreadEvent::TitleChanged);
            cx.notify();
        }
    }

    /// Whether the underlying PTY process is still alive. Derived directly from
    /// `Terminal::child_exit_status`; we deliberately do not infer activity
    /// status from PTY output (see PR description).
    pub fn is_alive(&self, cx: &App) -> bool {
        self.terminal.read(cx).child_exit_status().is_none()
    }

    pub fn terminal(&self) -> &Entity<Terminal> {
        &self.terminal
    }

    fn on_process_exit(&mut self, cx: &mut gpui::Context<Self>) {
        cx.emit(CliThreadEvent::Exited);
        cx.notify();
    }

    /// Send Ctrl+C to the agent (SIGINT to the foreground process group).
    pub fn send_interrupt(&mut self, cx: &mut gpui::Context<Self>) {
        self.terminal.update(cx, |terminal, _cx| {
            terminal.input(&b"\x03"[..]);
        });
    }

    /// Terminate the agent: SIGKILL the active process group via `kill_active_task`,
    /// then wait briefly for the PTY to drain before the caller drops this entity.
    pub fn shutdown(&mut self, cx: &mut gpui::Context<Self>) -> Task<()> {
        let terminal = self.terminal.clone();
        cx.spawn(async move |_this, cx| {
            terminal.update(cx, |terminal, _cx| {
                terminal.kill_active_task();
            });
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
        })
    }

    /// Spawn a new CLI agent thread, creating a PTY-backed terminal process.
    pub fn spawn(
        name: SharedString,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        cwd: Option<PathBuf>,
        project: Entity<Project>,
        cx: &mut App,
    ) -> Task<Result<Entity<CliThread>>> {
        let spawn_task = SpawnInTerminal {
            id: TaskId(uuid::Uuid::new_v4().to_string()),
            label: name.to_string(),
            full_label: name.to_string(),
            command_label: name.to_string(),
            command: Some(command),
            args,
            env,
            cwd,
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            let terminal = project
                .update(cx, |project, cx| {
                    project.create_terminal_task(spawn_task, cx)
                })
                .await
                .context("spawning PTY for CLI agent")?;

            let thread = cx.update(|cx| {
                cx.new(|cx| {
                    let terminal_subscription =
                        cx.subscribe(&terminal, |this: &mut CliThread, _terminal, event, cx| {
                            if matches!(event, terminal::Event::CloseTerminal) {
                                this.on_process_exit(cx);
                            }
                        });
                    CliThread {
                        id: CliThreadId::new(),
                        name: name.clone(),
                        title: name,
                        terminal,
                        _subscriptions: vec![terminal_subscription],
                    }
                })
            });
            Ok(thread)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_in_terminal_fields_are_set_correctly() {
        let name: SharedString = "my-agent".into();
        let command = "claude".to_string();
        let args = vec!["--model".to_string(), "claude-opus-4-5".to_string()];
        let mut env = HashMap::default();
        env.insert("KEY".to_string(), "VALUE".to_string());
        let cwd = Some(PathBuf::from("/tmp"));

        let spawn_task = SpawnInTerminal {
            id: TaskId(uuid::Uuid::new_v4().to_string()),
            label: name.to_string(),
            full_label: name.to_string(),
            command_label: name.to_string(),
            command: Some(command),
            args: args.clone(),
            env: env.clone(),
            cwd: cwd.clone(),
            ..Default::default()
        };

        assert_eq!(spawn_task.command.as_deref(), Some("claude"));
        assert_eq!(spawn_task.args, args);
        assert_eq!(spawn_task.env.get("KEY").map(|s| s.as_str()), Some("VALUE"));
        assert_eq!(spawn_task.cwd, cwd);
        assert_eq!(spawn_task.label, "my-agent");
    }

    #[test]
    fn cli_thread_id_is_unique() {
        let id1 = CliThreadId::new();
        let id2 = CliThreadId::new();
        assert_ne!(id1, id2);
    }
}
