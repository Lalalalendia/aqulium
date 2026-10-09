# Search matching RAM A/B experiment

This experiment compares the exact normalized-position map used by the original Freaction/Aquilum with an alternative packed-u32 map.

Original: 4 usize offsets per normalized codepoint (32 bytes per entry on Windows x64).
Candidate: 4 u32 offsets per normalized codepoint (16 bytes per entry).
An additional character-counting pass reserves a tighter Vec capacity. It may affect CPU time.

Both variants run on the exact pinned Aquilum commit, each in a fresh native Rust test process on the same Windows runner.
The test reports actual Windows Working Set and private committed bytes, before and while normalized buffers are live.

No modifications are made to upstream. The experimental compact variant is valid only for inputs shorter than 4 GiB. A real upstream PR must either fall back to the original 64-bit map for huge documents or represent offsets without such a limit. It must preserve matching offsets across Unicode casefolding, combining marks, and Russian text.

The test includes Unicode comparison against original search matching behavior. Positive memory improvement must be weighed against construction cost and measured end-to-end search latency. The candidate does not automatically improve idle desktop RAM; the normalized map is allocated transiently while constructing search matches.
