use thirtyfour::prelude::*;
use thirtyfour::{
    components::{Component, ElementResolver},
    WebElement,
};

#[derive(Debug, Clone, Component)]
pub struct DownloadListItemComponent {
    base: WebElement,

    #[by(css = "description[class='downloadTarget']", nowait)]
    description: ElementResolver<WebElement>,

    _is_done: bool,
}

impl DownloadListItemComponent {
    pub async fn update_state(&mut self) -> WebDriverResult<()> {
        self._is_done = self.base.attr("state").await?.unwrap() == "1";
        Ok(())
    }

    pub async fn filename(&self) -> WebDriverResult<String> {
        Ok(self.description.resolve().await?.value().await?.unwrap())
    }

    pub fn is_done(&self) -> bool {
        self._is_done
    }
}
