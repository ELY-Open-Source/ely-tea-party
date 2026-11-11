pub mod agents;
pub mod app;
pub mod blur;
pub mod butler;
pub mod communication;
pub mod curl;
#[cfg(target_os = "macos")]
pub mod dock;
#[cfg(windows)]
pub mod explorer;
pub mod files;
pub mod git;
pub mod handoff;
pub mod i18n;
pub mod interface;
pub mod memory;
pub mod menus;
pub mod pet;
pub mod profiles;
pub mod sessions;
pub mod settings;
pub mod skins;
pub mod term;
#[cfg(test)]
mod testlog;
#[cfg(all(test, target_os = "linux"))]
mod testx11;
pub mod themes;
pub mod update;
pub mod usage;
pub mod workspace;
