Moderation
==========

`bitomc` includes a block explorer, which you can run locally with `bitomc server`.

The block explorer allows viewing inscriptions. Inscriptions are user-generated
content, which may be objectionable or unlawful.

It is the responsibility of each individual who runs an ordinal block explorer
instance to understand their responsibilities with respect to unlawful content,
and decide what moderation policy is appropriate for their instance.

In order to prevent particular inscriptions from being displayed on an `bitomc`
instance, they can be included in a YAML config file, which is loaded with the
`--config` option.

To hide inscriptions, first create a config file, with the inscription ID you
want to hide:

```yaml
hidden:
- 0000000000000000000000000000000000000000000000000000000000000000i0
```

The suggested name for `bitomc` config files is `bitomc.yaml`, but any filename can
be used.

Then pass the file to `--config` when starting the server:

`bitomc --config bitomc.yaml server`

Note that the `--config` option comes after `bitomc` but before the `server`
subcommand.

`bitomc` must be restarted in to load changes to the config file.

`ordinals.com`
--------------

The `ordinals.com` instances use `systemd` to run the `bitomc server` service,
which is called `bitomc`, with a config file located at `/var/lib/ord/bitomc.yaml`.

To hide an inscription on `ordinals.com`:

1. SSH into the server
2. Add the inscription ID to `/var/lib/ord/bitomc.yaml`
3. Restart the service with `systemctl restart bitomc`
4. Monitor the restart with `journalctl -u bitomc`

Currently, `bitomc` is slow to restart, so the site will not come back online
immediately.
