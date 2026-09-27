# Koon v2 — visual prototype

First visual review requested before mechanical refinement. Built with Blender MCP, using its background execution mode after the interactive connection became unavailable. The v1 files are unchanged.

Proposed outer envelope: **64 × 100 × 34 mm**, excluding the small button protrusion. Blender uses one coordinate unit per millimeter (`scale_length = 0.001`). STL coordinates are millimeters.

## Review files

- `koon-device-v2.blend`: editable scene, separate front, back and top button, logo graphics, studio and hidden hardware envelopes. The source v1 scene is also retained in this file.
- `renders/overview.png`: visual comparison sheet.
- `renders/`: front, back, side, top, perspective and exploded views.
- `V2_*_VISUAL.stl`: three closed exterior prototype meshes, for evaluating physical size and shape only.
- `validation.json`: edge-manifold and volume checks for these STL meshes.

## Modeled

Asymmetric rounded front and back, recessed front panel, four rounded openings, white logo and dots, rear vertical-slot grille, side USB-C and indicator openings, switch opening, separate top button and perimeter alignment lip. The logo and white dots are surface graphics, not included in the shell STL files.

## Mechanical work remaining

This is not the final printable assembly. The alignment lip has a provisional 0.25 mm radial clearance; it does not yet provide snap retention. Module envelopes are placeholders, hidden by default, and have not passed a collision or cable-routing check. PCB carriers, microphone acoustic path, speaker retention, battery retention, button travel/stops and calibrated screwless latches remain to be developed after visual review. A closed mesh alone does not validate wall thickness, fit, strength or print orientation.

Target printer/material supplied by the user: Bambu Lab A1, PLA. Nozzle diameter has not been confirmed. Snap geometry and clearance must be calibrated with an actual PLA test coupon before printing the final enclosure. Do not scale the finished enclosure to fit hardware; revise its dimensions and supports instead.

## Rebuild

Run from the repository, with Blender available:

```sh
blender --background hardware/v1/koon-device.blend --python hardware/v2/build.py
```

`build.py` reuses geometric helpers from `../v1/build.py` and the repository SVG logo. `finish.py` creates the final material and render views. Rebuild into a fresh process; it creates a new scene and does not modify v1 on disk.
