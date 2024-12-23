# Contributing

## setting log levels

Set `RUST_LOG` to the level of your choice

## Debugging

Generally, set the following environment variables to help with debugging:

```bash
RUST_LOG=debug RUST_LOG_BACKTRACE=1
```

- RUST_LOG works via using `tracing-subscriber`, with the env-filter feature.
- RUST_LOG_BACKTRACE is provided by the `anyhow` crate.