use std::sync::Arc;

use gpui::{AppContext, Context, Entity, IntoElement, Window, div, prelude::*, rgb};

use crate::features::config::ConfigStore;
use crate::ui::{
    disclaimer::{Disclaimer, DisclaimerEvent},
    hub::Hub,
    theme,
};

enum Screen {
    Disclaimer(Entity<Disclaimer>),
    Hub(Entity<Hub>),
}

pub struct RootView {
    store: Arc<ConfigStore>,
    screen: Screen,
}

impl RootView {
    pub fn new(store: Arc<ConfigStore>, cx: &mut Context<Self>) -> Self {
        let disclaimer = cx.new(|_| Disclaimer::new());
        cx.subscribe(&disclaimer, Self::on_disclaimer_event)
            .detach();

        Self {
            store,
            screen: Screen::Disclaimer(disclaimer),
        }
    }

    fn on_disclaimer_event(
        &mut self,
        _disclaimer: Entity<Disclaimer>,
        event: &DisclaimerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            DisclaimerEvent::Continue => {
                let store = self.store.clone();
                self.screen = Screen::Hub(cx.new(|cx| Hub::new(store, cx)));
                cx.notify();
            }
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.screen {
            Screen::Disclaimer(view) => view.clone().into_any_element(),
            Screen::Hub(view) => view.clone().into_any_element(),
        };

        div()
            .size_full()
            .font_family("Nunito")
            .bg(rgb(theme::BACKGROUND))
            .text_color(rgb(theme::TEXT))
            .child(content)
    }
}
