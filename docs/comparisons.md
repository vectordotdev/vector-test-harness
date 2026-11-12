# Comparison requirements

Compare the same logical task from input A to receiver B:

- Use identical input and require equivalent application output.
- Define a shared delivery and durability contract.
- Verify record identities, counts and payloads at B, including duplicates and losses.
- Establish readiness, a measured input window and output drain.
- Report verified events/s or application payload bytes/s, with versions,
  hardware, configurations and repetitions.

Native protocols, batching and encodings may differ.

Historical comparisons were withdrawn because these conditions were not
established. Each retired case's README records its limitation.
