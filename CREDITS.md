# Credits

- **Taxi model** (`crates/delirium-room/src/taxi.glb`): "Fairheaven LT '80 Taxi - Low poly model" by Daniel Zhabotinsky
  (Sketchfab), licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Changed: converted from USDZ to
  glTF, turned to face +Y with Z up, wheels split out and named, origins moved to the wheel centres.
- **Photo sky** (`crates/delirium-room/src/sky.ktx2`): "Kloofendal 48d Partly Cloudy (Pure Sky)" by Greg Zaal
  ([Poly Haven](https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky)), [CC0](https://creativecommons.org/publicdomain/zero/1.0/). Changed: made a
  1024-pixel cube, its sun cut out.
- **Path tracing test textures** (`crates/delirium-room/src/pbr/`: cobblestone, ice, grass and the stained glass): from the
  [Faithful](https://faithfulpack.net) resource packs by the Faithful Resource Pack team, used under the
  [Faithful license](https://faithfulpack.net/license). Changed: converted to mipmapped KTX2, normal maps flipped to OpenGL.
- **Font** (`crates/delirium-room/src/archivo-black.ttf`): Archivo Black, Copyright 2017 The Archivo Black Project
  Authors (https://github.com/Omnibus-Type/ArchivoBlack), licensed under the
  [SIL Open Font License 1.1](https://openfontlicense.org). Unchanged.
- **Portal maths** (`crates/delirium-room/src/portal.rs`): by **Stink-O**, from their
  [Portal 2 Rust Engine](https://github.com/Stink-O/Portal2-rust-engine), the original Portal 2 rewrite in Rust. The
  portal-pair maths (`p2_core/src/math.rs`) is cut down from it. Licensed under the MIT License (full text below).
- **Portal mode rendering, placement, effects and funnel** (`crates/delirium-room/src/portal/`,
  `ghost_clip.wgsl`, `p2vtf.rs`): ported from **Stink-O**'s [Portal 2 Rust Engine](https://github.com/Stink-O/Portal2-rust-engine)
  and adapted to this room's Z-up world and Bevy 0.18: the portal opening animation and the recursive view tree (`p2_render`), the placement rules and shot trace (`p2_gun/rules.rs`), the ghost
  clip shader (`p2_body/clip.wgsl`), the air funnel (`p2_movement/funnel.rs`), the VTF reader and the VMT/KeyValues
  loaders (`source_assets`), and the particle system reader and simulator (`p2_fx`). The portal surface shader (`portal_surface.wgsl`) and the
  particle card shader (`portal/fx/spritecard.wgsl`) are this project's own.
- **Experimental walk** (`learned-preview/`, `crates/delirium-room/src/locomotion/`): its walk, fast walk and jog
  networks and the three motion clips they follow were made from Ubisoft La Forge's
  [LAFAN1](https://github.com/ubisoft/ubisoft-laforge-animation-dataset) motion capture (walk1_subject1,
  run1_subject2), licensed under [CC BY-NC-ND 4.0](https://creativecommons.org/licenses/by-nc-nd/4.0/): non-commercial
  use only. Changed: retargeted to Homer's body. A preview for this release only. Not affiliated with or endorsed by
  Ubisoft.
- **FSR 1 upscaler** (`crates/delirium-room/src/fsr1.wgsl`): EASU and RCAS ported from AMD FidelityFX Super Resolution 1
  ([FidelityFX-FSR](https://github.com/GPUOpen-Effects/FidelityFX-FSR)), Copyright (c) 2021 Advanced Micro Devices, Inc.,
  MIT licence (in the file). Not affiliated with or endorsed by AMD.
- **Anti-aliasing and DLSS** (`vendor/`): Bevy's bevy_anti_alias and dlss_wgpu (MIT / Apache-2.0), changed to draw into
  part of the window. DLSS is NVIDIA's; its runtime (nvngx_dlss.dll) comes under NVIDIA's licence.
- **Ray tracing** (`vendor/bevy_solari`): Bevy's bevy_solari (MIT / Apache-2.0), changed to draw into part of the window,
  to trace only the room's half, and to restart the path traced picture only when the camera moves. DLSS Ray
  Reconstruction's runtime (nvngx_dlssd.dll) comes under NVIDIA's licence.
- **Shadows and ambient occlusion** (`vendor/bevy_pbr`): Bevy's bevy_pbr (MIT / Apache-2.0), changed for soft shadows sized
  by the sun and for ambient occlusion in part of the window.
- **FSR 3 upscaler** (`crates/delirium-room/src/upscale.rs`): calls AMD FidelityFX SDK's Vulkan runtime
  (amd_fidelityfx_vk.dll, [FidelityFX SDK](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK)), Copyright (c)
  Advanced Micro Devices, Inc., MIT licence. Not affiliated with or endorsed by AMD.
- **XeSS upscaler** (`crates/delirium-room/src/upscale.rs`): calls Intel's XeSS runtime (libxess.dll,
  [XeSS](https://github.com/intel/xess)), Copyright (c) Intel Corporation, Intel Simplified Software License. Not
  affiliated with or endorsed by Intel.
- **Portal 2** is Valve's. None of its files are included: portal mode reads them from your own install. Not affiliated
  with or endorsed by Valve.

## Portal 2 Rust Engine licence

```
MIT License

Copyright (c) 2026 Stink-O

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
