use gpui::{App, Task};
use security_framework::passwords::{
    PasswordOptions, delete_generic_password, generic_password, set_generic_password,
};

const ITEM_NOT_FOUND: i32 = -25300;
type Entry = Option<(String, Vec<u8>)>;

pub(super) fn read(service: &str, provider: &str, cx: &App) -> Task<anyhow::Result<Entry>> {
    let service = service.to_string();
    let provider = provider.to_string();
    cx.background_executor().spawn(async move {
        match generic_password(PasswordOptions::new_generic_password(&service, &provider)) {
            Ok(bytes) => Ok(Some((provider, bytes))),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
            Err(error) => Err(error.into()),
        }
    })
}

pub(super) fn save(
    service: &str,
    provider: &str,
    key: Option<String>,
    cx: &App,
) -> Task<anyhow::Result<()>> {
    let service = service.to_string();
    let provider = provider.to_string();
    cx.background_executor().spawn(async move {
        match key {
            Some(key) => set_generic_password(&service, &provider, key.as_bytes())?,
            None => match delete_generic_password(&service, &provider) {
                Ok(()) => {}
                Err(error) if error.code() == ITEM_NOT_FOUND => {}
                Err(error) => return Err(error.into()),
            },
        }
        Ok(())
    })
}
