Wallet
======

Tighten and Ease can be held in a Bitcoin wallet and
transferred using Bitcoin transactions. Utils are a floating denomination
of sats and can be transferred by transferring the equivalent number of sats.

BitOMC requires a Bitcoin node to give you a view of the current state of the
Bitcoin blockchain, and a wallet that can transfer utils, mint and convert Tighten 
and Ease, and perform sat control when constructing transactions to send them to
another wallet.

Bitcoin Core provides both a Bitcoin node and wallet. However, the Bitcoin
Core wallet cannot mint or convert Tighten and Ease and does not perform sat control.

The utility [`bitomc`](https://github.com/BitOMC/BitOMC) doesn't implement its own 
wallet, so `bitomc wallet` subcommands interact with Bitcoin Core wallets.

This guide covers:

1. Installing Bitcoin Core
2. Syncing the Bitcoin blockchain
3. Creating a Bitcoin Core wallet
4. Using `bitomc wallet receive` to receive sats
5. Minting Tighten and Ease with `bitomc wallet mint`
5. Converting Tighten and Ease with `bitomc wallet convert-exact-input` and `bitomc wallet convert-exact-output`
6. Sending sats, utils, and Tighten and Ease with `bitomc wallet send`
7. Receiving Tighten and Ease with `bitomc wallet receive`

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

`bitomc` requires Bitcoin Core 24 or newer.

This guide does not cover installing Bitcoin Core in detail. Once Bitcoin Core
is installed, you should be able to run `bitcoind -version` successfully from
the command line. Do *NOT* use `bitcoin-qt`.

Configuring Bitcoin Core
------------------------

`bitomc` requires Bitcoin Core's rest interface and block data. The `bitomc`
explorer also requires the transaction index, but the explorer is optional. 

To configure your Bitcoin Core node to maintain a transaction
index, add the following to your `bitcoin.conf`:

```
txindex=1
```

Or, run `bitcoind` with `-txindex`:

```
bitcoind -txindex
```

`bitomc` can be run on a Bitcoin Core node pruned below block 854000. Subsequent
blocks may be manually pruned after being indexed by `bitomc`, but this will 
prevent `bitomc` from re-indexing.

Details on creating or modifying your `bitcoin.conf` file can be found
[here](https://github.com/bitcoin/bitcoin/blob/master/doc/bitcoin-conf.md).

Syncing the Bitcoin Blockchain
------------------------------

To sync the chain, run:

```
bitcoind
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

If you have `maxuploadtarget` set it can interfere with fetching blocks for
`bitomc` index. Either remove it or set `whitebind=127.0.0.1:8333`.

Installing `bitomc`
----------------

The `bitomc` utility is written in Rust and can be built from
[source](https://github.com/BitOMC/BitOMC). Pre-built binaries are available on the
[releases page](https://github.com/BitOMC/BitOMC/releases).

You can install the latest pre-built binary from the command line with:

```sh
curl --proto '=https' --tlsv1.2 -fsLS https://bitomc.org/install.sh | bash -s
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
requires `bitomc server` running in the background. Make sure these
programs are running:

```
bitcoind
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

Minting Tighten and Ease
---------------------

To mint Tighten and Ease, run:

```
bitomc wallet mint --fee-rate <FEE_RATE>
```

BitOMC will output the transaction ID, the amount of Tighten and Ease received,
and a `connected` boolean, which will be `true` if the transaction spends the
output left by the previous mint transaction. If so, the transaction will be
added to the mempool if and only if it is able to RBF the existing candidate mint
transaction (or if it is the first mint transaction seen). If `false`, the 
transaction will only mint Tighten and Ease if it is the first transaction in the block.

You can check the status of the transactions using [the mempool.space block
explorer](https://mempool.space/).

Once the transaction has been mined, you can confirm receipt by running:

```
bitomc wallet balance
```

Converting Tighten and Ease
---------------------

To convert between Tighten and Ease using an exact input amount, run:

```
bitomc wallet convert-exact-input --fee-rate <INPUT_AMOUNT> <MIN_OUTPUT_AMOUNT>
```

Where `INPUT_AMOUNT` is the number of runes to convert, a `:` character, and the
name of the input rune, and `MIN_OUTPUT_AMOUNT` is the minimum number of runes
you wish to receive, a `:` character, and the name of the output rune.


For example, if you want to convert 1000 TIGHTEN and receive at least 500 EASE, you
would use `1000:TIGHTEN` and `500:EASE`.

```
bitomc wallet convert-exact-input --fee-rate 1 1000:TIGHTEN 500:EASE
```

Alternatively, if you want to convert 1000 TIGHTEN at the latest conversion rate, you
would use `1000:TIGHTEN` and `0:EASE`.

```
bitomc wallet convert-exact-input --fee-rate 1 1000:TIGHTEN 0:EASE
```

To convert between Tighten and Ease using an exact output amount, run:

```
bitomc wallet convert-exact-input --fee-rate <OUTPUT_AMOUNT> <MAX_INPUT_AMOUNT>
```

BitOMC will output the transaction ID, the expected amount of Tighten and Ease received,
and a `connected` boolean, which will be `true` if and only if the transaction spends the
output left by the preceding conversion transaction.

You can check the status of the transactions using [the mempool.space block
explorer](https://mempool.space/).

Once the transaction has been mined, you can confirm receipt by running:

```
bitomc wallet balance
```

Sending Sats
--------------------

Send sats by running:

```
bitomc wallet send --fee-rate <FEE_RATE> <ADDRESS> <SAT_AMOUNT>
```

Where `SAT_AMOUNT` is the amount of sats to send, an optional space, and the
denomination (`bit|btc|cbtc|mbtc|msat|nbtc|pbtc|sat|satoshi|ubtc`). For example if you
want to send 1000 sats, you would use `1000 sats`.

```
bitomc wallet send --fee-rate 1 SOME_ADDRESS 1000 sats
```

See the pending transaction with:

```
bitomc wallet transactions
```

Once the send transaction confirms, the recipient can confirm receipt by
running:

```
bitomc wallet balance
```

Sending Utils
--------------------

Send utils by running:

```
bitomc wallet send --fee-rate <FEE_RATE> <ADDRESS> <UTIL_AMOUNT>
```

Where `UTIL_AMOUNT` is the amount of sats to send, an optional space, and the denomination
`util` or `utils`. For example if you want to send 1000 utils, you would use `1000 utils`.

```
bitomc wallet send --fee-rate 1 SOME_ADDRESS 1000 utils
```

See the pending transaction with:

```
bitomc wallet transactions
```

Once the send transaction confirms, the recipient can confirm receipt by
running:

```
bitomc wallet balance
```

Sending Tighten and Ease
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
name of the rune. For example if you want to send 1000 of TIGHTEN, you
would use `1000:TIGHTEN`.

```
bitomc wallet send --fee-rate 1 SOME_ADDRESS 1000:EXAMPLE
```

Likewise, if you want to send 1000 of EASE, you
would use `1000:EASE`.

```
bitomc wallet send --fee-rate 1 SOME_ADDRESS 1000:EASE
```

See the pending transaction with:

```
bitomc wallet transactions
```

Once the send transaction confirms, the recipient can confirm receipt with:

```
bitomc wallet balance
```

Receiving Runes
----------------------

Generate a new receive address using:

```
bitomc wallet receive
```

The sender can transfer the rune to your address using:

```
bitomc wallet send --fee-rate <FEE_RATE> <ADDRESS> <RUNES_AMOUNT>
```

See the pending transaction with:
```
bitomc wallet transactions
```

Once the send transaction confirms, you can confirm receipt by running:

```
bitomc wallet balance
```
