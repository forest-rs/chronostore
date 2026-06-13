# Chronostore Wind Tunnel

Criterion benchmarks for Chronostore's storage kernel.

Run them with:

```sh
cargo bench -p wind_tunnel
```

The wind tunnel starts with million-point baseline measurements for:

- batch insertion with no summary work
- batch insertion with the current simple summary
- forward and backward nearest-value lookup over an existing chronology

Criterion is scoped to this benchmark crate as a dev-dependency. The core
`chronostore` crate does not gain benchmark dependencies or dev-dependencies
from this harness.
