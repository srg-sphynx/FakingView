use crate::service::market::india::{IndianProvider, test_connection};
use crate::service::work_queue::{Pending, SNAPSHOTS};
use crate::ui::kit::{self, Autosave, Tone, destructive_button, field, open_button};
use crate::ui::settings::card::StatusMessage;
use crate::ui::settings::window::{card, section, status_message};
use eframe::egui;

#[derive(Default)]
pub struct TokenForm {
    entered: String,
    saved: Option<bool>,
    status: Option<StatusMessage>,
    pending: Option<Pending<Update>>,
    autosave: Autosave,
}

#[derive(Default)]
struct Update {
    saved: Option<bool>,
    status: Option<StatusMessage>,
}

impl TokenForm {
    fn start(&mut self, ui: &egui::Ui, job: impl FnOnce() -> Update + Send + 'static) {
        self.status = None;
        self.pending = Some(SNAPSHOTS.start(Some(ui.ctx()), job));
    }

    fn poll(&mut self) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        let Some(update) = pending.poll_or(|| Update {
            status: Some(StatusMessage::error(
                "The access-token operation stopped unexpectedly.",
            )),
            ..Update::default()
        }) else {
            return;
        };
        self.pending = None;
        self.saved = update.saved.or(self.saved).or(Some(false));
        self.status = update.status;
    }
}

pub fn body(ui: &mut egui::Ui, form: &mut TokenForm, provider: IndianProvider) {
    form.poll();
    let token = provider.token();
    if form.saved.is_none() && form.pending.is_none() {
        form.start(ui, move || Update {
            saved: Some(token.is_saved()),
            ..Update::default()
        });
    }
    let busy = form.pending.is_some();
    let (link, url, instructions) = match provider {
        IndianProvider::IndMoney => (
            "Open INDstocks",
            "https://www.indstocks.com/app/api-trading/access-tokens",
            "Generate an access token in INDstocks. Each token is valid for 24 hours.",
        ),
        IndianProvider::Groww => (
            "Open Groww Trading APIs",
            "https://groww.in/trade-api/api-keys",
            "Activate a plan with Live Data and Backtesting access, then generate an access token. Groww’s dashboard access tokens expire daily at 6 AM.",
        ),
    };
    let configured = form.saved.unwrap_or(false);
    let badge = match form.saved {
        None => ("Checking access token", Tone::Neutral),
        Some(false) => ("Access token needed", Tone::Neutral),
        Some(true) => ("Access token configured", Tone::Good),
    };
    kit::status_badge(ui, badge.0, badge.1);
    ui.add_space(18.0);
    section(ui, "API access", None, |ui| {
        card(ui, |ui| {
            ui.add_enabled_ui(!busy, |ui| {
                let label = if configured {
                    "Replace access token"
                } else {
                    "Access token"
                };
                let entry = field(
                    ui,
                    label,
                    Some(crate::platform::credential_stored_hint()),
                    "",
                    &mut form.entered,
                    true,
                );
                form.autosave.watch(&entry, true);
            });

            ui.horizontal(|ui| {
                if busy {
                    ui.add(kit::loading::LoadingIndicator::new().size(14.0));
                } else if kit::sheet_button(
                    ui,
                    "Test Connection",
                    false,
                    configured && form.entered.is_empty(),
                )
                .clicked()
                {
                    form.start(ui, move || Update {
                        status: Some(match test_connection(provider) {
                            Ok(()) => StatusMessage::ok("Received a market quote."),
                            Err(error) => StatusMessage::error(error.to_string()),
                        }),
                        ..Update::default()
                    });
                }
                open_button(ui, link, url);
                if configured && !busy && destructive_button(ui, "Remove Token").clicked() {
                    form.entered.clear();
                    form.autosave.cancel();
                    form.start(ui, move || match token.remove() {
                        Ok(()) => Update {
                            saved: Some(false),
                            status: Some(StatusMessage::ok("Access token removed.")),
                        },
                        Err(error) => Update {
                            saved: Some(false),
                            status: Some(StatusMessage::error(format!(
                                "Could not remove the token from secure storage. {}",
                                error.0
                            ))),
                        },
                    });
                }
            });
        });
        if let Some(status) = &form.status {
            ui.add_space(8.0);
            status_message(ui, &status.text, status.is_error);
        }
    });
    if form.pending.is_none() && form.autosave.due(ui.ctx()) && !form.entered.trim().is_empty() {
        let entered = std::mem::take(&mut form.entered);
        form.start(ui, move || match token.save(&entered) {
            Ok(()) => Update {
                saved: Some(true),
                status: Some(StatusMessage::ok(crate::platform::saved_securely())),
            },
            Err(error) => Update {
                saved: Some(token.is_saved()),
                status: Some(StatusMessage::error(format!(
                    "Could not save the token to secure storage. {}",
                    error.0
                ))),
            },
        });
    }
    ui.label(kit::text(
        instructions,
        kit::size::BODY,
        kit::REGULAR,
        crate::ui::theme::text_secondary(),
    ));
}
