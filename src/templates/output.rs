use super::*;

#[derive(Boilerplate)]
pub(crate) struct OutputHtml {
  pub(crate) chain: Chain,
  pub(crate) outpoint: OutPoint,
  pub(crate) output: TxOut,
  pub(crate) runes: BTreeMap<SpacedRune, Pile>,
  pub(crate) sat_ranges: Option<Vec<(u64, u64)>>,
  pub(crate) spent: bool,
}

impl PageContent for OutputHtml {
  fn title(&self) -> String {
    format!("Output {}", self.outpoint)
  }
}

#[cfg(test)]
mod tests {
  use {
    super::*,
    bitcoin::{blockdata::script, PubkeyHash},
  };

  #[test]
  fn unspent_output() {
    assert_regex_match!(
      OutputHtml {
        chain: Chain::Mainnet,
        outpoint: outpoint(1),
        output: TxOut { value: 3, script_pubkey: ScriptBuf::new_p2pkh(&PubkeyHash::all_zeros()), },
        runes: BTreeMap::new(),
        sat_ranges: Some(vec![(0, 1), (1, 3)]),
        spent: false,
      },
      "
        <h1>Output <span class=monospace>1{64}:1</span></h1>
        <dl>
          <dt>value</dt><dd>3</dd>
          <dt>script pubkey</dt><dd class=monospace>OP_DUP OP_HASH160 OP_PUSHBYTES_20 0{40} OP_EQUALVERIFY OP_CHECKSIG</dd>
          <dt>address</dt><dd class=monospace><a href=/address/1111111111111111111114oLvT2>1111111111111111111114oLvT2</a></dd>
          <dt>transaction</dt><dd><a class=monospace href=/tx/1{64}>1{64}</a></dd>
          <dt>spent</dt><dd>false</dd>
        </dl>
        <h2>2 Sat Ranges</h2>
        <ul class=monospace>
          <li><a href=/sat/0 class=mythic>0</a></li>
          <li><a href=/range/1/3 class=common>1–3</a></li>
        </ul>
      "
      .unindent()
    );
  }

  #[test]
  fn spent_output() {
    assert_regex_match!(
      OutputHtml {
        chain: Chain::Mainnet,
        outpoint: outpoint(1),
        output: TxOut {
          value: 1,
          script_pubkey: script::Builder::new().push_int(0).into_script(),
        },
        runes: BTreeMap::new(),
        sat_ranges: None,
        spent: true,
      },
      "
        <h1>Output <span class=monospace>1{64}:1</span></h1>
        <dl>
          <dt>value</dt><dd>1</dd>
          <dt>script pubkey</dt><dd class=monospace>OP_0</dd>
          <dt>transaction</dt><dd><a class=monospace href=/tx/1{64}>1{64}</a></dd>
          <dt>spent</dt><dd>true</dd>
        </dl>
      "
      .unindent()
    );
  }

  #[test]
  fn spent_output_with_ranges() {
    assert_regex_match!(
      OutputHtml {
        chain: Chain::Mainnet,
        outpoint: outpoint(1),
        output: TxOut { value: 3, script_pubkey: ScriptBuf::new_p2pkh(&PubkeyHash::all_zeros()), },
        runes: BTreeMap::new(),
        sat_ranges: Some(vec![(0, 1), (1, 3)]),
        spent: true,
      },
      "
        <h1>Output <span class=monospace>1{64}:1</span></h1>
        <dl>
          <dt>value</dt><dd>3</dd>
          <dt>script pubkey</dt><dd class=monospace>OP_DUP OP_HASH160 OP_PUSHBYTES_20 0{40} OP_EQUALVERIFY OP_CHECKSIG</dd>
          <dt>address</dt><dd class=monospace><a href=/address/1111111111111111111114oLvT2>1111111111111111111114oLvT2</a></dd>
          <dt>transaction</dt><dd><a class=monospace href=/tx/1{64}>1{64}</a></dd>
          <dt>spent</dt><dd>true</dd>
        </dl>
        <h2>2 Sat Ranges</h2>
        <ul class=monospace>
          <li><a href=/sat/0 class=mythic>0</a></li>
          <li><a href=/range/1/3 class=common>1–3</a></li>
        </ul>
      "
      .unindent()
    );
  }

  #[test]
  fn no_list() {
    assert_regex_match!(
      OutputHtml {
        chain: Chain::Mainnet,
        outpoint: outpoint(1),
        output: TxOut { value: 3, script_pubkey: ScriptBuf::new_p2pkh(&PubkeyHash::all_zeros()), },
        runes: BTreeMap::new(),
        sat_ranges: None,
        spent: false,
      }
      .to_string(),
      "
        <h1>Output <span class=monospace>1{64}:1</span></h1>
        <dl>
          <dt>value</dt><dd>3</dd>
          <dt>script pubkey</dt><dd class=monospace>OP_DUP OP_HASH160 OP_PUSHBYTES_20 0{40} OP_EQUALVERIFY OP_CHECKSIG</dd>
          <dt>address</dt><dd class=monospace><a href=/address/1111111111111111111114oLvT2>1111111111111111111114oLvT2</a></dd>
          <dt>transaction</dt><dd><a class=monospace href=/tx/1{64}>1{64}</a></dd>
          <dt>spent</dt><dd>false</dd>
        </dl>
      "
      .unindent()
    );
  }
}
