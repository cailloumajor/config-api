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

### Get configuration data

#### `GET` `/config/{collection}/{id}`

Returns configuration data.

##### Parameters

| Name       | Source | Description                |
| ---------- | ------ | -------------------------- |
| collection | _path_ | MongoDB collection         |
| id         | _path_ | ID of the MongoDB document |

##### Response

| Code | Description             |
| ---- | ----------------------- |
| 200  | Document in JSON format |
| 400  | Document not found      |
| 500  | Internal server error   |

##### Linked document

If the MongoDB document found contains a `_links` key with an [`ObjectId`][BSON ObjectId] value, the returned document will be the one with this index.

[BSON ObjectId]: https://www.mongodb.com/docs/v6.0/reference/bson-types/#objectid

### Patch configuration data

#### `PATCH` `/config/{collection}/{id}`

Applies changes on a configuration document.

##### Parameters

| Name       | Source | Description                |
| ---------- | ------ | -------------------------- |
| collection | _path_ | MongoDB collection         |
| id         | _path_ | ID of the MongoDB document |

##### Request body

The expected body request is a JSON object with key(s) and value(s) corresponding with those of the database document.

##### Authorization

The changes will be applied if all the following conditions are met:

* a document with `_authorization` primary key exists;
* this document contains a `patchAllowedFields` field;
* this field is an array;
* this array contains all of the keys of the request JSON body (as strings).

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
