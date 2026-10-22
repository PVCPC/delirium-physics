# Changelog

## 0.2.0

### New
- **Portal mode**: portals, the portal gun, the opening animation, particles and the air funnel, ported from Stink-O's
  Portal 2 Rust Engine. It reads Portal 2's files from your own install; none are included.
- **Companion cube**: with Portal mode on, thrown boxes are the companion cube.
- **Portal sounds**: the portal gun's shots and portals opening play Portal 2's sounds (from your install).
- **Experimental walk** (preview, this release only): a learned walk, fast walk and jog. Turn it on under Character →
  Experimental movement, then hold W to walk and Shift+W to jog (it ramps up over about 6 seconds). It walks in a
  straight line, stops by itself at walls, and slows to a walk before stopping when you let go. Trained on Ubisoft's
  LAFAN1 motion capture (non-commercial; see CREDITS.md). It will be replaced next release.
- **Audio sliders**: master, effects and portal volume under View → Audio.
- **Docs**: [How it works](docs/how-it-works.md) and [Modding](docs/modding.md), covering quick starts for each
  language, every message and its parameters, Playroom scripting and settings.

### Changed
- **Shotguns** fire their full pellet spread, each pellet its own hit, on a click and from the floating gun. A headshot
  or a point-blank chest blast kills.
- **Weapons panel** is grouped (Guns, Thrown, Melee, Portal 2) with a stats line for each weapon.
- **Floating portal gun** aims at the wall under the pointer and no longer locks onto Homer.
- **Near misses**: Homer flinches and keeps his balance instead of collapsing.
- **Shadows** have a wider softness slider.
- **Faster physics**: about 1.4x faster steps with the same results, and about 1.36x more on Windows and Linux x86
  builds (AVX2+FMA).
