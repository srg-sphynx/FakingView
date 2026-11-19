use super::api_keys::{KeyBackend, default_backend};
use super::cached_secret::CachedSecret;
use super::webhook_secrets::Scope;
use crate::service::webhook::delivery::SecretStoreError;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum MarketToken {
    IndMoney,
    Groww,
}

impl MarketToken {
    fn secret(self) -> &'static CachedSecret {
        static IND_MONEY: OnceLock<CachedSecret> = OnceLock::new();
        static GROWW: OnceLock<CachedSecret> = OnceLock::new();
        let (slot, service) = match self {
            Self::IndMoney => (&IND_MONEY, "local.fakingview.desktop.indmoney"),
            Self::Groww => (&GROWW, "local.fakingview.desktop.groww"),
        };
        slot.get_or_init(|| {
            CachedSecret::new(
                Scope {
                    service,
                    shared_group: false,
                },
                "access-token-v1",
            )
        })
    }

    pub fn is_saved(self) -> bool {
        self.secret().is_saved(backend())
    }

    pub(crate) fn read(self) -> Option<String> {
        self.secret().read(backend())
    }

    pub fn save(self, token: &str) -> Result<(), SecretStoreError> {
        let token = token.trim();
        if token.is_empty() || token.len() > 4096 || !token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(SecretStoreError(
                "Enter an access token without spaces or line breaks".into(),
            ));
        }
        self.secret().save(backend(), token)
    }

    pub fn remove(self) -> Result<(), SecretStoreError> {
        self.secret().remove(backend())
    }
}

fn backend() -> &'static dyn KeyBackend {
    static BACKEND: OnceLock<Box<dyn KeyBackend>> = OnceLock::new();
    BACKEND.get_or_init(default_backend).as_ref()
}
