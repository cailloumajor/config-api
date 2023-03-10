# Static configuration API

[![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg)](https://conventionalcommits.org)

Reads a TOML configuration file and serves it as JSON.

## Usage

```ShellSession
$ static-config-api --help
Usage: static-config-api [OPTIONS] --config-path <CONFIG_PATH>

Options:
      --listen-address <LISTEN_ADDRESS>
          Address to listen on [env: LISTEN_ADDRESS=] [default: 0.0.0.0:8080]
      --config-path <CONFIG_PATH>
          Path of the static configuration TOML file [env: CONFIG_PATH=]
  -h, --help
          Print help
```
