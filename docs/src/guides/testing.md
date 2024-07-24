Testing
=======

Test Environment
----------------

`bitomc env <DIRECTORY>` creates a test environment in `<DIRECTORY>`, spins up
`bitcoind` and `bitomc server` instances, prints example commands for interacting
with the test `bitcoind` and `bitomc server` instances, waits for `CTRL-C`, and
then shuts down `bitcoind` and `bitomc server`.

`bitomc env` tries to use port 9000 for `bitcoind`'s RPC interface, and port
`9001` for `bitomc`'s RPC interface, but will fall back to random unused ports.

Inside of the env directory, `bitomc env` will write `bitcoind`'s configuration to
`bitcoin.conf`, `bitomc`'s configuration to `bitomc.yaml`, and the env configuration
to `env.json`.

`env.json` contains the commands needed to invoke `bitcoin-cli` and `bitomc
wallet`, as well as the ports `bitcoind` and `bitomc server` are listening on.

These can be extracted into shell commands using `jq`:

```shell
bitcoin=`jq -r '.bitcoin_cli_command | join(" ")' env/env.json`
$bitcoin listunspent

bitomc=`jq -r '.ord_wallet_command | join(" ")' env/env.json`
$bitomc outputs
```

If `bitomc` is in the `$PATH` and the env directory is `env`, the `bitcoin-cli`
command will be:

```
bitcoin-cli -datadir=env`
```

And the `bitomc` will be:

```
bitomc --datadir env
```

Test Networks
-------------

BitOMC can be tested using the following flags to specify the test network. For more
information on running Bitcoin Core for testing, see [Bitcoin's developer documentation](https://developer.bitcoin.org/examples/testing.html).

Most `bitomc` commands in [wallet](wallet.md) can be run with the following network flags:

| Network | Flag |
|---------|------|
| Testnet | `--testnet` or `-t` |
| Signet  | `--signet` or `-s` |
| Regtest | `--regtest` or `-r` |

Regtest doesn't require downloading the blockchain since you create your own
private blockchain, so indexing `bitomc` is almost instantaneous.

Example
-------

Run `bitcoind` in regtest with:

```
bitcoind -regtest -txindex
```

Run `bitomc server` in regtest with:

```
bitomc --regtest server
```

Create a wallet in regtest with:

```
bitomc --regtest wallet create
```

Get a regtest receive address with:

```
bitomc --regtest wallet receive
```

Mine 101 blocks (to unlock the coinbase) with:

```
bitcoin-cli -regtest generatetoaddress 101 <receive address>
```

Mint in regtest with:

```
bitomc --regtest wallet mint --fee-rate 1
```

Mine the transaction with:

```
bitcoin-cli -regtest generatetoaddress 1 <receive address>
```