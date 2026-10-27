# Character skins

Open **Settings → Characters** to customize the eight agent characters and the whale mascot independently. Agent detection, hooks and status transitions keep working with the new art.

- **Replace appearance** imports one PNG, APNG, GIF or WebP. Every action initially uses this image or animation.
- **Replace this action** changes the selected action after a custom character has been imported. Use the action picker to preview each animation.
- **Import skin** loads a complete `.elyskin` file. Missing actions use that skin's `idle` image, so the character never switches back to built-in art during a missing action.
- **Export skin** saves a portable `.elyskin` containing the images and placement settings. Built-in characters can also be exported as starting templates. Agent packs can be assigned to any agent; the mascot uses a separate action set.
- **Restore default** removes the selected character's custom skin. It does not change other characters or the original imported files.

Changes apply immediately. Imported files are copied into the app's configuration directory under `characters/`; moving or deleting the original files does not break the character. The last successful skin stays active if a new import fails. Installation and export results appear on the settings page.

Custom agent skins hide the built-in character's accessory overlay. The app icon and Dock artwork remain application branding.

## Mascot placement

The mascot keeps its existing walking, dragging, waiting and sleeping behavior. A missing walking or looking animation uses the custom idle image.

The preview shows the image inside a 112-point square. Size ranges from 50% to 100%; horizontal and vertical offsets range from -30 to 30 points. These settings are included in exported mascot packs. Transparent images with consistent framing across actions work best. Positive vertical offsets place part of the character below the visible area, for a peeking pose.

## Supported actions

| Slot | Actions |
| --- | --- |
| `claude`, `gpt`, `deepseek`, `grok`, `kimi`, `gemini`, `pi`, `opencode` | `idle`, `working`, `working-research`, `working-special`, `working-slack`, `thinking`, `needs_input`, `sleeping` |
| `whale` | `idle`, `calm`, `working`, `working-special`, `working-slack`, `thinking`, `needs_input`, `sleeping`, `walking`, `walking-left`, `puzzled-up`, `puzzled-down` |

`idle` is required in every pack. Other actions are optional.

## Limits

- Each source image: up to 8 MiB, 2048 × 2048 pixels and 240 animation frames.
- A pack: up to 32 MiB of encoded image files; the JSON document can be up to 44 MiB including base64 overhead.
- Decoding one image: up to 128 MiB of cumulative source frame pixels before resizing.
- Retained animation frames: up to 128 MiB per character and 256 MiB across all active custom characters.
- Images larger than 224 × 224 are reduced proportionally for display. Smaller images are not enlarged in the decoded cache. Transparency and animation timing are preserved; delays shorter than 20 ms are clamped to 20 ms.

Validation and decoding run in the background when importing. Files are replaced atomically after validation. On startup, damaged saved packs are reported in logs and on the Characters page, and the affected slot shows its built-in character until repaired or reset.

## Pack format

A `.elyskin` is a UTF-8 JSON file. It embeds image bytes directly, so it contains no executable scripts or filesystem references. Exporting an existing character is the easiest way to create a template; individual actions can then be replaced in the settings UI.

```json
{
  "version": 1,
  "target": "claude",
  "layout": { "scale": 1.0, "x": 0.0, "y": 0.0 },
  "images": {
    "idle": "BASE64_ENCODED_IMAGE_BYTES",
    "working": "BASE64_ENCODED_IMAGE_BYTES"
  }
}
```

`layout` is optional and defaults to the values above. It affects the whale mascot. Unknown fields, unsupported versions, unknown actions and malformed images are rejected. Exported packs retain the original image bytes; resizing is only applied to the display cache.
