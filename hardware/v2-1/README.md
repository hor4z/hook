# Hook v2.1 — reference refinement

Visual refinement of v2, preserving the earlier version. Nominal outer envelope remains **64 × 100 × 34 mm**, plus the small button protrusion. One Blender coordinate unit and one STL coordinate unit represent one millimeter.

## Changes

- Rounder shoulders and base, using a softer footprint profile.
- Asymmetric recessed front panel with a thicker lower bumper.
- Lower opening moved to match the reference proportions.
- Continuous curved transitions into the front recess.
- Rear shell with a rounded cross-section and a smoothly shaded central surface.
- Rear grille proportions and position revised.
- Separate top button, clear of the front opening.
- USB-C metal rim, dark insert, tongue and indicator lens as visual details.
- Matte orange material with subdued specular reflection and fine shader-only surface texture.
- Alignment lip now connected to the rear shell through its root.

## Files

- `hook-device-v2-1.blend`: editable v2.1 scene, with the earlier scenes retained.
- `renders/overview.png`: six views of the refinement.
- `renders/comparison.png`: v2 versus v2.1 front views.
- `V21_*_VISUAL.stl`: front, back and button geometry for shape evaluation.
- `validation.json`: connected-component, edge-manifold and volume checks.

This is still a visual prototype. The connector details and graphics are not printable shell components. Module retention, cable routing, button travel, acoustic paths and screwless latches still require mechanical development and calibration with the actual hardware and PLA on the Bambu Lab A1. Render roughness is not a guarantee of the finish of a particular filament.

## Rebuild

```sh
blender --background hardware/v2/hook-device-v2.blend --python hardware/v2-1/build.py
python3 hardware/v2-1/contact_sheet.py
```

`build.py` uses the existing v1 geometry helpers and the repository SVG logo. `refine_surface.py` cleans sub-micron boolean artifacts and provides smooth normals for the rear shell. `finalize.py` applies these finishing steps to an already generated v2.1 scene.
