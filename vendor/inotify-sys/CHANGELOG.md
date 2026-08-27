# Changelog

## v0.1.8 (2026-07-07)

- Use native inotify on FreeBSD unconditionally ([#32])

[#32]: https://github.com/hannobraun/inotify-sys/pull/32

## v0.1.7 (2026-07-02)

- Add `freebsd-native` feature to bypass libnotify on FreeBSD ([#26])

[#26]: https://github.com/hannobraun/inotify-sys/pull/26

## v0.1.6 (2026-07-01)

- Add support for FreeBSD via libinotify ([#25], [#28])
- Add `IN_MASK_CREATE` ([#27])

[#25]: https://github.com/hannobraun/inotify-sys/pull/25
[#27]: https://github.com/hannobraun/inotify-sys/pull/27
[#28]: https://github.com/hannobraun/inotify-sys/pull/28

## v0.1.5 (2021-01-16)

- Add LICENSE file ([#21])

[#21]: https://github.com/hannobraun/inotify-sys/pull/21

## v0.1.4 (2020-11-06)

- Fix build by no longer failing on warnings

## v0.1.3 (2018-07-26)

### Bug Fixes

- Use platform-specific constants from libc ([b363dff1](b363dff1))
