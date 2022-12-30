use thirtyfour::prelude::*;
use thirtyfour::{
    components::{Component, ElementResolver},
    WebElement,
};

#[derive(Debug, Clone, Component)]
pub struct ConsentModalComponent {
    base: WebElement,

    #[by(css = "button[class='consent-close']", nowait)]
    close_button: ElementResolver<WebElement>,
}

impl ConsentModalComponent {
    pub async fn is_displayed(&self) -> Result<bool, WebDriverError> {
        self.base.is_displayed().await
    }

    pub async fn close(&self) -> WebDriverResult<()> {
        if self.is_displayed().await.unwrap() {
            self.close_button.resolve().await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Component)]
pub struct LoginFormComponent {
    base: WebElement,

    #[by(css = "input[name='uid']")]
    uid: ElementResolver<WebElement>,

    #[by(css = "button[type='submit']")]
    submit: ElementResolver<WebElement>,
}

impl LoginFormComponent {
    pub async fn fill_in_and_submit(self, uid: String) -> WebDriverResult<()> {
        self.uid.resolve().await?.send_keys(uid).await?;
        self.submit.resolve().await?.click().await
    }
}

#[derive(Debug, Clone, Component)]
pub struct AuthenticationFormComponent {
    base: WebElement,

    #[by(css = "div[id='r_state-2']")]
    pin_and_otp_button: ElementResolver<WebElement>,

    #[by(xpath = "//div[@id='r_state-2']//input[id='phoneCode']")]
    pin_input: ElementResolver<WebElement>,

    #[by(xpath = "//div[@id='r_state-2']//input[id='otpCode']")]
    otp_input: ElementResolver<WebElement>,

    #[by(xpath = "//div[@id='r_state-2']//button[type='submit']")]
    submit: ElementResolver<WebElement>,
}

impl AuthenticationFormComponent {
    pub async fn pin_and_otp_is_active(&self) -> WebDriverResult<bool> {
        // Inactive:
        // class="dnb-accordion__header dnb-accordion__header__icon--right dnb-accordion__header--description"
        // aria-controls="r_state-2-content"
        // aria-expanded="false"
        // role="button"
        // tabindex="0"

        // Active
        // class="dnb-accordion__header dnb-accordion__header__icon--right dnb-accordion__header--prevent-click dnb-accordion__header--description"
        // aria-controls="r_state-2-content"
        // aria-expanded="true"
        // role="button"
        // tabindex="0"
        Ok(self
            .pin_and_otp_button
            .resolve()
            .await?
            .attr("aria-expanded")
            .await?
            .unwrap_or("false".to_string())
            .parse::<bool>()
            .unwrap_or(false))
    }

    pub async fn select_pin_and_otp(&self) -> WebDriverResult<()> {
        let result = self.pin_and_otp_button.resolve().await?.click().await;
        assert!(self.pin_and_otp_is_active().await?);
        result
    }

    pub async fn fill_in_and_submit(self, pin: String, otp: String) -> WebDriverResult<()> {
        self.pin_input.resolve().await?.send_keys(pin).await?;
        self.otp_input.resolve().await?.send_keys(otp).await?;
        self.submit.resolve().await?.click().await
    }
}

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
