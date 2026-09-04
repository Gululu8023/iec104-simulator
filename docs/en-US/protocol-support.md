# IEC104 Protocol Support

This matrix describes v1.0.0 application behavior. It is not a claim of complete IEC 60870-5-104 coverage.

| Capability | Master send | Master parse | Slave response | Notes |
| --- | --- | --- | --- | --- |
| STARTDT / STOPDT / TESTFR | Yes | Yes | Yes | Manual link control and idle testing |
| Station interrogation | Yes | Yes | Yes | `C_IC_NA_1`, QOI=20 |
| Group interrogation 1–16 | Yes | Yes | Yes | Uses point interrogation groups |
| Counter interrogation | Yes | Yes | Yes | All/groups 1–4; freeze/reset qualifiers |
| Single, double, regulating commands | Yes | Yes | Yes | SBO or direct; CP56 variants included |
| Normalized, scaled, float setpoints | Yes | Yes | Yes | CP56 variants included |
| Bitstring command | Yes | Yes | Yes | 32-bit value; CP56 variant included |
| Read command | Yes | Yes | Yes | Reads a supported point by IOA |
| Clock sync, reset, test command | Yes | Yes | Yes | Qualifiers exposed by the current UI |
| Monitoring data, quality, timestamps | N/A | Yes | Sends | Common indications, measurements, totals, and protection events |
| SQ=1 sequential addresses | N/A | Yes | Yes | Optional preference for monitoring uploads |
| File directory, log query, upload/download | Yes | Yes | Yes | Single-section transfer |
| Parameter ASDUs | No business send flow | Parse only | No business handling | Not a parameter-setting feature |
| Security extension ASDUs | No | Partial structural parse | No | Parsing does not imply protocol workflow support |

## Known limitations

- COT is fixed at 2 bytes, common address at 2 bytes, and IOA at 3 bytes; the UI does not expose length switching.
- Parsing parameter or security-extension ASDUs does not imply a Master send or Slave business workflow.
- CSV, JSON, and XML point tables can be imported but not exported. Preview displays at most 200 points; this is not the import limit.
- The SOE panel has no independent CSV export. Communication frames can be exported as CSV, TXT, PCAP, or PCAPNG.
- File transfer is single-section, dynamically limited by `max_asdu_bytes`, and restricted to one task per connection.
- Periodic reporting, analog threshold reporting, reporting delay, control failure probability, and preset operating scenarios are unavailable.
- Avalanche testing flips binary indications on the selected logical Slave; it is not general network stress or fault injection.
- Official installation and verification cover Windows 10/11 x64 only.

See the [User Guide](./user-guide.md) for complete workflows.
