Introduction
============

This handbook is a guide to BitOMC, a metaprotocol on Bitcoin that facilitates 
the creation of a more stable unit of account for Bitcoin payments.

In a world with ever-changing economic conditions, Bitcoin's fixed supply makes it 
unattractive as a unit of account in contracts where payment is due in the future. 
BitOMC addresses this by defining a market-driven unit of account that can facilitate
price stability in a Bitcoin economy, without changing Bitcoin's core protocol.

BitOMC uses two interconvertible assets, Tighten and Ease, to establish a dynamic
interest rate and a new unit of Bitcoin called the "util". Tighten and Ease are 
transferable on Bitcoin using rules nearly identical to those used by the Runes
protocol.

BitOMC is not premined. Users can mint Tighten and Ease once per block according to
the same four-year halving schedule as Bitcoin. To limit network congestion and MEV, 
mint and conversion transactions leave a small anchor output, which must be spent by 
the next respective transaction.

For more high-level details, see the [overview](overview.md).

For details on the specification, see [bitomc](bitomc.md).

When you're ready to get your hands dirty, a good place to start is by 
[minting](guides/wallet.md).

Links
-----

- [GitHub](https://github.com/BitOMC/BitOMC/)
- [BitOMC Whitepaper](https://bitomc.org/bitomc.pdf)
- [BitOMC Telegram group](https://t.me/bitOMC_chat)
- [BitOMC Website](https://bitomc.org/)
