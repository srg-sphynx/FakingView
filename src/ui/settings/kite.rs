//! Kite's personal, daily market-data session. Secret storage and HTTP always run on workers.

use crate::service::keychain::api_keys::{KiteCredentials, KiteCredentialsStore};
use crate::service::market::kite;
use crate::service::work_queue::{self, Pending};
use crate::ui::icons::Icon;
use crate::ui::kit::{Autosave, Tone};
use crate::ui::settings::api_keys::{link, step};
use crate::ui::settings::card::StatusMessage;
use crate::ui::settings::window::{card, section, status_message};
use crate::ui::{kit, theme};
use eframe::egui;

#[derive(Default)]
pub struct KiteForm {
    pub api_key: String,
    pub access_token: String,
    api_secret: String,
    request_token: String,
    loaded: bool,
    configured: bool,
    pub status: Option<StatusMessage>,
    task: Option<Pending<Outcome>>,
    autosave: Autosave,
}

enum Outcome {
    Loaded(KiteCredentials),
    Saved(KiteCredentials, Result<(), String>),
    Removed(Result<(), String>),
    Tested(Result<(), String>),
    ExchangeFailed(String),
}

impl KiteForm {
    fn credentials(&self) -> KiteCredentials {
        KiteCredentials {
            api_key: self.api_key.trim().to_owned(),
            access_token: self.access_token.trim().to_owned(),
        }
    }

    fn can_save(&self) -> bool {
        self.credentials().is_configured()
    }

    fn is_busy(&self) -> bool {
        self.task.is_some()
    }

    fn start(
        &mut self,
        context: Option<&egui::Context>,
        action: impl FnOnce() -> Outcome + Send + 'static,
    ) {
        if self.is_busy() {
            return;
        }
        self.status = None;
        self.task = Some(work_queue::SNAPSHOTS.start(context, action));
    }

    fn load(&mut self, context: &egui::Context) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.start(Some(context), || {
            Outcome::Loaded(KiteCredentialsStore::shared().credentials())
        });
    }

    fn save(&mut self, context: &egui::Context) {
        if !self.can_save() {
            return;
        }
        let credentials = self.credentials();
        self.start(Some(context), move || {
            let result = KiteCredentialsStore::shared()
                .save(&credentials)
                .map_err(|error| error.to_string());
            Outcome::Saved(credentials, result)
        });
    }

    fn poll(&mut self) {
        let Some(task) = &mut self.task else {
            return;
        };
        let Some(outcome) = task.poll_or(|| {
            Outcome::ExchangeFailed("The operation stopped unexpectedly. Please try again.".into())
        }) else {
            return;
        };
        self.task = None;
        match outcome {
            Outcome::Loaded(credentials) => {
                self.configured = credentials.is_configured();
                self.api_key = credentials.api_key;
                self.access_token = credentials.access_token;
            }
            Outcome::Saved(credentials, result) => {
                self.configured = true;
                self.api_key = credentials.api_key;
                self.access_token = credentials.access_token;
                self.status = Some(match result {
                    Ok(()) => StatusMessage::ok(crate::platform::saved_securely()),
                    Err(error) => StatusMessage::error(format!(
                        "Active for this session. Secure storage could not save it: {error}"
                    )),
                });
            }
            Outcome::Removed(result) => {
                self.api_key.clear();
                self.access_token.clear();
                self.api_secret.clear();
                self.request_token.clear();
                self.configured = false;
                self.status = Some(match result {
                    Ok(()) => StatusMessage::ok("Kite credentials removed."),
                    Err(error) => StatusMessage::error(format!(
                        "Removed for this session. Secure storage could not remove the saved credentials: {error}"
                    )),
                });
            }
            Outcome::Tested(result) => {
                self.status = Some(match result {
                    Ok(()) => StatusMessage::ok(
                        "Session accepted. Live and historical data also require Kite market-data access.",
                    ),
                    Err(error) => StatusMessage::error(error),
                });
            }
            Outcome::ExchangeFailed(error) => {
                self.status = Some(StatusMessage::error(error));
            }
        }
    }

    fn exchange(&mut self, context: &egui::Context) {
        let api_key = self.api_key.trim().to_owned();
        let api_secret = std::mem::take(&mut self.api_secret);
        let request_token = std::mem::take(&mut self.request_token);
        self.autosave.cancel();
        self.start(Some(context), move || {
            match kite::exchange_request_token(&api_key, api_secret.trim(), request_token.trim()) {
                Ok(access_token) => {
                    let credentials = KiteCredentials {
                        api_key,
                        access_token,
                    };
                    let saved = KiteCredentialsStore::shared()
                        .save(&credentials)
                        .map_err(|error| error.to_string());
                    Outcome::Saved(credentials, saved)
                }
                Err(error) => Outcome::ExchangeFailed(error.to_string()),
            }
        });
    }
}

pub fn body(ui: &mut egui::Ui, form: &mut KiteForm) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    form.load(ui.ctx());
    form.poll();
    let busy = form.is_busy();
    let (badge, tone) = if form.configured {
        ("Daily session configured", Tone::Good)
    } else {
        ("Not configured", Tone::Neutral)
    };
    kit::status_badge(ui, badge, tone);
    ui.add_space(18.0);
    ui.add_enabled_ui(!busy, |ui| {
        section(ui, "Market-data credentials", None, |ui| {
            card(ui, |ui| {
                let key = kit::field(ui, "API key", None, "", &mut form.api_key, true);
                form.autosave.watch(&key, true);
                ui.add_space(12.0);
                let token = kit::field(
                    ui,
                    "Access token",
                    Some(
                        "Kite access tokens expire daily. Renew the session when Kite rejects it.",
                    ),
                    "",
                    &mut form.access_token,
                    true,
                );
                form.autosave.watch(&token, true);
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if kit::sheet_button(ui, "Test session", false, form.can_save()).clicked() {
                        let credentials = form.credentials();
                        form.start(Some(ui.ctx()), move || {
                            Outcome::Tested(
                                kite::test_connection(
                                    &credentials.api_key,
                                    &credentials.access_token,
                                )
                                .map_err(|error| error.to_string()),
                            )
                        });
                    }
                    if form.configured
                        && kit::destructive_button(ui, "Remove credentials").clicked()
                    {
                        form.autosave.cancel();
                        form.start(Some(ui.ctx()), || {
                            Outcome::Removed(
                                KiteCredentialsStore::shared()
                                    .remove()
                                    .map_err(|error| error.to_string()),
                            )
                        });
                    }
                });
            });
        });
        if (!form.api_key.is_empty() || !form.access_token.is_empty()) && !form.can_save() {
            status_message(
                ui,
                "Both credentials are required. Partial edits are not saved.",
                true,
            );
        }
        section(ui, "Start a daily session", None, |ui| {
            card(ui, |ui| {
                step(
                    ui,
                    1,
                    "Create a personal Kite Connect app with market-data access.",
                );
                step(
                    ui,
                    2,
                    "Enter its API key above, then sign in through Kite below.",
                );
                step(
                    ui,
                    3,
                    "Copy request_token from the redirect URL into this form promptly.",
                );
                if kit::sheet_button(
                    ui,
                    "Sign in through Kite",
                    false,
                    !form.api_key.trim().is_empty(),
                )
                .clicked()
                {
                    let url = format!(
                        "https://kite.zerodha.com/connect/login?v=3&api_key={}",
                        crate::model::webhook::policy::url_encode(form.api_key.trim())
                    );
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
                ui.add_space(12.0);
                kit::field(
                    ui,
                    "API secret",
                    Some("Used once for token exchange; never saved."),
                    "",
                    &mut form.api_secret,
                    true,
                );
                ui.add_space(12.0);
                kit::field(
                    ui,
                    "Request token",
                    Some("Paste the request_token value only."),
                    "",
                    &mut form.request_token,
                    true,
                );
                ui.add_space(12.0);
                let can_exchange = !form.api_key.trim().is_empty()
                    && !form.api_secret.trim().is_empty()
                    && !form.request_token.trim().is_empty();
                if kit::sheet_button(ui, "Create daily session", false, can_exchange).clicked() {
                    form.exchange(ui.ctx());
                }
                ui.add_space(8.0);
                link(
                    ui,
                    "Kite Connect developer console",
                    "https://developers.kite.trade/",
                );
                link(
                    ui,
                    "Authentication instructions",
                    "https://kite.trade/docs/connect/v3/user/",
                );
            });
        });
    });
    if busy {
        ui.horizontal(|ui| {
            ui.add(kit::loading::LoadingIndicator::new().size(14.0));
            ui.label(kit::text(
                "Working…",
                kit::size::BODY,
                kit::REGULAR,
                theme::text_secondary(),
            ));
        });
    }
    if let Some(status) = &form.status {
        status_message(ui, &status.text, status.is_error);
    }
    if !form.is_busy() && form.autosave.due(ui.ctx()) && form.can_save() {
        form.save(ui.ctx());
    }
    kit::notice_card(
        ui,
        Icon::InfoCircle,
        theme::text_secondary(),
        "Your personal market-data session",
        Some(
            "Search Kite instruments by exchange and symbol, such as NSE:INFY or BSE:INFY. The API secret is used only to create the daily access token. Credentials stay in this device's secure storage.",
        ),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait(form: &mut KiteForm) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while form.is_busy() && Instant::now() < deadline {
            form.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!form.is_busy());
    }

    #[test]
    fn incomplete_edits_cannot_save_and_background_errors_are_shown() {
        let mut form = KiteForm {
            api_key: "key".into(),
            ..Default::default()
        };
        assert!(!form.can_save());
        form.access_token = "token".into();
        assert!(form.can_save());
        form.start(None, || {
            Outcome::Tested(Err("Session expired; sign in again.".into()))
        });
        wait(&mut form);
        assert_eq!(
            form.status,
            Some(StatusMessage::error("Session expired; sign in again."))
        );
    }

    #[test]
    fn a_failed_secure_save_is_reported_as_session_only() {
        let mut form = KiteForm::default();
        form.start(None, || {
            Outcome::Saved(
                KiteCredentials {
                    api_key: "key".into(),
                    access_token: "token".into(),
                },
                Err("storage locked".into()),
            )
        });
        wait(&mut form);
        assert!(form.configured);
        assert!(form.status.as_ref().unwrap().is_error);
        assert!(
            form.status
                .as_ref()
                .unwrap()
                .text
                .contains("Active for this session")
        );
        assert_eq!(form.access_token, "token");
    }

    #[test]
    fn only_one_operation_runs_and_removal_clears_transient_secrets() {
        let mut form = KiteForm {
            api_secret: "secret".into(),
            request_token: "request".into(),
            ..Default::default()
        };
        form.start(None, || Outcome::Removed(Ok(())));
        form.start(None, || panic!("a second operation must not run"));
        wait(&mut form);
        assert!(!form.configured);
        assert!(form.api_secret.is_empty() && form.request_token.is_empty());
    }

    #[test]
    #[ignore = "writes images for a person to inspect"]
    fn gallery_of_kite_settings() {
        use crate::ui::render_tests::{Screen, gallery_dir};
        use crate::ui::theme::AppTheme;
        if gallery_dir().is_none() {
            return;
        }
        let _theme = crate::ui::theme::RestoreDefaultTheme;
        for (name, app_theme) in [("dark", AppTheme::Dark), ("light", AppTheme::Light)] {
            for width in [296.0, 616.0] {
                let mut screen = Screen::new(egui::vec2(width, 1000.0));
                theme::apply(&screen.ctx, app_theme);
                let mut form = KiteForm {
                    loaded: true,
                    ..Default::default()
                };
                let output = screen.run(Vec::new(), |ui| {
                    ui.set_width(width);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| body(ui, &mut form));
                });
                screen.save(&format!("kite-settings-{name}-{width}"), output);
            }
        }
    }
}
