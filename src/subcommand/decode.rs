use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Decode {
  #[arg(
    long,
    conflicts_with = "file",
    help = "Fetch transaction with <TXID> from Bitcoin Core."
  )]
  txid: Option<Txid>,
  #[arg(long, conflicts_with = "txid", help = "Load transaction from <FILE>.")]
  file: Option<PathBuf>,
}

#[derive(Serialize, Eq, PartialEq, Deserialize, Debug)]
pub struct Output {
  pub runestone: Option<Artifact>,
}

impl Decode {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    let transaction = if let Some(txid) = self.txid {
      settings
        .bitcoin_rpc_client(None)?
        .get_raw_transaction(&txid, None)?
    } else if let Some(file) = self.file {
      Transaction::consensus_decode(&mut fs::File::open(file)?)?
    } else {
      Transaction::consensus_decode(&mut io::stdin())?
    };

    let runestone = Runestone::decipher(&transaction);

    Ok(Some(Box::new(Output { runestone })))
  }
}
