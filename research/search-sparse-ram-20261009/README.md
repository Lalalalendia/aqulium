# Search normalization sparse offset map

This is an experiment on pinned Freaction/Aquilum a2ac434ba70c189a5e96e8712ec781ca608ad15c. No source changes are sent to upstream.

Problem: original search::matching::NormalizedText reserves one 32-byte NormalizedSpan for each normalized scalar and initializes capacity equal to the original byte count. This makes searching large notes temporarily allocate >100 MiB solely for offset maps.

Candidate: store UTF-8 offset *exceptions* only when one original character normalizes into a different number of scalar values or a different byte width. A char that folds to exactly one normalized scalar with the same UTF-8 byte width retains a fixed byte-offset shift across that ordinary interval and needs no map entry.

The sparse algorithm uses usize offsets throughout, has no 4 GiB limit, handles zero-byte removal of combining marks, multi-scalar decompositions, Cyrillic case folding, and Unicode. For lookup, binary-search exceptional spans, then map a normal character via the accumulated byte offset. Range semantics match the original reference implementation.

Validation:
- Additional direct Unicode samples (Russian, Turkish, Greek, combining marks, Korean Hangul), seeded randomized Unicode parity, and identity scalar checks.
- Full native Rust aquilum-core tests (including existing search tests); fix the Windows host temp path in the harness.
- Windows real Working Set and private commit A/B, baseline and sparse in separate processes, for 1 MB and 5 MB text.

The predicted RAM improvement is not a reported result until the workflow completes. The temporary map is allocated only during result highlighting; reducing it improves memory peaks and search CPU, not necessarily idle desktop RAM.
