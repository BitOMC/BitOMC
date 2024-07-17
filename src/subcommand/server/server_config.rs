use super::*;

#[derive(Default)]
pub(crate) struct ServerConfig {
  pub(crate) chain: Chain,
  pub(crate) domain: Option<String>,
  pub(crate) index_sats: bool,
  pub(crate) json_api_enabled: bool,
}
