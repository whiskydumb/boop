use gpui::{Hsla, Svg, prelude::*, svg};

#[derive(Clone, Copy)]
pub enum IconName {
    Play,
    Pause,
    Search,
    RunAll,
}

impl IconName {
    fn path(self) -> &'static str {
        match self {
            IconName::Play => "icons/play.svg",
            IconName::Pause => "icons/pause.svg",
            IconName::Search => "icons/search.svg",
            IconName::RunAll => "icons/run-all.svg",
        }
    }
}

pub fn icon(name: IconName, color: impl Into<Hsla>) -> Svg {
    svg().path(name.path()).text_color(color)
}
