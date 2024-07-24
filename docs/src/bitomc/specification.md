BitOMC Does Not Have a Specification
===================================

The BitOMC reference implementation, `bitomc`, is the normative specification of
the BitOMC protocol.

Nothing you read here or elsewhere, aside from the code of `bitomc`, is a
specification. This prose description of the BitOMC protocol is provided as a
guide to the behavior of `bitomc`, and the code of `bitomc` itself should always be
consulted to confirm the correctness of any prose description.

If, due to a bug in `bitomc`, this document diverges from the actual behavior of
`bitomc` and it is impractically disruptive to change `bitomc`'s behavior, this
document will be amended to agree with `bitomc`'s actual behavior.

Users of alternative implementations do so at their own risk, and services
wishing to integrate Runes are strongly encouraged to use `bitomc` itself to make
Runes transactions, and to determine the state of runes, mints, and balances.

Runestones
----------

Like Runes, BitOMC protocol messages are termed "runestones".

The protocol activates on block 854,000. Runestones in earlier blocks are
ignored.

Abstractly, runestones contain the following fields:

```rust
struct Runestone {
  edicts: Vec<Edict>,
  pointer: Option<u32>,
}
```

Runes are transferred by edict:

```rust
struct Edict {
  id: RuneId,
  amount: u128,
  output: u32,
}
```

Rune IDs are encoded as the block height and transaction index of the
transaction in which the rune was etched:

```rust
struct RuneId {
  block: u64,
  tx: u32,
}
```

Rune IDs are represented in text as `BLOCK:TX`.

Rune names are encoded as modified base-26 integers:

```rust
struct Rune(u128);
```

### Deciphering

Runestones are deciphered from transactions with the following steps:

1. Find the first transaction output whose script pubkey begins with `OP_RETURN
   OP_14`.

2. Concatenate all following data pushes into a payload buffer.

3. Decode a sequence 128-bit [LEB128](https://en.wikipedia.org/wiki/LEB128)
   integers from the payload buffer.

4. Parse the sequence of integers into an untyped message.

5. Parse the untyped message into a runestone.

Deciphering may produce a malformed runestone, termed a
[cenotaph](https://en.wikipedia.org/wiki/Cenotaph).

#### Locating the Runestone Output

Outputs are searched for the first script pubkey that beings with `OP_RETURN
OP_14`. If deciphering fails, later matching outputs are not considered.

#### Assembling the Payload Buffer

The payload buffer is assembled by concatenating data pushes, after `OP_14`, in
the matching script pubkey.

Data pushes are opcodes 0 through 78 inclusive. If a non-data push opcode is
encountered, i.e., any opcode equal to or greater than opcode 79, the
deciphered runestone is a cenotaph with no etching, mint, or edicts.

#### Decoding the Integer Sequence

A sequence of 128-bit integers are decoded from the payload as LEB128 varints.

LEB128 varints are encoded as sequence of bytes, each of which has the
most-significant bit set, except for the last.

If a LEB128 varint contains more than 18 bytes, would overflow a u128, or is
truncated, meaning that the end of the payload buffer is reached before
encountering a byte with the continuation bit not set, the decoded runestone is
a cenotaph with no etching, mint, or edicts.

#### Parsing the Message

The integer sequence is parsed into an untyped message:

```rust
struct Message {
  pointer: Option<u32>,
  edicts: Vec<Edict>,
}
```

If the number of integers is odd, the first integer is interpreted as the pointer,
and all subsequent integer pairs are interpreted as edicts. If the number of integers
is even, the pointer is set to null, and each integer pair is interpreted as an edict.

```rust
struct Edict {
  id: RuneId,
  amount: u128,
  output: u32,
}
```

In the integer pair, `id` is interpreted as `1:0` (Tighten) if the first integer is odd.
Otherwise, `id` is interpreted as `1:1` (Ease). `amount` is interpreted as the first
integer divided by 2, and `output` is interpreted as the second integer.

#### Minting

A transaction may mint Tighten and Ease by including an output that anyone can spend after
1 block, using a P2WSH for `OP_1 OP_CHECKSEQUENCEVERIFY`, and spending the output of the
previous mint, if unspent. If no such output exists, the first mint transaction receives
the mint reward.

If successful, the mint amount is added to the unallocated runes in the
transaction's inputs. These runes may be transferred using edicts, and will
otherwise be transferred to the first non-`OP_RETURN` output, or the output
designated by the `Pointer` field. A runestone is required for a mint transaction
to be valid.

At the start of each block, the amount of Tighten and Ease to be issued is immediately burned,
and the mint transaction receives the amount of outstanding burned Tighten and Ease. For this
reason, if no mint transaction is present, the runes to be issued are made available to
the first minter in subsequent blocks.

#### Transferring

Runes are transferred by edict:

```rust
struct Edict {
  id: RuneId,
  amount: u128,
  output: u32,
}
```

A runestone may contain any number of edicts, which are processed in sequence.

Before edicts are processed, input runes, as well as minted or premined runes,
if any, are unallocated.

Each edict decrements the unallocated balance of rune `id` and increments the
balance allocated to transaction outputs of rune `id`.

If an edict would allocate more runes than are currently unallocated, the
`amount` is reduced to the number of currently unallocated runes. In other
words, the edict allocates all remaining unallocated units of rune `id`.

An edict with `amount` zero allocates all remaining units of rune `id`.

An edict with `output` equal to the number of transaction outputs allocates
`amount` runes to each non-`OP_RETURN` output in order.

An edict with `amount` zero and `output` equal to the number of transaction
outputs divides all unallocated units of rune `id` between each non-`OP_RETURN`
output. If the number of unallocated runes is not divisible by the number of
non-`OP_RETURN` outputs, 1 additional rune is assigned to the first `R`
non-`OP_RETURN` outputs, where `R` is the remainder after dividing the balance
of unallocated units of rune `id` by the number of non-`OP_RETURN` outputs.

If any edict in a runestone has an `output` greater than the number of transaction
outputs, the runestone is a cenotaph.

Note that edicts in cenotaphs are not processed, and all input runes are
burned.

#### Converting

A runestone may convert between Tighten and Ease by burning one as an input
and allocating more of the other than are unallocated. The ID of the final edict
in the Runestone determines whether the input or the output of the conversion is
to be exact. If the ID is the same as the output ID, then the input is exact
and the required output is the minimum allowable output amount. Otherwise, the output
is exact and the input is the maximum allowable input amount.

Runes are converted according to the invariant `A^2 + B^2 = K^2`, where `A` and
`B` are the supply of Tighten and Ease, respectively, and `K` is some constant, 
which is the same before and after.

If the invariant is satisfied, the output runes are allocated. If the conversion 
yields more output runes than the minimum allowable output amount, the additional 
output runes are allocated to the first tx output already receiving output runes.

If the conversion requires fewer input runes than the maximum allowable input amount,
the leftover input runes are allocated to the output of the last edict of input ID
with an output whose value is less than the number of tx outputs. If no such output
exists, the input runes are allocated to the first output with a non-zero balance 
for input ID. If still unallocated, the input runes are allocated to the output
of the first edict.

If the invariant is not satisfied, the transaction does not signal RBF, the 
transaction fails to leave a P2WPKH output for the private key `0x01` repeating,
or the P2WPKH output of the preceding conversion is unspent and not spent by this
transaction, the burned input runes are unburned and allocated according to the same
rules as the preceding paragraph.

An exception to this final condition is made for conversions in a block `P` in which
the first conversion cannot spend an output of a previous conversion. When this is case, 
conversions in the block do not need to spend an output of the previous conversion
to be valid. The first conversion in the next block, however, must spend the output
created by the first conversion in block `P`.

The Util
----------

The util is the dynamic unit of account created by BitOMC. The util is a dynamic
sub-denomination of sats, determined by an interest rate that is controlled by
the relative quantity of Tighten and Ease. To calculate the number of utils per sat,
a second sub-denomination of sats is introduced, called the e-bond. The number of
e-bonds per sat increases monotically at the current interest rate. The number of
utils per sat is then determined by the number of utils per e-bond, which is 1 divided 
by the interest rate.

### The Interest Rate

The interest rate equals the median value of `(A - B) / (A + B)` over the last 100 blocks
where `A > B` and `(A - B) / (A + B)` is non-zero after integer division. `A` and `B` are
the supply of Tighten and Ease, respectively, and the value is calculated to 12 decimal 
places. The default interest rate is 100%.

The number of e-bonds per sat is stored as an integer with 12 decimal places and updated 
each block using the current interest rate divided by 52,595, which is approximately the 
number of blocks per year.

### Utils per Sat

The number of utils per bond is calculated by dividing 10^24 by the interest rate in
integer form.

The number of utils per sat is calculated by multiplying the number of bonds per sat by
the number of utils per bond and dividing by 10^12.
