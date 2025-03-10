use {
  super::*,
  crate::wallet::{wallet_constructor::WalletConstructor, Wallet},
  bitcoincore_rpc::bitcoincore_rpc_json::ListDescriptorsResult,
};

pub mod balance;
pub mod convert;
pub mod create;
pub mod dump;
pub mod mint;
pub mod outputs;
pub mod receive;
pub mod restore;
pub mod runics;
pub mod send;
mod shared_args;
pub mod transactions;

#[derive(Debug, Parser)]
pub(crate) struct WalletCommand {
  #[arg(long, default_value = "bitomc", help = "Use wallet named <WALLET>.")]
  pub(crate) name: String,
  #[arg(long, alias = "nosync", help = "Do not update index.")]
  pub(crate) no_sync: bool,
  #[arg(
    long,
    help = "Use bitomc running at <SERVER_URL>. [default: http://localhost:80]"
  )]
  pub(crate) server_url: Option<Url>,
  #[command(subcommand)]
  pub(crate) subcommand: Subcommand,
}

#[derive(Debug, Parser)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Subcommand {
  #[command(about = "Get wallet balance")]
  Balance,
  #[command(about = "Convert between tighten and ease using an exact input")]
  ConvertExactInput(convert::ConvertExactInput),
  #[command(about = "Convert between tighten and ease using an exact output")]
  ConvertExactOutput(convert::ConvertExactOutput),
  #[command(about = "Lookup current chain of conversions in mempool")]
  LookupConversionChain,
  #[command(about = "Create new wallet")]
  Create(create::Create),
  #[command(about = "Dump wallet descriptors")]
  Dump,
  #[command(about = "Mint a rune")]
  Mint(mint::Mint),
  #[command(about = "List all unspent outputs in wallet")]
  Outputs,
  #[command(about = "Generate receive address")]
  Receive(receive::Receive),
  #[command(about = "Restore wallet")]
  Restore(restore::Restore),
  #[command(about = "List unspent runic outputs in wallet")]
  Runics,
  #[command(about = "Send sat or inscription")]
  Send(send::Send),
  #[command(about = "See wallet transactions")]
  Transactions(transactions::Transactions),
}

impl WalletCommand {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    match self.subcommand {
      Subcommand::Create(create) => return create.run(self.name, &settings),
      Subcommand::Restore(restore) => return restore.run(self.name, &settings),
      _ => {}
    };

    let wallet = WalletConstructor::construct(
      self.name.clone(),
      self.no_sync,
      settings.clone(),
      self
        .server_url
        .as_ref()
        .map(Url::as_str)
        .or(settings.server_url())
        .unwrap_or("http://127.0.0.1:80")
        .parse::<Url>()
        .context("invalid server URL")?,
    )?;

    match self.subcommand {
      Subcommand::Balance => balance::run(wallet),
      Subcommand::ConvertExactInput(convert) => convert.run(wallet),
      Subcommand::ConvertExactOutput(convert) => convert.run(wallet),
      Subcommand::LookupConversionChain => convert::get_chain(wallet),
      Subcommand::Create(_) | Subcommand::Restore(_) => unreachable!(),
      Subcommand::Dump => dump::run(wallet),
      Subcommand::Mint(mint) => mint.run(wallet),
      Subcommand::Outputs => outputs::run(wallet),
      Subcommand::Receive(receive) => receive.run(wallet),
      Subcommand::Runics => runics::run(wallet),
      Subcommand::Send(send) => send.run(wallet),
      Subcommand::Transactions(transactions) => transactions.run(wallet),
    }
  }
}
