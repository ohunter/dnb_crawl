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
}

impl DownloadListItemComponent {
    pub async fn is_done(&self) -> WebDriverResult<bool> {
        Ok(self.base.attr("state").await?.unwrap() == "1")
    }

    pub async fn filename(&self) -> WebDriverResult<String> {
        Ok(self.description.resolve().await?.value().await?.unwrap())
    }
}
