# Changelog

## [1.2.0](https://github.com/cailloumajor/static-config-api/compare/v1.1.1...v1.2.0) (2022-06-22)


### Features

* implement etag-based caching for static config ([f0023b7](https://github.com/cailloumajor/static-config-api/commit/f0023b7ac2044d8571fb6fbe371b4f48bc1e1c3f))
* implement http problem details ([26a097d](https://github.com/cailloumajor/static-config-api/commit/26a097d09425c4b5680a7aef68437b6e01d9804f))
* make detail member optional ([2d84dbb](https://github.com/cailloumajor/static-config-api/commit/2d84dbb4befbc5176f1135a8294e8eddb683bbf2))
* switch to trillium ([16187bc](https://github.com/cailloumajor/static-config-api/commit/16187bcb3e32a1bf036cf79dddf8ee6dbdeb00b1))
* switch to trillium-client for health checking ([ad27f75](https://github.com/cailloumajor/static-config-api/commit/ad27f758dac6d15f5b93b710101227078f670a84))
* use JSON pointer to get configuration subset ([bc492c6](https://github.com/cailloumajor/static-config-api/commit/bc492c614872fb7af569f94bf8b4c9f6ed64500b))


### Bug Fixes

* **deps:** update rust crate anyhow to 1.0.58 ([2647c62](https://github.com/cailloumajor/static-config-api/commit/2647c62df7c0e6519f29283ea9f060ab20a38692))
* **deps:** update rust crate async-std to 1.12.0 ([f963a86](https://github.com/cailloumajor/static-config-api/commit/f963a86af6e3a84441680b2775dc91a5b19467be))
* do not use compression ([c299761](https://github.com/cailloumajor/static-config-api/commit/c299761b7cde34237a0cb71a8d814c51d10366bc))
* remove `Server` header from responses ([415dab1](https://github.com/cailloumajor/static-config-api/commit/415dab16a7673806c37b87906ac3d30a0e61e912))

## [1.1.1](https://github.com/cailloumajor/static-config-api/compare/v1.1.0...v1.1.1) (2022-06-16)


### Bug Fixes

* set correct path for health endpoint ([7035ad4](https://github.com/cailloumajor/static-config-api/commit/7035ad451cb9cc098012fb7888ab92b591d221e8))

## [1.1.0](https://github.com/cailloumajor/static-config-api/compare/v1.0.2...v1.1.0) (2022-06-16)


### Features

* add healthcheck endpoint ([b01b874](https://github.com/cailloumajor/static-config-api/commit/b01b874cb412ffbc6ef647a2b78505564beded8d))

## [1.0.2](https://github.com/cailloumajor/static-config-api/compare/v1.0.1...v1.0.2) (2022-06-14)


### Bug Fixes

* verify binaries after build ([3a22cfd](https://github.com/cailloumajor/static-config-api/commit/3a22cfdb901261b96d33b6ea6ffdbbae3d2aa949))

## [1.0.1](https://github.com/cailloumajor/static-config-api/compare/v1.0.0...v1.0.1) (2022-06-13)


### Bug Fixes

* update regex to get Rust version in Dockerfile ([ecd783b](https://github.com/cailloumajor/static-config-api/commit/ecd783b83219322ae27fcb8cad93753f48783650))
* use cross-compilation when building image ([61ed57d](https://github.com/cailloumajor/static-config-api/commit/61ed57d1ac2215998714a160ca6319f123478126))

## 1.0.0 (2022-06-13)


### Features

* add cli options handling ([92ddf91](https://github.com/cailloumajor/static-config-api/commit/92ddf9128351478d2d2df2978b31d04b16f9d0e7))
* add Dockerfile ([1ce0a55](https://github.com/cailloumajor/static-config-api/commit/1ce0a557a5f1a2ccf9339012cbc42b8c635b1372))
* add static config endpoint implementation ([43bf2ce](https://github.com/cailloumajor/static-config-api/commit/43bf2ce85fa0f1a3126c229be0b0438d68653137))
* allow to get options from environment ([6ffd63c](https://github.com/cailloumajor/static-config-api/commit/6ffd63caf32bdca8a2a4476b7794eeab5d46d675))
* implement configuration file watching ([d3ade58](https://github.com/cailloumajor/static-config-api/commit/d3ade58e00d9bd962181ff485d83ee7c68912480))
* implement healthcheck binary ([718a3e1](https://github.com/cailloumajor/static-config-api/commit/718a3e1af900ec41ed1e3061f8380f88d0fac675))
* implement loading of static configuration ([ab53edc](https://github.com/cailloumajor/static-config-api/commit/ab53edc8bbc1160fc97e93a6b6997218dc494486))
* implement signals handling ([9a4c2af](https://github.com/cailloumajor/static-config-api/commit/9a4c2af642899e8ebcb4c7091a446cca6fda2570))


### Miscellaneous Chores

* release 1.0.0 ([75784da](https://github.com/cailloumajor/static-config-api/commit/75784dab57b18925b2aa700b32af82c198bb5ad9))
