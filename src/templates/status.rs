use super::*;

#[derive(Boilerplate, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusHtml {
  pub address_index: bool,
  pub chain: Chain,
  pub height: Option<u32>,
  pub initial_sync_time: Duration,
  pub inscriptions: u64,
  pub lost_sats: u64,
  pub rune_index: bool,
  pub runes: u64,
  pub sat_index: bool,
  pub started: DateTime<Utc>,
  pub transaction_index: bool,
  pub unrecoverably_reorged: bool,
  pub uptime: Duration,
  pub last_mint_outpoint: OutPoint,
  pub last_conversion_outpoint: OutPoint,
}

impl PageContent for StatusHtml {
  fn title(&self) -> String {
    "Status".into()
  }
}
