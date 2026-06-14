# Chronostore Wind Tunnel

Criterion benchmarks for Chronostore's storage kernel.

Run them with:

```sh
cargo bench -p wind_tunnel
```

The wind tunnel starts with million-point baseline measurements for:

- batch insertion with no summary work
- batch insertion with the current simple summary
- forward and backward nearest-value lookup over 1M and 10M point chronologies
- full-range summary queries over 1M and 10M point chronologies
- 1,024-bucket viewport summaries over 1M and 10M point chronologies
- raw vs Gorilla `f64` sealed-chunk codec storage, lookup, and viewport queries
- raw vs Gorilla `f64` exact range visiting over 65,536 entries
- retained appends that evict one sealed chunk while keeping 256 sealed chunks

Criterion is scoped to this benchmark crate as a dev-dependency. The core
`chronostore` crate does not gain benchmark dependencies or dev-dependencies
from this harness.
