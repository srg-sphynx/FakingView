# Third-party notices and source provenance

FakingView is independently maintained. The acknowledgements below identify code and assets used
by the project; they do not imply affiliation, sponsorship or endorsement by their authors.

## Application source

FakingView includes modified source from **DegenView**, originally authored by **Nico Oelgart and
contributors**.

- Original source and history: <https://github.com/degenview/degenview>
- Imported baseline: `5950cbef2091f588ec4a1d950efcb52e2cf5200d`
- Original application copyright notice: **Copyright © 2026 Nico Oelgart**
- Application license: **GNU General Public License v3.0 only** (`GPL-3.0-only`), reproduced in
  [LICENSE.md](LICENSE.md)

The FakingView modifications include the personal Kite Connect market-data adapter, Indian
instrument routing and timestamp handling, secure daily-session settings, application identity
and branding, integration regression tests, and repository/release documentation. The original
charting, drawing, scripting and other application components retain their original provenance
and copyright. Git history records the imported source and subsequent modifications.

This is a modified version distributed under its own name. It is not an official release of the
original project. The GPL license and applicable copyright and legal notices must accompany
redistributions. Binary distributions must provide corresponding source as required by the license.

## Bundled fonts

The fonts retain the **SIL Open Font License 1.1** rather than the application's GPL license.

| Component | Copyright | License text |
| --- | --- | --- |
| JetBrains Mono NL Regular 2.304 | Copyright 2020 The JetBrains Mono Project Authors | [OFL-JetBrainsMono.txt](assets/fonts/OFL-JetBrainsMono.txt) |
| Noto Sans Symbols 2, subset of 2.008 | Copyright 2022 The Noto Project Authors | [OFL-NotoSansSymbols2.txt](assets/fonts/OFL-NotoSansSymbols2.txt) |

Full font provenance, file names and redistribution details are in
[assets/fonts/README.md](assets/fonts/README.md) and [docs/licensing.md](docs/licensing.md).

## Dependencies and service marks

Rust dependencies, including `degenview-brokers`, keep their own published licenses and names.
The exact resolved dependency set is recorded in [Cargo.lock](Cargo.lock); application dependency
configuration is in [Cargo.toml](Cargo.toml). A dependency's name is not a claim of project affiliation.

Market-provider logos identify their respective services and remain their owners' marks. They
are not relicensed by FakingView's application license; see [source-logo notices](docs/source-logos.md).
The Kite source glyph drawn for FakingView is described there separately. Zerodha, Kite Connect,
TradingView and all other product/service names remain the marks of their respective owners.

The FakingView application icon and new repository artwork are distributed with the project.
Existing example/source screenshots, if retained elsewhere in the repository, do not establish
live exchange verification for this release. Public product screenshots should identify sample
or fixture data when it is used.
