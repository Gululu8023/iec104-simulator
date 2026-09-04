# IEC104 Simulator User Guide

This guide covers the actual Master and Slave operating model. See [Protocol Support](./protocol-support.md) for protocol boundaries and [Point Table Format](./point-table-format.md) for import fields.

The workflow screenshots below use the Simplified Chinese locale; the English interface has the same controls and layout.

## Object model

- A Master **Link** represents one TCP connection and owns the remote endpoint, K/W/T0–T3, reconnect policy, and automatic actions. Its **logical Slaves** own common addresses and point tables.
- A Slave **Listener** owns the local TCP endpoint. Its **logical Slaves** own common addresses, point tables, control modes, SQ=1 preferences, and SOE policies.
- Creating a Link or Listener is not enough; create at least one logical Slave. One TCP endpoint may carry multiple logical Slaves with different common addresses.

## Loopback walkthrough

1. Start Slave and create a Listener named `Connection-1` on `127.0.0.1:2404`.
2. Create `Slave-1` below it with common address `1`, then add or import test points.
3. Select the Listener or logical Slave and start listening.
4. Start Master, create a Link to `127.0.0.1:2404`, then add a logical Slave with common address `1`.
5. Connect; if automatic actions are disabled, send STARTDT and station interrogation manually.
6. Confirm values, quality, cause, timestamps, and frames in the data and communication views.

IEC104 application commands are blocked while TCP is connected but STARTDT is incomplete.

## Master operations

### Connection lifecycle

The normal flow is TCP connect → STARTDT confirmation → application commands → optional automatic interrogation. Automatic reconnect covers remote disconnects and protocol timeouts; user-initiated disconnect does not reconnect. Reconnect parameters are edited on the Link.

![Master Link settings for link timers, reconnect, and automatic actions](../../assets/screenshots/master-link-settings.png)

*The Link editor configures K/W/T0–T3, retry policy, automatic STARTDT, and automatic interrogation in one place.*

### Acquisition and system commands

The top command area provides:

- station and group 1–16 interrogation;
- all-counter and group 1–4 interrogation with read, freeze, freeze-and-reset, and reset qualifiers;
- IOA read, clock synchronization, process reset, and test command;
- manual STARTDT/STOPDT and TESTFR.

Parameter ASDUs do not have a complete send workflow.

![Master counter-interrogation and freeze parameters](../../assets/screenshots/master-counter-interrogation.png)

*Counter interrogation selects all counters or groups 1–4 together with read, freeze, freeze-and-reset, or reset.*

### Controls

Supported commands include single, double, regulating step, normalized/scaled/short-float setpoints, and 32-bit bitstring, including corresponding CP56Time2a variants.

- **Automatic SBO** sends Execute after a confirmed Select.
- **Manual mode** exposes Select, Execute, and cancel separately.
- **Direct mode** sends Execute only.

The Master dispatch mode must match the Slave SBO/direct configuration. Use confirmation, termination, timeout, and captured frames as the result.

![Completed automatic SBO control workflow](../../assets/screenshots/master-control-sbo.png)

*Automatic SBO exposes Select, Confirm, Execute, and Finish. A point-value change alone is not sufficient evidence of command success.*

### Point tables and file transfer

Master imports CSV, JSON, and XML point tables. Append-only rejects the entire batch on any existing IOA; replace-all affects only the selected logical Slave. Point-table export is unavailable.

![Point-table preflight detecting append-only IOA conflicts](../../assets/screenshots/point-table-import-preview.png)

*This example intentionally shows IOA conflicts. Preflight reports errors and the previous point count before writing, and displays at most 200 preview records.*

After STARTDT, Master can fetch a remote directory, query logs by name and time, upload, download, or cancel the current transfer. One task may run per connection. Transfer is single-section and its limit is derived from maximum ASDU size.

![Remote file directory and download parameters](../../assets/screenshots/file-transfer.png)

*File transfer requires completed STARTDT. Select a remote entry and local destination before starting a download.*

## Slave operations

### Lifecycle and behavior policy

Stopping listening shuts down background tasks and freezes runtime values while retaining simulation configuration and the point selection for the next start.

The mismatch policy applies only when a command ASDU type differs from the point-table type; it is not a whole-stack compatibility level:

- **strict** rejects with a negative confirmation;
- **compatible** executes;
- **debug** executes and records detailed warnings.

Each logical Slave independently uses SBO or direct controls. An SBO selection expires after the configured timeout.

### Point table and command mapping

Create points from templates, edit them manually, or import CSV/JSON/XML. Store `control_ioa` on the monitoring point with the corresponding control IOA. Points may also join station/group 1–16 interrogation and all/group 1–4 counter interrogation.

![Slave command-to-monitor point mapping](../../assets/screenshots/slave-command-mapping.png)

*The control point is the source and the monitoring point is the target. Batch fill populates only compatible targets that are still unmapped.*

### Command handling and simulation

Slave handles link control, interrogation, counter interrogation, common controls/setpoints, bitstring, read, clock sync, reset, test command, and file transfer. Parameter ASDUs are not processed as a parameter-setting workflow.

Simulation provides 15 type-constrained modes: fixed, step, pulse, square, rising/falling saw, triangle, sine, ramp-hold-fall, stair, exponential approach, damped oscillation, random, random walk, and counter.

- Automatic upload requires a changed value, enabled automatic upload, and a peer that completed STARTDT.
- Force upload also requires an active data-transfer connection.
- Avalanche testing flips binary indications; it is not network fault injection.
- SOE is generated only when **SOE Upload** is enabled for the logical Slave, the peer has completed STARTDT, and a point value changes. A control-triggered SOE additionally requires the monitoring point's `control_ioa` to map to that control point. With SOE Upload disabled, ordinary spontaneous data is sent without a timestamp and is not added to either SOE panel. There is no analog threshold configuration.

| Simulation parameters and waveform preview | Active simulation state |
| --- | --- |
| ![Step waveform configuration in Slave](../../assets/screenshots/slave-simulation.png) | ![Point values while Slave simulation is running](../../assets/screenshots/slave-simulation-running.png) |

Once simulation starts, the toolbar and point table show the stop action and running markers. Stopping the Listener stops simulation first, so values and cursors should not continue changing after stop completes. Avalanche testing has no separate dialog; verify it through the notification, point values, and captured frames.

## Frames, SOE, and parser

The communication monitor supports search, direction/slave/type filters, table/tree details, and CSV/TXT/PCAP/PCAPNG export. The SOE panel supports viewing, filtering, and clearing but has no independent CSV export.

![CP56-timestamped SOE generated after a mapped control changes a monitoring point](../../assets/screenshots/soe-panel.png)

*After control execution, the mapped monitoring point is uploaded as Type 30 with COT 3 and appears in the SOE panel.*

| Communication monitor | Parsed frame tree |
| --- | --- |
| ![Communication monitor with TX and RX frames](../../assets/screenshots/message-monitor.png) | ![Parsed APCI and ASDU tree](../../assets/screenshots/message-tree.png) |

**Tools → Message Parser** parses one complete APDU at a time, including the `68` start byte and APCI.

## Troubleshooting

### Cannot connect

Match the Master Link and Slave Listener address, start the Listener, confirm the port is free, and check Windows Firewall. Connect only to authorized test networks.

### TCP connects but no data appears

Complete STARTDT, match the logical Slave common address on both sides, send station interrogation, and inspect the communication monitor.

### A control is rejected

Check the IOA, expected ASDU type, Master dispatch mode, and Slave SBO/direct configuration. A strict type mismatch returns a negative confirmation.

### Simulation changes locally but is not uploaded

Confirm that the Listener is running, the peer completed STARTDT, and automatic upload is enabled. Force upload also requires an active data-transfer connection.

### Point-table import fails

Use CSV, JSON, or XML. Put standard type names in the CSV `data_type` field. Append-only rejects the entire batch if any IOA already exists.
