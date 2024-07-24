BitOMC
=====

BitOMC is a fork of the [Runes protocol](https://docs.ordinals.com/runes.html).
Runes allow Bitcoin transactions to mint, transfer, and convert Bitcoin-native
digital commodities. There are only two runes in BitOMC, Tighten and Ease.

Runestones
----------

Rune protocol messages, called runestones, are stored in Bitcoin transaction
outputs.

In BitOMC, a runestone output's script pubkey begins with an `OP_RETURN`, followed
by `OP_14`, followed by zero or more data pushes. These data pushes are
concatenated and decoded into a sequence of 128-bit integers, and finally
parsed into a runestone.

A transaction may have at most one runestone.

A runestone may mint an existing rune, transfer runes from a transaction's inputs 
to its outputs, and convert from one rune to another.

A transaction output may hold balances of any number of runes.

Runes are identified by IDs. The IDs of Tighten and Ease are 
`1:0` and `1:1`, respectively.

Minting
-------

Anyone may create a mint transaction that mints the amount of Tighten and Ease
available in the block the transaction is mined, plus any amount of Tighten and
Ease that has been burned. The issuance of Tighten and Ease follows a four-year
halving schedule as outlined in [the whitepaper](https://bitomc.org/bitomc.pdf).

Transferring
------------

When transaction inputs contain runes, or new runes are created by a mint, those 
runes are transferred to that transaction's outputs. A transaction's runestone 
may change how input runes transfer to outputs.

### Edicts

A runestone may contain any number of edicts. Edicts consist of a rune ID, an
amount, and an output number. Edicts are processed in order, allocating
unallocated runes to outputs.

### Pointer

After all edicts are processed, remaining unallocated runes are transferred to
the transaction's first non-`OP_RETURN` output. A runestone may optionally
contain a pointer that specifies an alternative default output.

### Burning

Runes may be burned by transferring them to an `OP_RETURN` output with an edict
or pointer.

Converting
------------

When runes of one ID are burned and runes of another are transferred in excess
of their input balance, those runes are created if the conversion rules are 
satisfied. Any remaining runes post-conversion are transferred to the ID of the 
last edict in the runestone.

Cenotaphs
---------

Runestones may be malformed for a number of reasons, including non-pushdata
opcodes in the runestone `OP_RETURN`, invalid varints, or unrecognized
runestone fields.

Malformed runestones are termed
[cenotaphs](https://en.wikipedia.org/wiki/Cenotaph).

Runes input to a transaction with a cenotaph are burned. Runes etched in a
transaction with a cenotaph are set as unmintable. Mints in a transaction with
a cenotaph count towards the mint cap, but the minted runes are burned.

Cenotaphs are an upgrade mechanism, allowing runestones to be given new
semantics that change how runes are created and transferred, while not
misleading unupgraded clients as to the location of those runes, as unupgraded
clients will see those runes as having been burned.
