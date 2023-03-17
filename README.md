<!-- markdownlint-configure-file
{
    "no-duplicate-header": {
        "siblings_only": true
    }
}
-->

# Configuration API microservice

[![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-yellow.svg)](https://conventionalcommits.org)

Microservice serving configuration data as an HTTP API backed by MongoDB.

## Routes

### Health

#### `GET` `/health`

Returns the service health status.

##### Parameters

None

##### Response

| Code | Description          |
| ---- | -------------------- |
| 204  | Service is healthy   |
| 500  | Service in unhealthy |

### Configuration data

#### `GET` `/config/{collection}/{id}`

Returns configuration data.

##### Parameters

| Name                | Description                |
| ------------------- | -------------------------- |
| collection _(path)_ | MongoDB collection         |
| id _(path)_         | ID of the MongoDB document |

##### Response

| Code | Description             |
| ---- | ----------------------- |
| 200  | Document in JSON format |
| 400  | Document not found      |
| 500  | Internal server error   |

## Usage

```ShellSession
$ config-api --help
Usage: config-api [OPTIONS] --mongodb-database <MONGODB_DATABASE>

Options:
      --listen-address <LISTEN_ADDRESS>
          Address to listen on [env: LISTEN_ADDRESS=] [default: 0.0.0.0:8080]
      --mongodb-uri <MONGODB_URI>
          URI of MongoDB server [env: MONGODB_URI=] [default: mongodb://mongodb]
      --mongodb-database <MONGODB_DATABASE>
          MongoDB database [env: MONGODB_DATABASE=]
  -v, --verbose...
          More output per occurrence
  -q, --quiet...
          Less output per occurrence
  -h, --help
          Print help
```
