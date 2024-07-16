use super::*;

#[derive(Debug, Parser)]
#[clap(group(
  ArgGroup::new("input")
    .required(true)
    .multiple(true)
    .args(&["delegate", "file"]))
)]
pub(crate) struct Inscribe {
  #[command(flatten)]
  shared: SharedArgs,
  #[arg(
    long,
    help = "Include CBOR in file at <METADATA> as inscription metadata",
    conflicts_with = "json_metadata"
  )]
  pub(crate) cbor_metadata: Option<PathBuf>,
  #[arg(long, help = "Delegate inscription content to <DELEGATE>.")]
  pub(crate) delegate: Option<InscriptionId>,
  #[arg(long, help = "Send inscription to <DESTINATION>.")]
  pub(crate) destination: Option<Address<NetworkUnchecked>>,
  #[arg(
    long,
    help = "Inscribe sat with contents of <FILE>. May be omitted if `--delegate` is supplied."
  )]
  pub(crate) file: Option<PathBuf>,
  #[arg(
    long,
    help = "Include JSON in file at <METADATA> converted to CBOR as inscription metadata",
    conflicts_with = "cbor_metadata"
  )]
  pub(crate) json_metadata: Option<PathBuf>,
  #[clap(long, help = "Set inscription metaprotocol to <METAPROTOCOL>.")]
  pub(crate) metaprotocol: Option<String>,
  #[clap(long, help = "Make inscription a child of <PARENT>.")]
  pub(crate) parent: Option<InscriptionId>,
  #[arg(
    long,
    help = "Include <AMOUNT> postage with inscription. [default: 10000sat]"
  )]
  pub(crate) postage: Option<Amount>,
  #[clap(long, help = "Allow reinscription.")]
  pub(crate) reinscribe: bool,
  #[arg(long, help = "Inscribe <SAT>.", conflicts_with = "satpoint")]
  pub(crate) sat: Option<Sat>,
  #[arg(long, help = "Inscribe <SATPOINT>.", conflicts_with = "sat")]
  pub(crate) satpoint: Option<SatPoint>,
}

#[allow(dead_code)]
impl Inscribe {
  pub(crate) fn run(self, wallet: Wallet) -> SubcommandResult {
    let chain = wallet.chain();

    if let Some(delegate) = self.delegate {
      ensure! {
        wallet.inscription_exists(delegate)?,
        "delegate {delegate} does not exist"
      }
    }

    batch::Plan {
      commit_fee_rate: self.shared.commit_fee_rate.unwrap_or(self.shared.fee_rate),
      destinations: vec![match self.destination.clone() {
        Some(destination) => destination.require_network(chain.network())?,
        None => wallet.get_change_address()?,
      }],
      dry_run: self.shared.dry_run,
      etching: None,
      inscriptions: vec![Inscription::new(
        chain,
        self.shared.compress,
        self.delegate,
        Inscribe::parse_metadata(self.cbor_metadata, self.json_metadata)?,
        self.metaprotocol,
        self.parent.into_iter().collect(),
        self.file,
        None,
        None,
      )?],
      mode: batch::Mode::SeparateOutputs,
      no_backup: self.shared.no_backup,
      no_limit: self.shared.no_limit,
      parent_info: wallet.get_parent_info(self.parent)?,
      postages: vec![self.postage.unwrap_or(TARGET_POSTAGE)],
      reinscribe: self.reinscribe,
      reveal_fee_rate: self.shared.fee_rate,
      reveal_satpoints: Vec::new(),
      satpoint: if let Some(sat) = self.sat {
        Some(wallet.find_sat_in_outputs(sat)?)
      } else {
        self.satpoint
      },
    }
    .inscribe(
      &wallet.locked_utxos().clone().into_keys().collect(),
      wallet.get_runic_outputs()?,
      wallet.utxos(),
      &wallet,
    )
  }

  fn parse_metadata(cbor: Option<PathBuf>, json: Option<PathBuf>) -> Result<Option<Vec<u8>>> {
    if let Some(path) = cbor {
      let cbor = fs::read(path)?;
      let _value: Value = ciborium::from_reader(Cursor::new(cbor.clone()))
        .context("failed to parse CBOR metadata")?;

      Ok(Some(cbor))
    } else if let Some(path) = json {
      let value: serde_json::Value =
        serde_json::from_reader(fs::File::open(path)?).context("failed to parse JSON metadata")?;
      let mut cbor = Vec::new();
      ciborium::into_writer(&value, &mut cbor)?;

      Ok(Some(cbor))
    } else {
      Ok(None)
    }
  }
}