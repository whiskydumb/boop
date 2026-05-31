use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|file| file.starts_with(path))
            .map(|file| file.as_ref().into())
            .collect())
    }
}
