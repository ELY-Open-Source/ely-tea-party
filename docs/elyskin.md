# The `.elyskin` format

Version 2, 2026-10-31. This is the contract between Ely Tea Party, the people who make character packs, and the skin market. The words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119. Where this text and the app disagree, file an issue: the app's reader, `src/skins/pack.rs`, is the reference implementation, and `docs/elyskin.schema.json` is the machine-readable form of the structural rules below.

## File

- Extension `.elyskin`. Served as `application/json`, with `Content-Disposition: attachment` so browsers save it.
- One JSON object, UTF-8, without a byte order mark. At most 44 MiB.
- No key appears twice in any object. Readers MUST refuse a document with a repeated key in `images`; the app refuses one in any object it knows.

## Document

```json
{
  "format": "elyskin",
  "version": 2,
  "target": "claude",
  "meta": {
    "id": "tester.blue-otter",
    "name": "Blue Otter",
    "author": "Tester",
    "description": "An otter in a diving suit.",
    "license": "CC-BY-4.0",
    "homepage": "https://example.com/blue-otter"
  },
  "layout": { "scale": 1.0, "x": 0.0, "y": 0.0 },
  "images": {
    "idle": "BASE64",
    "working": "BASE64"
  }
}
```

| Field | Required | Rule |
| --- | --- | --- |
| `format` | yes | The string `"elyskin"`. |
| `version` | yes | The integer `2`. |
| `target` | yes | The character the pack was made for: `whale` for the mascot, otherwise an agent id. Ids this version knows: `claude`, `gpt`, `deepseek`, `grok`, `kimi`, `gemini`, `pi`, `opencode`. Any id matching `^[a-z][a-z0-9_-]{0,31}$` is valid. |
| `meta` | yes | The pack's credits, below. |
| `layout` | no | The mascot's placement, below. Agents ignore it. |
| `images` | yes | Actions to images, below. Must hold `idle`. |

### `meta`

| Field | Required | Rule |
| --- | --- | --- |
| `name` | yes | 1 to 64 characters, no leading or trailing whitespace. |
| `author` | yes | 1 to 64 characters, no leading or trailing whitespace. |
| `id` | no | 3 to 64 of `a-z`, `0-9`, `.` and `-`, starting and ending with a letter or digit. A stable identity for updates; the market MAY assign it and SHOULD prefix it with the uploader's handle. |
| `description` | no | Up to 500 characters; line feeds allowed. |
| `license` | no | Up to 64 characters of an SPDX license expression: letters, digits, space, `.`, `+`, `-`, `(` and `)`. |
| `homepage` | no | An `https://` URL of at most 200 bytes, without whitespace. |

Characters are Unicode scalar values. No text field may hold a control character (Unicode category Cc; `description` excepts U+000A) or a bidirectional control: U+061C, U+200E, U+200F, U+202A to U+202E, U+2066 to U+2069. Readers show these fields as plain text, never as markup or links they open without asking.

### `layout`

`scale` from 0.5 to 1 (default 1), `x` and `y` from -30 to 30 points (default 0). The mascot is drawn in a 112-point square at `scale`, moved by `x` and `y`; a positive `y` hides part of it below the edge, for a peeking pose.

### `images`

Keys are action names: 1 to 32 of `a-z`, `0-9`, `_` and `-`. At most 64 entries. Values are standard base64 with padding and no line breaks (RFC 4648 §4).

| Target | Actions |
| --- | --- |
| an agent | `idle`, `working`, `working-research`, `working-special`, `working-slack`, `thinking`, `needs_input`, `sleeping` |
| `whale` | `idle`, `calm`, `working`, `working-special`, `working-slack`, `thinking`, `needs_input`, `sleeping`, `walking`, `walking-left`, `puzzled-up`, `puzzled-down` |

`idle` is required. A missing action shows `idle`, never the built-in art.

Each image is PNG, APNG, GIF or WebP (still or animated) and must:

- be at most 8 MiB encoded and 2048 × 2048 pixels;
- hold at most 240 frames and at most 128 MiB of frame pixels in all;
- together with the pack's other images, come to at most 32 MiB encoded.

Frame delays under 20 ms play at 20 ms. Images over 224 × 224 are scaled down for display; the pack keeps the original bytes.

## Reading

A reader of version 2:

1. MUST refuse a document whose `version` is greater than the newest it reads, saying an update is needed, and SHOULD say so in the user's language.
2. MUST ignore fields it does not know, at any level, so later minor additions do not break it.
3. MUST skip, not refuse, actions it does not know whose names are valid, and SHOULD tell the user which were skipped.
4. MUST refuse an action name that breaks the naming rule, a repeated key in `images`, missing `idle`, and any image or text field that breaks a rule above. A refused pack changes nothing already installed.
5. Treats every agent pack as fit for every agent, whatever its `target`, including ids it does not know. A mascot pack fits only the mascot, and an agent pack never fits it.

## Writing

A writer MUST emit only the fields of the version it writes, plus extension fields whose names start with `x-`, which readers ignore. Unprefixed fields are reserved for later versions. The app writes keys in the order shown above and `images` in key order, so the same skin exports to the same bytes.

## Versions

- A field or action added with a safe default keeps version 2: older readers ignore or skip it.
- A change older readers would misread raises `version` to 3.
- Version 1, written by Ely Tea Party 0.3.12 to 0.3.16, had no `format` or `meta`, and its readers refuse any field or action they do not know. Ely Tea Party 0.3.17 and later read version 1 as those versions did, and still save a skin without credits as version 1 in its own settings folder, so a downgrade keeps it. Every exported pack is version 2, which needs 0.3.17 or later.

## For the market

The market SHOULD accept only version 2 uploads that pass `docs/elyskin.schema.json` and every rule above that a schema cannot state: decoded image formats and limits, the 32 MiB total, repeated keys and the 44 MiB document size. It SHOULD record each pack's SHA-256, serve the uploaded bytes unchanged, and state that version 2 packs need Ely Tea Party 0.3.17 or later. A pack is data: it holds no scripts or file paths, and the app never runs or follows anything in it.
