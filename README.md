`bitomc`
=====

_**Disclaimer:** This project has been archived. Do not USE. It reflects my exploration and journey to understand all facets of Bitcoin. At the time of this project's creation, I struggled to understand how Bitcoin could become a viable unit of account, because I assumed that a risk-off asset will always rise in value relative to all other assets during times of crisis. This assumption, however, does not hold if Bitcoin is valued by a generally agreed upon social contract at 50% of the world's wealth (the maximum possible extent). As a result, I no longer think Bitcoin needs additional monetary features in order to be a viable unit of account in equilibrium._

`bitomc` is an index and command-line wallet. It is experimental
software with no warranty. See [LICENSE](LICENSE) for more details.

BitOMC is an experimental metaprotocol on Bitcoin that defines a dynamic sub-denomination 
of sats called the "util," which can adapt to changing economic conditions. The objective
is to define a credibly-neutral unit of account that can provide price stability to 
the Bitcoin economy.

This unit of account is controlled by the open market through an
interest rate, set by the relative quantity of two interconvertible assets, Tighten and 
Ease. Users convert between them according to a constant function conversion rule, ensuring
the relative quantity reflects the relative price. Tighten and Ease are issued
steadily over time, through a free mint that halves every four years. 

See [the whitepaper](https://bitomc.org/bitomc.pdf) for a technical description of the
protocol.

See [the docs](docs/src/SUMMARY.md) for documentation and guides.

Join [the Telegram group](https://t.me/bitOMC_chat) to chat with fellow users
of BitOMC.

Wallet
------

`bitomc` relies on Bitcoin Core for private key management and transaction signing.
This has a number of implications that you must understand in order to use
`bitomc` wallet commands safely:

- Bitcoin Core is not aware of Tighten and Ease runes and does not perform sat
  control. Using `bitcoin-cli` commands and RPC calls with `bitomc` wallets may
  lead to loss of runes.

- `bitomc wallet` commands automatically load the `bitomc` wallet given by the
  `--name` option, which defaults to 'bitomc'. Keep in mind that after running
  an `bitomc wallet` command, an `bitomc` wallet may be loaded.

- Because `bitomc` has access to your Bitcoin Core wallets, `bitomc` should not be
  used with wallets that contain a material amount of funds. Keep runic and
  cardinal wallets segregated.

Installation
------------

`bitomc` is written in Rust and can be built from
[source](https://github.com/BitOMC/BitOMC). Pre-built binaries are available on the
[releases page](https://github.com/BitOMC/BitOMC/releases).

You can install the latest pre-built binary from the command line with:

```sh
curl --proto '=https' --tlsv1.2 -fsLS https://bitomc.org/install.sh | bash -s
```

Once `bitomc` is installed, you should be able to run `bitomc --version` on the
command line.

Building
--------

On Linux, `bitomc` requires `libssl-dev` when building from source.

On Debian-derived Linux distributions, including Ubuntu:

```
sudo apt-get install pkg-config libssl-dev build-essential
```

On Red Hat-derived Linux distributions:

```
yum install -y pkgconfig openssl-devel
yum groupinstall "Development Tools"
```

You'll also need Rust:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Clone the `bitomc` repo:

```
git clone https://github.com/BitOMC/BitOMC.git
cd bitomc
```

To build a specific version of `bitomc`, first checkout that version:

```
git checkout <VERSION>
```

And finally to actually build `bitomc`:

```
cargo build --release
```

Once built, the `bitomc` binary can be found at `./target/release/bitomc`.

`bitomc` requires `rustc` version 1.76.0 or later. Run `rustc --version` to ensure
you have this version. Run `rustup update` to get the latest stable release.

### Docker

A Docker image can be built with:

```
docker build -t BitOMC/BitOMC .
```

### Debian Package

To build a `.deb` package:

```
cargo install cargo-deb
cargo deb
```

Contributing
------------

If you wish to contribute there are a couple things that are helpful to know. We
put a lot of emphasis on proper testing in the code base, with three broad
categories of tests: unit, integration and fuzz. Unit tests can usually be found at
the bottom of a file in a mod block called `tests`. If you add or modify a
function please also add a corresponding test. Integration tests try to test
end-to-end functionality by executing a subcommand of the binary. Those can be
found in the [tests](tests) directory. We don't have a lot of fuzzing but the
basic structure of how we do it can be found in the [fuzz](fuzz) directory.

We strongly recommend installing [just](https://github.com/casey/just) to make
running the tests easier. To run our CI test suite you would do:

```
just ci
```

This corresponds to the commands:

```
cargo fmt -- --check
cargo test --all
cargo test --all -- --ignored
```

Have a look at the [justfile](justfile) to see some more helpful recipes
(commands). Here are a couple more good ones:

```
just fmt
just fuzz
just doc
just watch ltest --all
```

If the tests are failing or hanging, you might need to increase the maximum
number of open files by running `ulimit -n 1024` in your shell before you run
the tests, or in your shell configuration.

We also try to follow a TDD (Test-Driven-Development) approach, which means we
use tests as a way to get visibility into the code. Tests have to run fast for that
reason so that the feedback loop between making a change, running the test and
seeing the result is small. To facilitate that we created a mocked Bitcoin Core
instance in [test-bitcoincore-rpc](./test-bitcoincore-rpc).

Syncing
-------

`bitomc` requires a synced `bitcoind` node. `bitomc` communicates with `bitcoind` via RPC.

If `bitcoind` is run locally by the same user, without additional
configuration, `bitomc` should find it automatically by reading the `.cookie` file
from `bitcoind`'s datadir, and connecting using the default RPC port.

If `bitcoind` is not on mainnet, is not run by the same user, has a non-default
datadir, or a non-default port, you'll need to pass additional flags to `bitomc`.
See `bitomc --help` for details.

`bitcoind` RPC Authentication
-----------------------------

`bitomc` makes RPC calls to `bitcoind`, which usually requires a username and
password.

By default, `bitomc` looks a username and password in the cookie file created by
`bitcoind`.

The cookie file path can be configured using `--cookie-file`:

```
bitomc --cookie-file /path/to/cookie/file server
```

Alternatively, `bitomc` can be supplied with a username and password on the
command line:

```
bitomc --bitcoin-rpc-username foo --bitcoin-rpc-password bar server
```

Using environment variables:

```
export BITOMC_BITCOIN_RPC_USERNAME=foo
export BITOMC_BITCOIN_RPC_PASSWORD=bar
bitomc server
```

Or in the config file:

```yaml
bitcoin_rpc_username: foo
bitcoin_rpc_password: bar
```

Logging
--------

`bitomc` uses [env_logger](https://docs.rs/env_logger/latest/env_logger/). Set the
`RUST_LOG` environment variable in order to turn on logging. For example, run
the server and show `info`-level log messages and above:

```
$ RUST_LOG=info cargo run server
```

New Releases
------------

Release commit messages use the following template:

```
Release x.y.z

- Bump version: x.y.z → x.y.z
- Update changelog
- Update changelog contributor credits
- Update dependencies
```
