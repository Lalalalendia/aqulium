# Packed search offset map, production-shaped candidate

Source: pinned Freaction/Aquilum commit a2ac434ba70c189a5e96e8712ec781ca608ad15c.
Prototype evidence: GitHub Actions run 37928766161, isolated Rust A/B memory test.

Candidate:
- Compact offsets as four u32 values per Unicode scalar in normal inputs.
- Preserve original 64-bit-wide (usize) offsets for huge documents.
- Automatically promote to wide map if normalization itself exceeds the 32-bit boundary.
- Preallocate spans according to Unicode character count, not input byte count.
- Keep matching semantics including UTF-8 range offsets, folded case and removed combining marks.

Tests:
- Full aquilum-core Rust release test suite.
- Explicit Unicode samples, seeded random mixed-language cases, and high-offset promotion test.
- Windows working-set/private-commit snapshots on live production-shaped normalizer.

These modifications happen only in a temporary CI clone. They have not been merged into the author's repository. Benchmarks focus transient search RAM; do not interpret them as an idle-app RAM reduction.
