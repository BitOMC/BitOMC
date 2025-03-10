Settings
========

`bitomc` can be configured with the command line, environment variables, a
configuration file, and default values.

The command line takes precedence over environment variables, which take
precedence over the configuration file, which takes precedence over defaults.

The path to the configuration file can be given with `--config <CONFIG_PATH>`.
`bitomc` will error if `<CONFIG_PATH>` doesn't exist.

The path to a directory containing a configuration file name named `bitomc.yaml`
can be given with `--config-dir <CONFIG_DIR_PATH>` or `--datadir
<DATA_DIR_PATH>` in which case the config path is `<CONFIG_DIR_PATH>/bitomc.yaml`
or `<DATA_DIR_PATH>/bitomc.yaml`. It is not an error if it does not exist.

If none of `--config`, `--config-dir`, or `--datadir` are given, and a file
named `bitomc.yaml` exists in the default data directory, it will be loaded.

For a setting named `--setting-name` on the command line, the environment
variable will be named `BITOMC_SETTING_NAME`, and the config file field will be
named `setting_name`. For example, the data directory can be configured with
`--datadir` on the command line, the `BITOMC_DATA_DIR` environment variable, or
`data_dir` in the config file.

See `bitomc --help` for documentation of all the settings.

`bitomc`'s current configuration can be viewed as JSON with the `bitomc settings`
command.

Example Configuration
---------------------

```yaml
{{#include ../../../bitomc.yaml}}
```
