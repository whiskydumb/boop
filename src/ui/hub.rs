use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, FocusHandle, IntoElement, KeyDownEvent, SharedString, Window, div,
    prelude::*, px, rgb, rgba,
};

use crate::features::apps::AppEntry;
use crate::features::config::{Config, ConfigStore};
use crate::features::launcher::Launcher;
use crate::features::update::{self, UpdateInfo};
use crate::platform;
use crate::ui::icon::{IconName, icon};
use crate::ui::theme;

const MUTED: u32 = 0xf0e3de99;
const SURFACE: u32 = 0xffffff14;
const SEPARATOR: u32 = 0xffffff10;

struct Queue {
    ids: Vec<String>,
    index: usize,
    ends_at: Instant,
    step: Duration,
}

pub struct Hub {
    store: Arc<ConfigStore>,
    launcher: Launcher,
    error: Option<String>,
    query: String,
    search_focus: FocusHandle,
    update: Option<UpdateInfo>,
    error_expanded: bool,
    queue: Option<Queue>,
    search_open: bool,
}

impl Hub {
    pub fn new(store: Arc<ConfigStore>, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |this, cx| {
                    let exited = this.launcher.poll();
                    let reloaded = this.store.take_dirty();
                    let queue_was_active = this.queue.is_some();
                    let queue_active = this.tick_queue(exited);
                    if this.error_expanded && this.store.error().is_none() {
                        this.error_expanded = false;
                    }
                    // redraw while a pass runs (countdown) and on the tick it ends.
                    if exited
                        || reloaded
                        || queue_was_active
                        || queue_active
                        || this.launcher.running_id().is_some()
                    {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { update::check() })
                .await;
            match found {
                Ok(Some(info)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.update = Some(info);
                        cx.notify();
                    });
                }
                Ok(None) => {}
                Err(error) => eprintln!("update check failed: {error:#}"),
            }
        })
        .detach();

        Self {
            store,
            launcher: Launcher::new(),
            error: None,
            query: String::new(),
            search_focus: cx.focus_handle(),
            update: None,
            error_expanded: false,
            queue: None,
            search_open: false,
        }
    }

    fn config(&self) -> Arc<Config> {
        self.store.snapshot()
    }

    fn on_search_key(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.query.pop();
            }
            "escape" => {
                self.query.clear();
                self.search_open = false;
            }
            _ => {
                if let Some(text) = &event.keystroke.key_char
                    && !text.chars().any(char::is_control)
                {
                    self.query.push_str(text);
                }
            }
        }
        cx.notify();
    }

    fn status_line(&self) -> String {
        if let Some(queue) = &self.queue {
            let position = queue.index + 1;
            let total = queue.ids.len();
            let left = queue.ends_at.saturating_duration_since(Instant::now());
            return format!("Queue {position}/{total}  |  {} left", format_runtime(left));
        }

        if let Some(error) = &self.error {
            return format!("! {error}");
        }

        match self.launcher.running_since() {
            Some(since) => format!("Uptime  {}", format_runtime(since.elapsed())),
            None => "Idle".to_string(),
        }
    }

    fn queue_active(&self) -> bool {
        self.queue.is_some()
    }

    fn toggle_queue(&mut self) {
        if self.queue.is_some() {
            self.stop_queue();
        } else {
            self.start_queue();
        }
    }

    fn start_queue(&mut self) {
        let config = self.config();
        let query = self.query.trim().to_lowercase();
        let ids: Vec<String> = config
            .apps()
            .iter()
            .filter(|app| query.is_empty() || app.name.to_lowercase().contains(&query))
            .map(|app| app.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }

        let step = Duration::from_secs(u64::from(config.quest_minutes.max(1)) * 60);
        self.queue = Some(Queue {
            ids,
            index: 0,
            ends_at: Instant::now() + step,
            step,
        });

        // launch the first app; if it cant start, skip ahead like a normal step
        if let Some(first) = self.queue.as_ref().and_then(|q| q.ids.first()).cloned() {
            if self.launch_id(&first) {
                if let Some(queue) = &mut self.queue {
                    queue.ends_at = Instant::now() + step;
                }
            } else {
                self.advance_queue();
            }
        }
    }

    fn stop_queue(&mut self) {
        self.launcher.kill();
        self.queue = None;
    }

    fn advance_queue(&mut self) {
        loop {
            let (id, step) = {
                let Some(queue) = &mut self.queue else {
                    return;
                };
                queue.index += 1;
                match queue.ids.get(queue.index) {
                    Some(id) => (id.clone(), queue.step),
                    None => break,
                }
            };

            if self.launch_id(&id) {
                if let Some(queue) = &mut self.queue {
                    queue.ends_at = Instant::now() + step;
                }
                return;
            }
        }

        self.stop_queue();
    }

    /// @return whether a queue is currently active
    fn tick_queue(&mut self, current_exited: bool) -> bool {
        let due = match &self.queue {
            Some(queue) => current_exited || Instant::now() >= queue.ends_at,
            None => return false,
        };
        if due {
            self.advance_queue();
        }
        self.queue.is_some()
    }

    /// @return true on success; false if the entry is gone or failed to start
    fn launch_id(&mut self, id: &str) -> bool {
        let config = self.config();
        let Some(entry) = config.apps().iter().find(|app| app.id == id).cloned() else {
            return false;
        };
        let exe = config.resolve_exe(&entry);
        let cwd = config.resolve_cwd(&entry);
        match self.launcher.launch(&entry, &exe, cwd.as_deref()) {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                false
            }
        }
    }

    fn toggle(&mut self, entry: &AppEntry) {
        if self.launcher.is_running(&entry.id) {
            self.launcher.kill();
            return;
        }

        let config = self.config();
        let exe = config.resolve_exe(entry);
        let cwd = config.resolve_cwd(entry);

        match self.launcher.launch(entry, &exe, cwd.as_deref()) {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = self.config();
        if config.apps().is_empty() {
            return self.empty_state("No apps yet. Add them in config.toml.");
        }

        let query = self.query.trim().to_lowercase();
        let matches: Vec<&AppEntry> = config
            .apps()
            .iter()
            .filter(|app| query.is_empty() || app.name.to_lowercase().contains(&query))
            .collect();

        if matches.is_empty() {
            return self.empty_state(&format!("No apps match '{}'", self.query));
        }

        let mut rows = Vec::with_capacity(matches.len());
        for entry in matches {
            rows.push(self.render_row(entry, cx));
        }

        div()
            .id("app-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .overflow_y_scroll()
            .children(rows)
            .into_any_element()
    }

    fn render_update_bar(&self, info: &UpdateInfo, cx: &mut Context<Self>) -> AnyElement {
        let url = info.url.clone();

        div()
            .id("update-bar")
            .flex()
            .items_center()
            .gap_2()
            .flex_none()
            .px(px(10.0))
            .py(px(5.0))
            .rounded_lg()
            .bg(rgb(theme::UPDATE_BAR))
            .text_color(rgb(theme::ON_ACCENT))
            .text_sm()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme::UPDATE_BAR_HOVER)))
            .on_click(cx.listener(move |_this, _event, _window, _cx| {
                platform::open_url(&url);
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(format!("Update available: v{}", info.version)),
            )
            .child(
                div()
                    .id("update-dismiss")
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_4()
                    .rounded_md()
                    .text_color(rgba(0xfffbf5cc))
                    .hover(|style| style.text_color(rgb(theme::ON_ACCENT)))
                    .child("\u{00d7}")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        cx.stop_propagation();
                        this.update = None;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_error_bar(&self, message: &str, cx: &mut Context<Self>) -> AnyElement {
        let bar = div()
            .id("config-error-bar")
            .flex()
            .flex_col()
            .flex_none()
            .gap_1()
            .px(px(10.0))
            .py(px(6.0))
            .rounded_lg()
            .bg(rgb(theme::ERROR_BAR))
            .text_color(rgb(theme::ON_ACCENT))
            .text_sm()
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.error_expanded = !this.error_expanded;
                cx.notify();
            }));

        if self.error_expanded {
            bar.child(
                div()
                    .id("config-error-detail")
                    .flex()
                    .flex_col()
                    .max_h(px(160.0))
                    .overflow_y_scroll()
                    .children(
                        message
                            .lines()
                            .map(|line| div().child(line.to_string()).into_any_element()),
                    ),
            )
            .into_any_element()
        } else {
            bar.child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(error_headline(message)),
            )
            .into_any_element()
        }
    }

    fn empty_state(&self, message: &str) -> AnyElement {
        div()
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(rgba(MUTED))
                    .child(message.to_string()),
            )
            .into_any_element()
    }

    fn render_queue_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.queue_active();
        let glyph = if active {
            IconName::Pause
        } else {
            IconName::RunAll
        };

        div()
            .id("queue-toggle")
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size_8()
            .rounded_lg()
            .bg(rgb(theme::ACCENT))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme::ACCENT_HOVER)))
            .child(icon(glyph, rgb(theme::ON_ACCENT)).size_4())
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.toggle_queue();
                cx.notify();
            }))
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let row = div().flex().items_center().gap_3().px(px(8.0));

        if self.search_open {
            // expanded: the field takes the whole header width.
            row.child(self.render_search_field(cx)).into_any_element()
        } else {
            row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_sm()
                    .text_color(rgba(MUTED))
                    .child(self.status_line()),
            )
            .child(self.render_queue_toggle(cx))
            .child(self.render_search_icon(cx))
            .into_any_element()
        }
    }

    fn render_search_icon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("search-icon")
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size_8()
            .rounded_lg()
            .bg(rgba(SURFACE))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(theme::ACCENT)))
            .child(icon(IconName::Search, rgba(MUTED)).size_4())
            .on_click(cx.listener(|this, _event, window, cx| {
                this.search_open = true;
                window.focus(&this.search_focus, cx);
                cx.notify();
            }))
    }

    fn render_search_field(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let placeholder = self.query.is_empty();
        let text = if placeholder {
            "Search".to_string()
        } else {
            self.query.clone()
        };

        div()
            .id("search")
            .track_focus(&self.search_focus)
            .flex()
            .items_center()
            .gap_2()
            .flex_1()
            .px(px(8.0))
            .py(px(5.0))
            .rounded_lg()
            .bg(rgba(SURFACE))
            .cursor_text()
            .child(icon(IconName::Search, rgba(MUTED)).size_4().flex_none())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_sm()
                    .when(placeholder, |el| el.text_color(rgba(MUTED)))
                    .child(text),
            )
            .child(
                div()
                    .id("search-close")
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_4()
                    .rounded_md()
                    .text_color(rgba(MUTED))
                    .hover(|style| style.text_color(rgb(theme::TEXT)))
                    .child("\u{00d7}")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        cx.stop_propagation();
                        this.query.clear();
                        this.search_open = false;
                        cx.notify();
                    })),
            )
            .on_key_down(cx.listener(Self::on_search_key))
            .on_click(cx.listener(|this, _event, window, cx| {
                window.focus(&this.search_focus, cx);
                cx.notify();
            }))
    }

    fn render_row(&self, entry: &AppEntry, cx: &mut Context<Self>) -> AnyElement {
        let id = entry.id.clone();
        let is_running = self.launcher.is_running(&entry.id);
        let initial = entry
            .name
            .chars()
            .next()
            .unwrap_or('?')
            .to_ascii_uppercase()
            .to_string();
        let glyph = if is_running {
            IconName::Pause
        } else {
            IconName::Play
        };
        let interactive = !self.queue_active();
        let button_bg = if interactive {
            rgb(theme::ACCENT)
        } else {
            rgba(0xff5e1f55)
        };
        let glyph_color = if interactive {
            rgb(theme::ON_ACCENT)
        } else {
            rgba(0xfffbf599)
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .px(px(8.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(rgba(SEPARATOR))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_8()
                    .rounded_md()
                    .bg(rgba(SURFACE))
                    .text_sm()
                    .child(initial),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(entry.name.clone()),
            )
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{id}")))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_8()
                    .rounded_lg()
                    .bg(button_bg)
                    .when(interactive, |el| {
                        el.cursor_pointer()
                            .hover(|style| style.bg(rgb(theme::ACCENT_HOVER)))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                let entry = this
                                    .config()
                                    .apps()
                                    .iter()
                                    .find(|app| app.id == id)
                                    .cloned();
                                if let Some(entry) = entry {
                                    this.toggle(&entry);
                                }
                                cx.notify();
                            }))
                    })
                    .child(icon(glyph, glyph_color).size_4()),
            )
            .into_any_element()
    }
}

impl Render for Hub {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_4()
            .child(self.render_header(cx))
            .child(self.render_list(cx))
            .children(
                self.store
                    .error()
                    .map(|message| self.render_error_bar(&message, cx)),
            )
            .children(
                self.update
                    .as_ref()
                    .map(|info| self.render_update_bar(info, cx)),
            )
    }
}

fn error_headline(message: &str) -> String {
    let line = match message.find("error at line") {
        Some(start) => &message[start..],
        None => message,
    };
    line.lines().next().unwrap_or(message).to_string()
}

fn format_runtime(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    let (hours, minutes, seconds) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
