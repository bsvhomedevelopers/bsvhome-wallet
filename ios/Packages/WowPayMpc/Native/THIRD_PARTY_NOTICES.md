# Third-party code

The threshold ECDSA implementation is [0xCarbon/DKLs23](https://github.com/0xCarbon/DKLs23), fixed to commit `c9c407eae3447bcbfb539d77bb910ceb2d3a005e` (v0.5.1). Its MIT and Apache-2.0 license texts are retained in `licenses/`. The protocol, this platform wrapper, and its recovery flow are not represented as independently audited.

The participant, backup, identity, and transport implementation was adapted from the WowPay Android role-A library at source commit `32047f8a5441a5674db974331269a5b90060a257`, retaining its MIT package license. The JNI adapter is not included.

`Cargo.lock` retains the exact dependency versions and checksums, with only the iOS package identity and removal of JNI-only dependencies changed. The fixed `k256`, `elliptic-curve`, `rustcrypto-ff/group`, `ecdsa`, and `rand` versions remain separate from the Gemstone dependency graph.

AES-GCM, HKDF, SHA-256, RIPEMD-160, system randomness, and independent secp256k1 verification use their respective Rust libraries. Final distribution still requires review of target-platform dependencies, licenses, and vulnerability status; a protocol name does not imply an audit of this build.
