# Stock-client activation receipt

Captured 2026-09-13 with stock NomadNet 1.4.2 on an isolated Reticulum TCP
server/client pair. The server listened only on `127.0.0.1:42540`; no radio,
public network, third-party destination, or real credential was used.

The node destination `fee391eb08d160cc4028d33154da58ea` was derived from the
node's task-local identity with the public RNS `Destination` API. The stock
client's Network URL dialog loaded these exact addresses:

```text
fee391eb08d160cc4028d33154da58ea:/page/index.mu
fee391eb08d160cc4028d33154da58ea:/page/text.mu
fee391eb08d160cc4028d33154da58ea:/page/all-submit.mu
fee391eb08d160cc4028d33154da58ea:/page/selected-submit.mu
```

The client rendered `index.mu`, activated its `:/page/linked.mu` link by
keyboard, and rendered the linked page. This proves same-node `:/page/...`
resolution for the live stock client. Bare and relative destinations have only
parse-retention evidence and remain unqualified.

The controlled dynamic endpoint was exactly `:/page/capture.mu`. It writes a
JSON line containing only explicitly named dummy fields or environment names
containing the unique controlled marker `mnprobe`; it never dumps inherited
environment variables. The captured bytes are in
[`logs/request-record.jsonl`](logs/request-record.jsonl).

| UI source and link selector | Exact controlled callback data |
| --- | --- |
| `/page/text.mu` with `mnprobe_text` selector | `field_mnprobe_text=seed-text` |
| `/page/all-submit.mu` with `*` | `field_mnprobe_text=seed-text`; `field_mnprobe_empty=`; `field_mnprobe_mask=mask-seed`; `field_mnprobe_checks=red,blue`; `field_mnprobe_radio=red`; `field_mnprobe_multiline=` |
| `/page/selected-submit.mu` with `mnprobe_text|mnprobe_checks|mnprobe_radio|mnprobe_fixed=ready` | `field_mnprobe_text=seed-text`; `field_mnprobe_checks=red,blue`; `field_mnprobe_radio=red`; `var_mnprobe_fixed=ready` |

Observed contract for this runtime:

- Selected submitted fields use `field_<field-name>` environment variables.
- Fixed `key=value` selector entries use `var_<key>`.
- `*` includes empty text and multiline fields as empty values, includes masked
  fields by their actual initial value, concatenates checked duplicate checkbox
  values with a comma, and selects the active radio value.
- Selected names exclude unlisted dummy fields. The fixed variable is included
  only when declared in the link selector.
- The dynamic script did not receive `PATH_INFO` or `REQUEST_PATH` in this
  runtime. Endpoint identity is established by execution of the exact requested
  `capture.mu` script, not by those absent variables.

The multiline field was captured only while empty. Keyboard population of its
multi-row editor and the byte representation of a nonempty newline remain open.
The user can edit a text field in the client, but this receipt intentionally
uses its declared dummy defaults only.

## Commands

```text
nomadnet --daemon --console \
  --config /mnt/c/t/micron-reference-20260912/activation/node/nomadnet \
  --rnsconfig /mnt/c/t/micron-reference-20260912/activation/node/rns

nomadnet --textui \
  --config /mnt/c/t/micron-reference-20260912/activation/client/nomadnet \
  --rnsconfig /mnt/c/t/micron-reference-20260912/activation/client/rns
```

The UI client ran in a task-owned tmux session solely to capture the terminal
screen and send keyboard activation. The local node process was stopped after
capture.
