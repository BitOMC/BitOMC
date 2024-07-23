Wallet
======

Individual sats can be inscribed with arbitrary content, creating
Bitcoin-native digital artifacts that can be held in a Bitcoin wallet and
transferred using Bitcoin transactions. Inscriptions are as durable, immutable,
secure, and decentralized as Bitcoin itself.

Working with inscriptions requires a Bitcoin full node, to give you a view of
the current state of the Bitcoin blockchain, and a wallet that can create
inscriptions and perform sat control when constructing transactions to send
inscriptions to another wallet.

Bitcoin Core provides both a Bitcoin full node and wallet. However, the Bitcoin
Core wallet cannot create inscriptions and does not perform sat control.

This requires [`bitomc`](https://github.com/BitOMC/BitOMC), the ordinal utility. `bitomc`
doesn't implement its own wallet, so `bitomc wallet` subcommands interact with
Bitcoin Core wallets.

This guide covers:

1. Installing Bitcoin Core
2. Syncing the Bitcoin blockchain
3. Creating a Bitcoin Core wallet
4. Using `bitomc wallet receive` to receive sats
5. Creating inscriptions with `bitomc wallet inscribe`
6. Sending inscriptions with `bitomc wallet send`
7. Receiving inscriptions with `bitomc wallet receive`
8. Batch inscribing with `bitomc wallet inscribe --batch`

Getting Help
------------

If you get stuck, try asking for help on the [Ordinals Discord
Server](https://discord.com/invite/87cjuz4FYg), or checking GitHub for relevant
[issues](https://github.com/BitOMC/BitOMC/issues) and
[discussions](https://github.com/BitOMC/BitOMC/discussions).

Installing Bitcoin Core
-----------------------

Bitcoin Core is available from [bitcoincore.org](https://bitcoincore.org/) on
the [download page](https://bitcoincore.org/en/download/).

Making inscriptions requires Bitcoin Core 24 or newer.

This guide does not cover installing Bitcoin Core in detail. Once Bitcoin Core
is installed, you should be able to run `bitcoind -version` successfully from
the command line. Do *NOT* use `bitcoin-qt`.

Configuring Bitcoin Core
------------------------

`bitomc` requires Bitcoin Core's transaction index and rest interface.

To configure your Bitcoin Core node to maintain a transaction
index, add the following to your `bitcoin.conf`:

```
txindex=1
```

Or, run `bitcoind` with `-txindex`:

```
bitcoind -txindex
```

Details on creating or modifying your `bitcoin.conf` file can be found
[here](https://github.com/bitcoin/bitcoin/blob/master/doc/bitcoin-conf.md).

Syncing the Bitcoin Blockchain
------------------------------

To sync the chain, run:

```
bitcoind -txindex
```

…and leave it running until `getblockcount`:

```
bitcoin-cli getblockcount
```

agrees with the block count on a block explorer like [the mempool.space block
explorer](https://mempool.space/). `bitomc` interacts with `bitcoind`, so you
should leave `bitcoind` running in the background when you're using `bitomc`.

The blockchain takes about 600GB of disk space. If you have an external drive
you want to store blocks on, use the configuration option
`blocksdir=<external_drive_path>`. This is much simpler than using the
`datadir` option because the cookie file will still be in the default location
for `bitcoin-cli` and `bitomc` to find.

Troubleshooting
---------------

Make sure you can access `bitcoind` with `bitcoin-cli -getinfo` and that it is
fully synced.

If `bitcoin-cli -getinfo` returns `Could not connect to the server`, `bitcoind`
is not running.

Make sure `rpcuser`, `rpcpassword`, or `rpcauth` are *NOT* set in your
`bitcoin.conf` file. `bitomc` requires using cookie authentication. Make sure there
is a file `.cookie` in your bitcoin data directory.

If `bitcoin-cli -getinfo` returns `Could not locate RPC credentials`, then you
must specify the cookie file location.
If you are using a custom data directory (specifying the `datadir` option),
then you must specify the cookie location like
`bitcoin-cli -rpccookiefile=<your_bitcoin_datadir>/.cookie -getinfo`.
When running `bitomc` you must specify the cookie file location with
`--cookie-file=<your_bitcoin_datadir>/.cookie`.

Make sure you do *NOT* have `disablewallet=1` in your `bitcoin.conf` file. If
`bitcoin-cli listwallets` returns `Method not found` then the wallet is disabled
and you won't be able to use `bitomc`.

Make sure `txindex=1` is set. Run `bitcoin-cli getindexinfo` and it should
return something like
```json
{
  "txindex": {
    "synced": true,
    "best_block_height": 776546
  }
}
```
If it only returns `{}`, `txindex` is not set.
If it returns `"synced": false`, `bitcoind` is still creating the `txindex`.
Wait until `"synced": true` before using `bitomc`.

If you have `maxuploadtarget` set it can interfere with fetching blocks for
`bitomc` index. Either remove it or set `whitebind=127.0.0.1:8333`.

Installing `bitomc`
----------------

The `bitomc` utility is written in Rust and can be built from
[source](https://github.com/BitOMC/BitOMC). Pre-built binaries are available on the
[releases page](https://github.com/BitOMC/BitOMC/releases).

You can install the latest pre-built binary from the command line with:

```sh
curl --proto '=https' --tlsv1.2 -fsLS https://ordinals.com/install.sh | bash -s
```

Once `bitomc` is installed, you should be able to run:

```
bitomc --version
```

Which prints out `bitomc`'s version number.

Creating a Wallet
-----------------

`bitomc` uses `bitcoind` to manage private keys, sign transactions, and
broadcast transactions to the Bitcoin network. Additionally the `bitomc wallet`
requires [`bitomc server`](explorer.md) running in the background. Make sure these
programs are running:

```
bitcoind -txindex
```

```
bitomc server
```

To create a wallet named `bitomc`, the default, for use with `bitomc wallet`, run:

```
bitomc wallet create
```

This will print out your seed phrase mnemonic, store it somewhere safe.

```
{
  "mnemonic": "dignity buddy actor toast talk crisp city annual tourist orient similar federal",
  "passphrase": ""
}
```

If you want to specify a different name or use an `bitomc server` running on a
non-default URL you can set these options:

```
bitomc wallet --name foo --server-url http://127.0.0.1:8080 create
```

To see all available wallet options you can run:

```
bitomc wallet help
```

Restoring and Dumping Wallet
----------------------------

The `bitomc` wallet uses descriptors, so you can export the output descriptors and
import them into another descriptor-based wallet. To export the wallet
descriptors, which include your private keys:

```
$ bitomc wallet dump
==========================================
= THIS STRING CONTAINS YOUR PRIVATE KEYS =
=        DO NOT SHARE WITH ANYONE        =
==========================================
{
  "wallet_name": "bitomc",
  "descriptors": [
    {
      "desc": "tr([551ac972/86'/1'/0']tprv8h4xBhrfZwX9o1XtUMmz92yNiGRYjF9B1vkvQ858aN1UQcACZNqN9nFzj3vrYPa4jdPMfw4ooMuNBfR4gcYm7LmhKZNTaF4etbN29Tj7UcH/0/*)#uxn94yt5",
      "timestamp": 1296688602,
      "active": true,
      "internal": false,
      "range": [
        0,
        999
      ],
      "next": 0
    },
    {
      "desc": "tr([551ac972/86'/1'/0']tprv8h4xBhrfZwX9o1XtUMmz92yNiGRYjF9B1vkvQ858aN1UQcACZNqN9nFzj3vrYPa4jdPMfw4ooMuNBfR4gcYm7LmhKZNTaF4etbN29Tj7UcH/1/*)#djkyg3mv",
      "timestamp": 1296688602,
      "active": true,
      "internal": true,
      "range": [
        0,
        999
      ],
      "next": 0
    }
  ]
}
```

An `bitomc` wallet can be restored from a mnemonic:

```
bitomc wallet restore --from mnemonic
```

Type your mnemonic and press return.

To restore from a descriptor in `descriptor.json`:

```
cat descriptor.json | bitomc wallet restore --from descriptor
```

To restore from a descriptor in the clipboard:

```
bitomc wallet restore --from descriptor
```

Paste the descriptor into the terminal and press CTRL-D on unix and CTRL-Z
on Windows.

Receiving Sats
--------------

Inscriptions are made on individual sats, using normal Bitcoin transactions
that pay fees in sats, so your wallet will need some sats.

Get a new address from your `bitomc` wallet by running:

```
bitomc wallet receive
```

And send it some funds.

You can see pending transactions with:

```
bitomc wallet transactions
```

Once the transaction confirms, you should be able to see the transactions
outputs with `bitomc wallet outputs`.

Creating Inscription Content
----------------------------

Sats can be inscribed with any kind of content, but the `bitomc` wallet only
supports content types that can be displayed by the `bitomc` block explorer.

Additionally, inscriptions are included in transactions, so the larger the
content, the higher the fee that the inscription transaction must pay.

Inscription content is included in transaction witnesses, which receive the
witness discount. To calculate the approximate fee that an inscribe transaction
will pay, divide the content size by four and multiply by the fee rate.

Inscription transactions must be less than 400,000 weight units, or they will
not be relayed by Bitcoin Core. One byte of inscription content costs one
weight unit. Since an inscription transaction includes not just the inscription
content, limit inscription content to less than 400,000 weight units. 390,000
weight units should be safe.

Creating Inscriptions
---------------------

To create an inscription with the contents of `FILE`, run:

```
bitomc wallet inscribe --fee-rate FEE_RATE --file FILE
```

Ord will output two transactions IDs, one for the commit transaction, and one
for the reveal transaction, and the inscription ID. Inscription IDs are of the
form `TXIDiN`, where `TXID` is the transaction ID of the reveal transaction,
and `N` is the index of the inscription in the reveal transaction.

The commit transaction commits to a tapscript containing the content of the
inscription, and the reveal transaction spends from that tapscript, revealing
the content on chain and inscribing it on the first sat of the input that
contains the corresponding tapscript.

Wait for the reveal transaction to be mined. You can check the status of the
commit and reveal transactions using  [the mempool.space block
explorer](https://mempool.space/).

Once the reveal transaction has been mined, the inscription ID should be
printed when you run:

```
bitomc wallet inscriptions
```

Parent-Child Inscriptions
-------------------------

Parent-child inscriptions enable what is colloquially known as collections, see
[provenance](../inscriptions/provenance.md) for more information.

To make an inscription a child of another, the parent inscription has to be
inscribed and present in the wallet. To choose a parent run `bitomc wallet inscriptions`
and copy the inscription id (`<PARENT_INSCRIPTION_ID>`).

Now inscribe the child inscription and specify the parent like so:

```
bitomc wallet inscribe --fee-rate FEE_RATE --parent <PARENT_INSCRIPTION_ID> --file CHILD_FILE
```

This relationship cannot be added retroactively, the parent has to be
present at inception of the child.

Sending Inscriptions
--------------------

Ask the recipient to generate a new address by running:

```
bitomc wallet receive
```

Send the inscription by running:

```
bitomc wallet send --fee-rate <FEE_RATE> <ADDRESS> <INSCRIPTION_ID>
```

See the pending transaction with:

```
bitomc wallet transactions
```

Once the send transaction confirms, the recipient can confirm receipt by
running:

```
bitomc wallet inscriptions
```

Sending Runes
-------------

Ask the recipient to generate a new address by running:

```
bitomc wallet receive
```

Send the runes by running:

```
bitomc wallet send --fee-rate <FEE_RATE> <ADDRESS> <RUNES_AMOUNT>
```

Where `RUNES_AMOUNT` is the number of runes to send, a `:` character, and the
name of the rune. For example if you want to send 1000 of the EXAMPLE rune, you
would use `1000:EXAMPLE`.

```
bitomc wallet send --fee-rate 1 SOME_ADDRESS 1000:EXAMPLE
```

See the pending transaction with:

```
bitomc wallet transactions
```

Once the send transaction confirms, the recipient can confirm receipt with:

```
bitomc wallet balance
```

Receiving Inscriptions
----------------------

Generate a new receive address using:

```
bitomc wallet receive
```

The sender can transfer the inscription to your address using:

```
bitomc wallet send --fee-rate <FEE_RATE> ADDRESS INSCRIPTION_ID
```

See the pending transaction with:
```
bitomc wallet transactions
```

Once the send transaction confirms, you can confirm receipt by running:

```
bitomc wallet inscriptions
```
