# EIP-8282 EL/CL wire fixture

Synthetic real-BLS records from ETH2030's EIP-8282 fixture generator, with genesis fork version `0x00000001`, signing domain `DOMAIN_BUILDER_DEPOSIT` (`0x0e000000`) and EIPs `3db38a3725105cb67b8804ac3e147967113bc3ca` / consensus-specs `2ea9d96a420927eb1a35d7c9399c8b3691c69f6a`.

The same 184/68-byte records are matched byte-for-byte against the actual node-backed Engine output in `pkg/eiptest/builder_requests_8282_test.go`. The wrong-domain record has the same key/credentials/amount signed under the validator deposit domain and must not register a builder. No private production keys or funds are involved.
