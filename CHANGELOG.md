# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2022-08-05
- Fix transaction value conversion.

- Fix `tx-hash` calculation.

- GasOracle is now optional and defaults to the provider.

- Fix "already known" error.

- Fix "transaction underpriced" error.

## [0.3.0] - 2022-08-01
- Add `rustls` feature.

## [0.2.0] - 2022-07-26
- Complete `tx-manager` redesign

  This version makes the transaction manager's interface synchronous.
  Additionally, it offers support for EIP-1559,
  has persistent storage and a more robust tracking of the state of transactions,
  and uses a gas oracle.


## [0.1.0] - 2021-12-28
- Initial release

[Unreleased]: https://github.com/cartesi-corp/tx-manager/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/cartesi-corp/tx-manager/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/cartesi-corp/tx-manager/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/cartesi-corp/tx-manager/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cartesi-corp/tx-manager/releases/tag/v0.1.0
