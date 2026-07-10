# ELY private-operation gate

Source: `rsa 0.10.0-rc.18` from crates.io.

Source archive SHA-256: `30b2aa4ba0d89f73d1e332df05be0eeab8840351c36ca5654341dfdb57bb3caf`.

ELY adds and enables the `private-key-operations-disabled` default feature. The shared
`rsa_decrypt` primitive returns before every private exponent operation. This gates RSA
decrypt and signing while preserving public-key encryption and verification.
The vendored test source also removes one trailing space for repository hygiene.

The gate mitigates `RUSTSEC-2023-0071` while the advisory has no patched release.
Remove this vendor patch after RustCrypto publishes a complete constant-time fix.
