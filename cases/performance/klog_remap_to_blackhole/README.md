# Klog remapping performance

The baked million-line fixture contains plain klog and structured JSON records.
Each record is decoded once into `log`, classified as text or JSON, and given the
structured record's numeric `ts` timestamp when present. Source-provided host
metadata is retained. The payload is not duplicated into top-level fields.

`remap-multihop.yaml` performs parsing and timestamp handling in two VRL nodes;
`remap-nohop.yaml` performs the same work in one node. Both route JSON directly to
the blackhole and pass text through the same multiline reducer. Klog headers
start new text records; continuation lines are joined with newlines.

The fixture does not contain Docker/container envelopes. That parsing belongs in
the dedicated wrapped-JSON and Docker cases rather than unused stages here.
The simplified workloads require fresh performance baselines.

Run `vth bench klog_remap_to_blackhole --vector-bin /path/to/vector`. Use
`--config` to select one variant; otherwise the harness also runs the stdin
baseline under `configs/`.
