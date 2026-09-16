# Artwork

Drop image files in here. Trunk copies the whole directory into the bundle, so
a file saved as `assets/brick.svg` is served at `/assets/brick.svg` and can be
used from the UI as `<img src="/assets/brick.svg"/>`.

## Format

**SVG for anything symbolic** - resource marks, ships, harbour badges, dev-card
faces. It scales to any size without going soft, weighs almost nothing, and can
be recoloured from CSS.

**PNG for anything painted** - hex textures, backgrounds, a title illustration.
Export at twice the size it will be displayed. Not JPEG: it has no
transparency and smears hard edges.

## Exporting SVG

From Inkscape, Illustrator, Figma or Affinity:

- Save as **plain SVG** (Inkscape: "Plain SVG", not "Inkscape SVG").
- **Convert text to paths / outlines.** A font that is not on the player's
  machine renders as nothing.
- Flatten the layers and remove hidden objects and guides.
- Crop the canvas to the artwork - no surrounding whitespace - and use a
  square canvas for anything meant to sit in a square slot.
- Keep it to a handful of paths. Traced photos produce thousands and are
  slower to draw than a PNG would be.

### Single-colour marks

For a mark meant to be tinted by the UI - a knight, a road, a ship - draw it in
one colour and leave the fill as `currentColor`, or just use black and say so:
it can be swapped in the file. That is what lets one icon appear in each
player's colour without exporting six copies.

For a resource card face, colour is part of the artwork, so paint it however it
should look.

## Naming

Lower case, hyphens, no spaces. The UI looks for these names:

    brick.svg  wood.svg  sheep.svg  wheat.svg  ore.svg
    port-generic.svg  port-brick.svg  port-wood.svg
    port-sheep.svg    port-wheat.svg  port-ore.svg
    dev-knight.svg  dev-victory.svg  dev-road.svg
    dev-monopoly.svg  dev-plenty.svg
    robber.svg

Sizes are up to you; the UI scales them. Around 64x64 for a resource mark and
128x128 for a card face is comfortable.
