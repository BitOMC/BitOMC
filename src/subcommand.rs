use super::*;

pub mod balances;
pub mod decode;
pub mod env;
pub mod history;
pub mod index;
pub mod runes;
pub(crate) mod server;
mod settings;
pub mod util;
pub mod wallet;

#[derive(Debug, Parser)]
pub(crate) enum Subcommand {
  #[command(about = "List all rune balances")]
  Balances,
  #[command(about = "Decode a transaction")]
  Decode(decode::Decode),
  #[command(about = "Start a regtest bitomc and bitcoind instance")]
  Env(env::Env),
  #[command(subcommand, about = "Index commands")]
  Index(index::IndexSubcommand),
  #[command(about = "Run the explorer server")]
  Server(server::Server),
  #[command(about = "Display settings")]
  Settings,
  #[command(about = "Wallet commands")]
  Wallet(wallet::WalletCommand),
  #[command(about = "Display current monetary policy")]
  MonetaryPolicy,
  #[command(about = "Display recent interest rates")]
  RateHistory,
  #[command(about = "Display utils in terms of sats")]
  UtilToSat(util::UtilToSatInput),
  #[command(about = "Display sats in terms of utils")]
  SatToUtil(util::SatToUtilInput),
}

impl Subcommand {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    match self {
      Self::Balances => balances::run(settings),
      Self::Decode(decode) => decode.run(settings),
      Self::Env(env) => env.run(),
      Self::Index(index) => index.run(settings),
      Self::Server(server) => {
        let index = Arc::new(Index::open(&settings)?);
        let handle = axum_server::Handle::new();
        LISTENERS.lock().unwrap().push(handle.clone());
        server.run(settings, index, handle)
      }
      Self::Settings => settings::run(settings),
      Self::Wallet(wallet) => wallet.run(settings),
      Self::MonetaryPolicy => util::run(settings),
      Self::RateHistory => history::run(settings),
      Self::UtilToSat(util_to_sat) => util_to_sat.run(settings),
      Self::SatToUtil(sat_to_util) => sat_to_util.run(settings),
    }
  }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum OutputFormat {
  #[default]
  Json,
  Yaml,
  Minify,
}

pub trait Output: Send {
  fn print(&self, format: OutputFormat);
}

impl<T> Output for T
where
  T: Serialize + Send,
{
  fn print(&self, format: OutputFormat) {
    match format {
      OutputFormat::Json => serde_json::to_writer_pretty(io::stdout(), self).ok(),
      OutputFormat::Yaml => serde_yaml::to_writer(io::stdout(), self).ok(),
      OutputFormat::Minify => serde_json::to_writer(io::stdout(), self).ok(),
    };
    println!();
  }
}

pub(crate) type SubcommandResult = Result<Option<Box<dyn Output>>>;
