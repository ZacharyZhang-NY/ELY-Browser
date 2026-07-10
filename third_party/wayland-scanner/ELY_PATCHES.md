# ELY security backport

Source: `wayland-scanner` 0.31.10 from crates.io, archive SHA-256
`9c324a910fd86ebdc364a3e61ec1f11737d3b1d6c273c0239ee8ff4bc0d24b4a`, published under the MIT license.

ELY carries the following changes while the upstream security release remains unpublished:

- Upgrade `quick-xml` from 0.39 to 0.41 for RUSTSEC-2026-0194 and RUSTSEC-2026-0195.
- Use `GeneralRef::xml10_content()` for the quick-xml 0.41 API.
- Split message parsing and writing generation into a dedicated module so production source files remain below 500 lines.

Upstream references:

- Security dependency update: `d07c4f91f28b42e5a485823ffd9d8d5a210b1053`.
- quick-xml API migration: `ec2d932855593d48aa83c76820f3efbcfea86d39`.
- Equivalent boolean attribute parsing cleanup: `006b508ee49669fc42031a150ea9b316297e9cf8`.
