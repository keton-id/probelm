# Changelog

All notable changes to `probelm` are documented here.

The file is maintained by Release Please from conventional commits. Do not edit release sections or bump versions by hand.

## [1.1.0](https://github.com/keton-id/probelm/compare/v1.0.0...v1.1.0) (2026-09-21)


### Features

* add --reasoning-effort flag and reasoningEffort config option ([d976c4a](https://github.com/keton-id/probelm/commit/d976c4a2323dd37ff4829f7e17a1226fac99a20c))
* add --reasoning-effort flag and reasoningEffort config option ([0d54bfa](https://github.com/keton-id/probelm/commit/0d54bfad0d4d9a9590002ce42e49f4099b14317d))


### Bug Fixes

* make release recovery rerunnable ([9f292f3](https://github.com/keton-id/probelm/commit/9f292f31eb0b7b1119413d1be009057eabd7f647))
* recover release distribution publishing ([5f29615](https://github.com/keton-id/probelm/commit/5f296155fbaca158f48b3e13f5bb5bc61073e7ab))
* **release:** make recovery reruns idempotent ([6dbe1dd](https://github.com/keton-id/probelm/commit/6dbe1dd441f929baf24650f3a519410e29689bc0))
* **release:** recover registry and tap publishing ([94254fe](https://github.com/keton-id/probelm/commit/94254feb0227050f7b55426ef41a6d9d577d7ad0))

## 1.0.0 (2026-09-18)


### Features

* add authoritative specs catalog and sync-specs command from LiteLLM database ([db60d76](https://github.com/keton-id/probelm/commit/db60d763d6b0ecb209259d1baed01a9c3cef9e6c))
* add interactive init wizard, installer script, capability icons and table formatting ([e31423e](https://github.com/keton-id/probelm/commit/e31423ea9de077031da0e14ad3f57052fc3454be))
* add MCP harness integration ([e5b521f](https://github.com/keton-id/probelm/commit/e5b521fdbca37831e768a8b2ace4e85c14bb23c4))
* add multi-key sorting support including context window size (ctx) ([e0a87f1](https://github.com/keton-id/probelm/commit/e0a87f12939d916193c465bd493a31542ea24dcc))
* add test command aliases, positional args, sorting, markdown table, and refreshed docs ([c4d17b0](https://github.com/keton-id/probelm/commit/c4d17b034c3e8a12453d91e4980ef7db5acb4e25))
* add wildcard glob matching and multi-prefix filtering for model testing ([a83dc8d](https://github.com/keton-id/probelm/commit/a83dc8d96ef2ca08b2ad1d1b3151a42e80502da8))
* calculate fair token generation throughput based on stream generation duration ([a1a9918](https://github.com/keton-id/probelm/commit/a1a9918c40f1b91ad8b70dfbb47822eb7759cb3b))
* **mcp:** add stdio server and harness install ([18561b0](https://github.com/keton-id/probelm/commit/18561b0b175a50ab50931ac173c87f1ff81069d7))
* **probe:** improve TTFT detection for reasoning deltas and add stream_options usage parsing ([4d7a9e9](https://github.com/keton-id/probelm/commit/4d7a9e9de3a217aebf21a1d5a75f6350658e5e74))
* support probelm primary binary name and pure inference TTFT measurement ([2ddbafb](https://github.com/keton-id/probelm/commit/2ddbafbc386e9b49c6387ab824de9f07b458b91b))


### Bug Fixes

* **cli:** standardize probelm command output ([8201d7b](https://github.com/keton-id/probelm/commit/8201d7be9cc4ebedaf543d0ed2ee4fbf4b911f87))
* parse chunked SSE responses safely ([#1](https://github.com/keton-id/probelm/issues/1)) ([5c933cf](https://github.com/keton-id/probelm/commit/5c933cf2543c2075d8bb0e2461f1e5ed4c5cdf89))

## [Unreleased]

- Prepare public distribution through crates.io, npm, Homebrew, and Scoop.
