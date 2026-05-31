use gpui::{Context, EventEmitter, IntoElement, Window, div, prelude::*, px, rgb};

use crate::ui::theme;

const DISCLAIMER_TEXT: &str = "This software is provided \"as is\", without warranty of any kind. \
You use it entirely at your own risk. The authors are not responsible for any damage, data loss, \
account suspensions, bans, or any other consequences arising from its use. \
By continuing, you acknowledge and accept these terms.";

pub enum DisclaimerEvent {
    Continue,
}

#[derive(Default)]
pub struct Disclaimer;

impl Disclaimer {
    pub fn new() -> Self {
        Self
    }
}

impl EventEmitter<DisclaimerEvent> for Disclaimer {}

impl Render for Disclaimer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_6()
            .p_6()
            .child(div().text_xl().child("Disclaimer"))
            .child(div().text_sm().text_center().child(DISCLAIMER_TEXT))
            .child(
                div()
                    .id("continue")
                    .w(px(160.0))
                    .py(px(8.0))
                    .rounded_full()
                    .bg(rgb(theme::ACCENT))
                    .text_color(rgb(theme::ON_ACCENT))
                    .text_center()
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(theme::ACCENT_HOVER)))
                    .child("Continue")
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        cx.emit(DisclaimerEvent::Continue);
                    })),
            )
    }
}
