Reindexing
==========

Sometimes the `bitomc` database must be reindexed, which means deleting the
database and restarting the indexing process with either `bitomc index update` or
`bitomc server`. Reasons to reindex are:

1. A new major release of bitomc, which changes the database scheme
2. The database got corrupted somehow

The database `bitomc` uses is called [redb](https://github.com/cberner/redb),
so we give the index the default file name `index.redb`. By default we store this
file in different locations depending on your operating system.

|Platform | Value                                            | Example                                      |
| ------- | ------------------------------------------------ | -------------------------------------------- |
| Linux   | `$XDG_DATA_HOME`/bitomc or `$HOME`/.local/share/bitomc | /home/alice/.local/share/bitomc                 |
| macOS   | `$HOME`/Library/Application Support/bitomc          | /Users/Alice/Library/Application Support/bitomc |
| Windows | `{FOLDERID_RoamingAppData}`\bitomc                  | C:\Users\Alice\AppData\Roaming\bitomc           |

So to delete the database and reindex on MacOS you would have to run the following
commands in the terminal:

```bash
rm ~/Library/Application Support/bitomc/index.redb
bitomc index update
```

You can of course also set the location of the data directory yourself with `bitomc
--datadir <DIR> index update` or give it a specific filename and path with `bitomc
--index <FILENAME> index update`.
