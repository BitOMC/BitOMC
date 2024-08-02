use {
  super::*,
  base64::{self, Engine},
  bitcoin::bip32::{ChildNumber, DerivationPath, ExtendedPrivKey, Fingerprint},
  bitcoin::secp256k1::{All, Secp256k1},
  bitcoincore_rpc::bitcoincore_rpc_json::{Descriptor, ImportDescriptors, Timestamp},
  miniscript::descriptor::{DescriptorSecretKey, DescriptorXKey, Wildcard},
  reqwest::header,
};

pub mod wallet_constructor;

pub(crate) struct Wallet {
  bitcoin_client: Client,
  rpc_url: Url,
  utxos: BTreeMap<OutPoint, TxOut>,
  ord_client: reqwest::blocking::Client,
  output_info: BTreeMap<OutPoint, api::Output>,
  locked_utxos: BTreeMap<OutPoint, TxOut>,
  settings: Settings,
}

impl Wallet {
  pub(crate) fn bitcoin_client(&self) -> &Client {
    &self.bitcoin_client
  }

  pub(crate) fn utxos(&self) -> &BTreeMap<OutPoint, TxOut> {
    &self.utxos
  }

  pub(crate) fn locked_utxos(&self) -> &BTreeMap<OutPoint, TxOut> {
    &self.locked_utxos
  }

  pub(crate) fn lock_non_cardinal_outputs(&self) -> Result {
    let locked = self
      .locked_utxos()
      .keys()
      .cloned()
      .collect::<HashSet<OutPoint>>();

    let outputs = self
      .get_runic_outputs()?
      .into_iter()
      .filter(|utxo| !locked.contains(utxo))
      .collect::<Vec<OutPoint>>();

    if !self.bitcoin_client().lock_unspent(&outputs)? {
      bail!("failed to lock UTXOs");
    }

    Ok(())
  }

  pub(crate) fn get_runic_outputs(&self) -> Result<BTreeSet<OutPoint>> {
    let mut runic_outputs = BTreeSet::new();
    for (output, info) in self.output_info.iter() {
      if !info.runes.is_empty() {
        runic_outputs.insert(*output);
      }
    }

    Ok(runic_outputs)
  }

  pub(crate) fn get_runes_balances_in_output(
    &self,
    output: &OutPoint,
  ) -> Result<BTreeMap<SpacedRune, Pile>> {
    Ok(
      self
        .output_info
        .get(output)
        .ok_or(anyhow!("output not found in wallet"))?
        .runes
        .clone(),
    )
  }

  pub(crate) fn get_rune(
    &self,
    rune: Rune,
  ) -> Result<Option<(RuneId, RuneEntry, Option<InscriptionId>)>> {
    let response = self
      .ord_client
      .get(
        self
          .rpc_url
          .join(&format!("/rune/{}", SpacedRune { rune, spacers: 0 }))
          .unwrap(),
      )
      .send()?;

    if !response.status().is_success() {
      return Ok(None);
    }

    let rune_json: api::Rune = serde_json::from_str(&response.text()?)?;

    Ok(Some((rune_json.id, rune_json.entry, rune_json.parent)))
  }

  pub(crate) fn get_last_conversion_outpoint(&self) -> Result<(OutPoint, u64)> {
    let response = self
      .ord_client
      .get(self.rpc_url.join("/status").unwrap())
      .send()?;

    if !response.status().is_success() {
      return Ok((OutPoint::null(), 0));
    }

    let status_json: api::Status = serde_json::from_str(&response.text()?)?;

    Ok(status_json.last_conversion_outpoint)
  }

  pub(crate) fn get_last_mint_outpoint(&self) -> Result<(OutPoint, u64)> {
    let response = self
      .ord_client
      .get(self.rpc_url.join("/status").unwrap())
      .send()?;

    if !response.status().is_success() {
      return Ok((OutPoint::null(), 0));
    }

    let status_json: api::Status = serde_json::from_str(&response.text()?)?;

    Ok(status_json.last_mint_outpoint)
  }

  pub(crate) fn get_util_state(&self) -> Result<api::UtilState> {
    let response = self
      .ord_client
      .get(self.rpc_url.join("/util").unwrap())
      .header(reqwest::header::ACCEPT, "application/json")
      .send()?;

    if !response.status().is_success() {
      bail!("wallet failed to fetch util state: {}", response.text()?);
    }
    Ok(serde_json::from_str::<api::UtilState>(&response.text()?)?)
  }

  pub(crate) fn simulate(&self, transactions: &Vec<Transaction>) -> Result<Vec<api::SupplyState>> {
    let response = self
      .ord_client
      .post(self.rpc_url.join("/simulate")?)
      .json(transactions)
      .header(reqwest::header::ACCEPT, "application/json")
      .send()
      .map_err(|err| anyhow!(err))?;

    if !response.status().is_success() {
      bail!(
        "wallet failed to simulate transactions: {}",
        response.text()?
      );
    }

    Ok(serde_json::from_str::<Vec<api::SupplyState>>(
      &response.text()?,
    )?)
  }

  pub(crate) fn get_change_address(&self) -> Result<Address> {
    Ok(
      self
        .bitcoin_client
        .call::<Address<NetworkUnchecked>>("getrawchangeaddress", &["bech32m".into()])
        .context("could not get change addresses from wallet")?
        .require_network(self.chain().network())?,
    )
  }

  pub(crate) fn chain(&self) -> Chain {
    self.settings.chain()
  }

  fn check_descriptors(wallet_name: &str, descriptors: Vec<Descriptor>) -> Result<Vec<Descriptor>> {
    let tr = descriptors
      .iter()
      .filter(|descriptor| descriptor.desc.starts_with("tr("))
      .count();

    let rawtr = descriptors
      .iter()
      .filter(|descriptor| descriptor.desc.starts_with("rawtr("))
      .count();

    if tr != 2 || descriptors.len() != 2 + rawtr {
      bail!("wallet \"{}\" contains unexpected output descriptors, and does not appear to be an `bitomc` wallet, create a new wallet with `bitomc wallet create`", wallet_name);
    }

    Ok(descriptors)
  }

  pub(crate) fn initialize_from_descriptors(
    name: String,
    settings: &Settings,
    descriptors: Vec<Descriptor>,
  ) -> Result {
    let client = Self::check_version(settings.bitcoin_rpc_client(Some(name.clone()))?)?;

    let descriptors = Self::check_descriptors(&name, descriptors)?;

    client.create_wallet(&name, None, Some(true), None, None)?;

    let descriptors = descriptors
      .into_iter()
      .map(|descriptor| ImportDescriptors {
        descriptor: descriptor.desc.clone(),
        timestamp: descriptor.timestamp,
        active: Some(true),
        range: descriptor.range.map(|(start, end)| {
          (
            usize::try_from(start).unwrap_or(0),
            usize::try_from(end).unwrap_or(0),
          )
        }),
        next_index: descriptor
          .next
          .map(|next| usize::try_from(next).unwrap_or(0)),
        internal: descriptor.internal,
        label: None,
      })
      .collect::<Vec<ImportDescriptors>>();

    client.import_descriptors(descriptors)?;

    Ok(())
  }

  pub(crate) fn initialize(name: String, settings: &Settings, seed: [u8; 64]) -> Result {
    Self::check_version(settings.bitcoin_rpc_client(None)?)?.create_wallet(
      &name,
      None,
      Some(true),
      None,
      None,
    )?;

    let network = settings.chain().network();

    let secp = Secp256k1::new();

    let master_private_key = ExtendedPrivKey::new_master(network, &seed)?;

    let fingerprint = master_private_key.fingerprint(&secp);

    let derivation_path = DerivationPath::master()
      .child(ChildNumber::Hardened { index: 86 })
      .child(ChildNumber::Hardened {
        index: u32::from(network != Network::Bitcoin),
      })
      .child(ChildNumber::Hardened { index: 0 });

    let derived_private_key = master_private_key.derive_priv(&secp, &derivation_path)?;

    for change in [false, true] {
      Self::derive_and_import_descriptor(
        name.clone(),
        settings,
        &secp,
        (fingerprint, derivation_path.clone()),
        derived_private_key,
        change,
      )?;
    }

    Ok(())
  }

  fn derive_and_import_descriptor(
    name: String,
    settings: &Settings,
    secp: &Secp256k1<All>,
    origin: (Fingerprint, DerivationPath),
    derived_private_key: ExtendedPrivKey,
    change: bool,
  ) -> Result {
    let secret_key = DescriptorSecretKey::XPrv(DescriptorXKey {
      origin: Some(origin),
      xkey: derived_private_key,
      derivation_path: DerivationPath::master().child(ChildNumber::Normal {
        index: change.into(),
      }),
      wildcard: Wildcard::Unhardened,
    });

    let public_key = secret_key.to_public(secp)?;

    let mut key_map = HashMap::new();
    key_map.insert(public_key.clone(), secret_key);

    let descriptor = miniscript::descriptor::Descriptor::new_tr(public_key, None)?;

    settings
      .bitcoin_rpc_client(Some(name.clone()))?
      .import_descriptors(vec![ImportDescriptors {
        descriptor: descriptor.to_string_with_secret(&key_map),
        timestamp: Timestamp::Now,
        active: Some(true),
        range: None,
        next_index: None,
        internal: Some(change),
        label: None,
      }])?;

    Ok(())
  }

  pub(crate) fn check_version(client: Client) -> Result<Client> {
    const MIN_VERSION: usize = 240000;

    let bitcoin_version = client.version()?;
    if bitcoin_version < MIN_VERSION {
      bail!(
        "Bitcoin Core {} or newer required, current version is {}",
        Self::format_bitcoin_core_version(MIN_VERSION),
        Self::format_bitcoin_core_version(bitcoin_version),
      );
    } else {
      Ok(client)
    }
  }

  fn format_bitcoin_core_version(version: usize) -> String {
    format!(
      "{}.{}.{}",
      version / 10000,
      version % 10000 / 100,
      version % 100
    )
  }
}
