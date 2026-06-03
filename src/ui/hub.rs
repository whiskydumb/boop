use std::sync::Arc;
use std::time::Duration;

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

pub struct Hub {
    store: Arc<ConfigStore>,
    launcher: Launcher,
    error: Option<String>,
    query: String,
    search_focus: FocusHandle,
    update: Option<UpdateInfo>,
}

impl Hub {
    pub fn new(store: Arc<ConfigStore>, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |this, cx| {
                    let exited = this.launcher.poll();
                    let reloaded = this.store.take_dirty();
                    if exited || reloaded || this.launcher.running_id().is_some() {
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
            "escape" => self.query.clear(),
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
        if let Some(error) = &self.error {
            return format!("! {error}");
        }

        match self.launcher.running_since() {
            Some(since) => format!("Uptime  {}", format_runtime(since.elapsed())),
            None => "Idle".to_string(),
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

    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .w(px(132.0))
            .px(px(8.0))
            .py(px(5.0))
            .rounded_lg()
            .bg(rgba(SURFACE))
            .cursor_text()
            .child(icon(IconName::Search, rgba(MUTED)).size_4().flex_none())
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .when(placeholder, |el| el.text_color(rgba(MUTED)))
                    .child(text),
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
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::ON_ACCENT))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::ACCENT_HOVER)))
                    .child(icon(glyph, rgb(theme::ON_ACCENT)).size_4())
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
                    })),
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px(px(8.0))
                    .child(
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
                    .child(self.render_search(cx)),
            )
            .child(self.render_list(cx))
            .children(
                self.update
                    .as_ref()
                    .map(|info| self.render_update_bar(info, cx)),
            )
    }
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
