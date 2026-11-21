use super::*;

pub fn page(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, page: u8) {
    match page {
        0 => graphics(ui, sim),
        1 => water(ui, sim),
        2 => display(ui, sim),
        3 => audio(ui, sim, pressed),
        _ => camera(ui, sim, pressed),
    }
}

fn reset_graphics(ui: &mut egui::Ui, sim: &mut Sim) {
    ui.add_space(8.0);
    if ui.button("Reset graphics & water").on_hover_text("Lights, shadows, effects and water back to how they start.").clicked() {
        reset_settings(sim, &["shadows", "shadow_quality", "sun_size", "soft", "fill", "sun_r", "sun_g", "sun_b", "ssao", "ao_strength", "bloom", "dof", "contact", "contact_strength", "vignette", "lens", "chroma", "fire", "real_fire", "smoke", "water_quality", "voxel_water", "photon_caustics", "caustics", "spray", "waves", "pixels", "haze", "mirror", "foam", "mist", "wet", "wet_film", "wet_drips", "sky_shade", "traced_water", "glass_tint", "glass_shadows", "water_ripples", "water_refraction", "finer_caustics", "depth_colour", "keep_volume", "ghost_surface", "vorticity", "spray_physics", "water_surface", "fine_ripples", "clear_spill", "wet_trail", "impact_foam", "faucet", "fire_light", "fire_homer", "shadow_filter"]);
        sim.sun_dir = SUN_DIR;
    }
}

fn reset_av(ui: &mut egui::Ui, sim: &mut Sim) {
    ui.add_space(8.0);
    if ui.button("Reset display & sound").on_hover_text("Graphics card, display, anti-aliasing and sound back to how they start.").clicked() {
        reset_settings(sim, &["render_scale", "msaa", "gpu", "aa", "smaa", "dlss_mode", "sharpen", "sharp_by", "fps_limit", "upscaler", "fsr_mode", "xess_mode", "sound", "vol", "vol_fx", "vol_portal", "vol_amb", "amb_theme", "vol_body"]);
    }
}

fn pct(x: f64) -> String {
    if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }
}

fn graphics(ui: &mut egui::Ui, sim: &mut Sim) {
    head(ui, "Graphics");
    info(ui, &format!("{:.0} FPS ({:.1} ms a frame). (GPU: ...) is what a setting costs a slow graphics card; lower Graphics quality first, Resolution (Display) last.", 1000.0 / sim.frame_ms.max(0.1), sim.frame_ms));
    card(ui, |ui| {
        let custom = preset(sim);
        let mut q = custom.unwrap_or(1) as f32;
        if ui.add(egui::Slider::new(&mut q, 0.0..=2.0).step_by(1.0).text("Graphics quality")
            .custom_formatter(|x, _| if custom.is_none() { "custom".into() } else { ["Low", "Medium", "High"][x as usize].into() }))
            .on_hover_text("Sets edge smoothing, shadow quality, the sun's size, fire and water together. Low: off, no shadows, wireframe fire, plain water. Medium: 4x, medium, a point sun, flames + light, rippling water. High: 4x, high, a 1° sun, flames + light on Homer, ripples and refraction.").changed() {
            let (m, s, soft, fire, water) = VIDEO_PRESETS[q as usize];
            sim.set_fire(fire);
            sim.water_q = water;
            sim.water_traced = water >= 3;
            (sim.water_ripples, sim.water_refract, sim.photons, sim.water_fine, sim.water_depth) = (water >= 1, water >= 2, water >= 2, water >= 4, water >= 1);
            (sim.water_relax, sim.water_ghost, sim.water_vort, sim.water_spray_phys) = (water >= 2, water >= 2, water >= 3, water >= 3);
            (sim.aa, sim.shadows, sim.shadow_q) = (if m == 1 { 0 } else { AA_MSAA }, s > 0, s.max(1));
            sim.msaa = m.max(2);
            sim.shadow = if !soft { 0.0 } else if sim.shadow > 0.0 { sim.shadow } else { REAL_SUN };
        }
    });
    fold(ui, "Renderer", true, |ui| {
        let mut q = sim.rt as f32;
        if ui.add_enabled(sim.rt_ok, egui::Slider::new(&mut q, 0.0..=2.0).step_by(1.0).text(gpu("Renderer", 3)).custom_formatter(|x, _| ["raster", "ray traced", "path traced"][x as usize].into()))
            .on_hover_text("Raster: the usual game picture, fast (shadow maps, ambient occlusion). Ray traced: light and shadows traced as rays (Bevy's Solari), bouncing off the walls and floor, in real time; with an RTX card DLSS's Ray Reconstruction cleans up the noise (anti-aliasing turns to DLAA if it was below it), other cards are grainy. Path traced: the true picture, every bounce of light traced; with an RTX card DLSS's Ray Reconstruction cleans it up as it goes (as games' path tracing modes do), so it holds up while the camera moves; other cards build it up over a few seconds while the camera holds still.")
            .on_disabled_hover_text("Needs a graphics card that can ray trace (RTX, RX 6000 or newer).").changed() {
            sim.rt = q as u8;
        }
        switch_if(ui, sim.rt_now() == 1, &mut sim.restir, &gpu("ReSTIR", 2), "Ray-traced lighting reuses light paths between pixels and frames: finer shadows and steadier light, for a good chunk of speed.",
                  "Ray traced only: path tracing traces every light path itself and has no use for it.");
    });
    fold(ui, "Lights & shadows", true, |ui| {
        let mut f = if sim.soft { sim.fill_k } else { 0.0 };
        if ui.add(egui::Slider::new(&mut f, 0.0..=3.0).text("Fill light").custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{x:.2}×") }))
            .on_hover_text("How bright the shadows and the dark side are: off is near black, high is flat and washed out.").changed() {
            (sim.soft, sim.fill_k) = (f > 0.0, if f > 0.0 { f } else { sim.fill_k });
        }
        let mut q = if sim.shadows { sim.shadow_q as f32 } else { 0.0 };
        if ui.add(egui::Slider::new(&mut q, 0.0..=3.0).step_by(1.0).text(gpu("Shadow quality", 3)).custom_formatter(|x, _| ["off", "low", "medium", "high"][x as usize].into()))
            .on_hover_text("Low: a coarse shadow near the camera, crisp but jagged. Medium: twice the detail, smoothed, further out. High: twice that again, out to the horizon; with TAA, DLAA, DLSS, FSR 3 or XeSS it smooths over frames (no grain on soft shadows). Shadows are the biggest cost after resolution.").changed() {
            (sim.shadows, sim.shadow_q) = (q > 0.0, (q as u8).max(1));
        }
        ui.add_enabled(sim.shadows, egui::Slider::new(&mut sim.shadow, 0.0..=SUN_SIZE_MAX).logarithmic(true).smallest_positive(0.25).text(gpu("Sun size", 2)).custom_formatter(|x, _| if x <= 0.0 { "point (hard)".into() } else { format!("{x:.1}°") }))
            .on_hover_text("How big the sun is. Like real shadows, they stay sharp where they touch and soften further away; a bigger sun softens more. The real sun is about 0.5°, a hazy sky 2–5°; a point is sharp everywhere. Costs more on a slow graphics card; grainy without TAA, DLAA, DLSS, FSR 3 or XeSS.");
        if ui.add_enabled(sim.shadows && (sim.shadow - REAL_SUN).abs() > 0.005, egui::Button::new("Real sun")).on_hover_text("Sun size as the real sun's, 0.53°: sharp shadows near, a little soft far off.").clicked() {
            sim.shadow = REAL_SUN;
        }
        let mut f = sim.shadow_filter as f32;
        if ui.add_enabled(sim.shadows, egui::Slider::new(&mut f, 0.0..=3.0).step_by(1.0).text("Shadow edges").custom_formatter(|x, _| ["match anti-aliasing", "soft", "smoothed over frames", "sharp"][x as usize].into()))
            .on_hover_text("Raster's shadow edge filter. Match anti-aliasing: sharp at Low shadows, else smoothed over frames with TAA, DLAA, DLSS, FSR 3 or XeSS and soft without. Soft: a blurred edge, steady. Smoothed over frames: soft without the grain, needs one of those methods. Sharp: crisp, jagged.").changed() {
            sim.shadow_filter = f as u8;
        }
        switch(ui, &mut sim.glass_tint, "Stained glass colour", "The path tracing test's stained glass coloured, in raster and the ray tracers (off: plain clear glass, clear light through it).");
        switch_if(ui, sim.glass_tint, &mut sim.glass_shade, "Coloured glass light", "The sun through the stained glass takes its colours onto the floor and walls, in raster and the ray tracers (off: the light through it clear, the glass still coloured).",
                  "With Stained glass colour.");
        info(ui, "The sun's colour, height and direction: Scene > Look.");
    });
    fold(ui, "Effects", true, |ui| {
        let mut q = sim.ssao as f32;
        if ui.add(egui::Slider::new(&mut q, 0.0..=4.0).step_by(1.0).text(gpu("Ambient occlusion", 2)).custom_formatter(|x, _| ["off", "low", "medium", "high", "ultra"][x as usize].into()))
            .on_hover_text("Darkens creases and corners where light can't reach, like under Homer's arms and where he meets the floor (SSAO). Grainy without TAA, DLAA, DLSS, FSR 3 or XeSS.").changed() {
            sim.ssao = q as u8;
        }
        ui.add_enabled(sim.ssao > 0, egui::Slider::new(&mut sim.ao_strength, 0.0..=2.0).text("Ambient occlusion strength").custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
            .on_hover_text("How dark ambient occlusion makes the creases. It darkens all the light there, the sun's too, as most games do.");
        let mut q = sim.fire as f32;
        if ui.add(egui::Slider::new(&mut q, 0.0..=FIRE_HIGH as f32).step_by(1.0).text(gpu("Fire", 1))
            .custom_formatter(|x, _| ["wireframe", "flames", "flames + light", "flames + light on Homer"][x as usize].into()))
            .on_hover_text("Wireframe: plain rings, for a slow PC. Flames: flames and embers, no light from them. Flames + light: they light the room. \
                            Flames + light on Homer: burning Homer glows and his flames light him from every side.").changed() {
            sim.set_fire(q as u8);
        }
        let fire = sim.fire > 0;
        switch_if(ui, fire, &mut sim.real_fire, "Realistic fire", "Rolling, glowing flames instead of voxels (cubes).", "With Fire above wireframe.");
        switch_if(ui, fire, &mut sim.smoke, "Fire smoke", "Smoke rising off the flames.", "With Fire above wireframe.");
        switch(ui, &mut sim.fire_light, "Fire light", "The flames light the room (the molotov's and burning fires'); off: they give no light.");
        switch(ui, &mut sim.fire_homer, "Light on Homer", "Burning Homer glows and his flames light him from every side (needs Fire light for the light round him).");
    });
    fold(ui, "Lens", false, |ui| {
        ui.add(egui::Slider::new(&mut sim.bloom, 0.0..=1.0).text(gpu("Bloom", 1)).custom_formatter(|x, _| pct(x)))
            .on_hover_text("A soft glow around bright things, like a camera lens.");
        ui.add(egui::Slider::new(&mut sim.dof, 0.0..=1.0).text(gpu("Depth of field", 2)).custom_formatter(|x, _| pct(x)))
            .on_hover_text("Blurs what's nearer or further than what the camera orbits, like a real lens; more blurs more.");
        ui.add_enabled(sim.shadows, egui::Slider::new(&mut sim.contact, 0.0..=1.0).text(gpu("Contact shadows", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0} cm", x * 100.0) }))
            .on_hover_text("Fine sun shadows where things touch, which the shadow map is too coarse for: between the fingers, under the feet. The number is how far they reach.");
        ui.add_enabled(sim.contact > 0.0, egui::Slider::new(&mut sim.contact_k, 0.0..=1.0).text("Contact shadow strength").custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
            .on_hover_text("How dark the contact shadows are.");
        ui.add(egui::Slider::new(&mut sim.vignette, 0.0..=1.0).text(gpu("Vignette", 1)).custom_formatter(|x, _| pct(x)))
            .on_hover_text("Darkens the corners, like a camera lens.");
        ui.add(egui::Slider::new(&mut sim.lens, 0.0..=1.0).text(gpu("Lens distortion", 1)).custom_formatter(|x, _| pct(x)))
            .on_hover_text("Bends straight lines near the edges, like a wide camera lens.");
        ui.add(egui::Slider::new(&mut sim.chroma, 0.0..=1.0).text(gpu("Chromatic aberration", 1)).custom_formatter(|x, _| pct(x)))
            .on_hover_text("Splits the colours apart toward the edges, like a cheap lens.");
        info(ui, "Screen-space reflections need Bevy's deferred renderer, which the room doesn't use; not offered yet.");
    });
    reset_graphics(ui, sim);
}

fn water(ui: &mut egui::Ui, sim: &mut Sim) {
    head(ui, "Water");
    let mut q = sim.water_q as f32;
    if ui.add(egui::Slider::new(&mut q, 0.0..=4.0).step_by(1.0).text(gpu("Water", 1)).custom_formatter(|x, _| ["plain", "ripples", "ripples + refraction", "Extra High", "Max"][x as usize].into()))
        .on_hover_text("Plain: a see-through blue, for a slow PC. Ripples: the breeze's and splashes' ripples catch the light. Ripples + refraction: what's under the water bends and gets bluer the deeper it is. Extra High: the surface itself moves, finer ripples, caustics (sunlight focused by the ripples) on the pool's floor and walls, spray from splashes, a blue haze underwater. Max: finer caustics and three times the spray.").changed() {
        sim.set_water(q as u8);
    }
    fold(ui, "Surface & light", true, |ui| {
        switch(ui, &mut sim.voxel_water, "Voxel water (experimental)", "The water's surface in blocks, splashes and all. Raster only: the ray tracers draw the smooth water.");
        switch(ui, &mut sim.water_ripples, "Ripples", "The pool's own surface: the breeze's and splashes' ripples catch the light, glints on it (off: a flat, smooth surface). Real water's spill and splashes: Fine ripples.");
        switch_if(ui, sim.rt > 0, &mut sim.water_traced, "Ray-traced water", "Ray traced and path traced: the pool's water traced as glass, the light bent through it and its haze from in it the ray tracer's (off: raster's water over the traced room; the slider sets it on from Extra High).",
                  "With ray tracing (Renderer).");
        switch(ui, &mut sim.water_refract, "Refraction", "What's under the water bent and bluer the deeper it is, in raster and the ray tracers.");
        switch(ui, &mut sim.water_depth, "Depth colour", "The colour the water takes on with depth: red soaked up the deeper it is, the blue-green haze seen from in it (off: clear).");
        switch(ui, &mut sim.water_caustics, "Caustics", "The sun's light through the ripples, focused into bright lines on the pool's floor and walls.");
        switch_if(ui, sim.water_caustics && sim.rt > 0, &mut sim.photons, "Photon caustics", "Ray and path traced: the caustics traced by photons on the graphics card (off: worked out on the CPU, as without ray tracing).",
                  "With Caustics and ray tracing.");
        switch_if(ui, sim.water_caustics, &mut sim.water_fine, "Finer caustics", "The caustics on a texture twice as fine (sharper lines, more memory and work), in raster and the CPU's for the ray tracers.", "With Caustics.");
        switch(ui, &mut sim.water_waves, "Moving surface", "The surface itself rises and falls with the ripples (without ray tracing).");
        switch(ui, &mut sim.water_pixels, "Pixel look", "The ripples drawn in blocky pixels, a glint a pixel, and the wet look's patches blocky (off: smooth).");
        switch(ui, &mut sim.water_haze, "Underwater haze", "With the camera under the water: a blue haze the sun shines through in shafts (without ray tracing).");
        switch(ui, &mut sim.sky_shade, "Sky pool shadow", "The sky pool's shadow on the floor tinted by its water, softly edged (without ray tracing: they do it themselves). Path tracing test.");
        switch(ui, &mut sim.water_mirror, "Reflections", "The room mirrored in the water (without ray tracing: the ray tracers reflect it themselves).");
    });
    fold(ui, "Splashes & wet", true, |ui| {
        switch(ui, &mut sim.water_spray, "Spray & foam", "Drops thrown up where things hit the water (without ray tracing); with Real water, its spray, foam and bubbles (ray traced too).");
        switch(ui, &mut sim.water_foam, "Surface foam", "With Real water and Spray & foam: the foam as a lacy sheet on the water, solid where it's thick, a web of bubbles where it thins, carried along and popping (off: foam as flecks).");
        switch(ui, &mut sim.water_mist, "Splash mist", "With Real water and Spray & foam: hard splashes throw off a fine mist that drifts, spreads and fades in a couple of seconds (raster; the ray tracers leave it out).");
        switch(ui, &mut sim.water_wet, "Wet look", "Homer and Hero wet from the water: darker and glossier where it reached them, drying over ~25 s, dripping. Extra High and up: the wet runs down and pools, beads, a water film.");
        switch_if(ui, sim.water_wet, &mut sim.wet_film, "Water film", "Wet look: the wet runs down the skin and pools at the low points, beads, a glossy clear coat and richer colour (off: darkening and shine, drying evenly).", "With Wet look.");
        switch_if(ui, sim.water_wet, &mut sim.wet_drips, "Drips", "Wet look: drops falling off wet bodies (without ray tracing).", "With Wet look.");
    });
    fold(ui, "Real water (experimental)", true, |ui| {
        if switch(ui, &mut sim.real_water, &gpu("Real water (experimental)", 2), "The pool's water simulated for real (particles on a grid, on the graphics card; the sky pool's too in the path tracing test): bodies and things push it about and it pushes back (they float, bob and get carried off), it sloshes and rises. A Flood with it: real water poured in, spilling over.").changed() {
            sim.water_sync();
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(sim.room.pool, egui::Button::selectable(sim.pouring, "Flood"))
                .on_hover_text("Pours water into the pool till it spills over its sides and runs off across the floor (with Real water, real water; without, a cheaper one: a height a square). Again: stops pouring.")
                .on_disabled_hover_text("In the room.").clicked() {
                sim.flood_toggle();
            }
            if ui.add_enabled(sim.real_water || sim.flood.is_some(), egui::Button::new("Drain")).on_hover_text("The pool back as it was, the floor dry.").clicked() {
                sim.drain();
            }
        });
        let real = sim.real_water;
        let why = "With Real water.";
        switch_if(ui, real, &mut sim.real_surface, "Water surface", "Real water: the water in the air (the pour, splashes) a smooth surface round it, in raster, ray and path tracing (off: only the pool's surface and the spill on the floor, a little faster).", why);
        switch_if(ui, real, &mut sim.water_clear, "Clear spill", "Real water: its spill on the floor a clear thin film, as real: the floor darker where it's wet, the sky and sun glinting in it, faint caustics only where it's deep (off: the pool's blue).", why);
        switch_if(ui, real, &mut sim.wet_trail, "Wet trail", "Real water: the floor stays dark where the spill has been after it's gone, drying over half a minute (raster).", why);
        switch_if(ui, real && sim.water_clear, &mut sim.water_impact, "Impact foam", "Real water: white where water lands on its clear spill (under the rim's overflow, splashes coming down; raster).", "With Real water and Clear spill.");
        switch_if(ui, real, &mut sim.water_faucet, "Faucet", "Real water: the pour comes out of a fill pipe over the pool's near end (off: out of the air).", why);
        switch_if(ui, real, &mut sim.real_ripples, "Fine ripples", "Real water: small ripples running over its spill on the floor and its splashes (in Pixel look, blocky), and the sky pool's breeze in the path tracing test (the pool's own surface: Ripples).", why);
        sub(ui, "Water physics");
        switch_if(ui, real, &mut sim.water_relax, "Keep volume", "Real water: the water's thickness (its particles a cell) kept even: crowded spots spread out, sparse ones pulled in, so the water doesn't swell after splashes and pours (the slider sets it on from High).", why);
        switch_if(ui, real, &mut sim.water_vort, "Vorticity", "Real water: swirls and eddies (behind a swimmer, under a pour) kept alive instead of smoothed away (the slider sets it on from Extra High).", why);
        switch_if(ui, real, &mut sim.water_spray_phys, "Spray physics", "Real water: spray thrown where the water's churned and cresting, its drops slowed by the air by their size (fine mist stops short), and more simulation steps in fast splashes (the slider sets it on from Extra High).", why);
        switch_if(ui, real, &mut sim.water_ghost, "Smooth surface pressure", "Real water: the pressure at the water's surface worked out where the surface really is, not a cell out: no steps in the surface, waves that run smooth (the slider sets it on from High).", why);
    });
    reset_graphics(ui, sim);
}

fn display(ui: &mut egui::Ui, sim: &mut Sim) {
    head(ui, "Display");
    sub(ui, "Graphics card");
    let names: Vec<String> = std::iter::once(String::new()).chain(sim.gpus.clone().unwrap_or_default()).collect();
    ui.horizontal_wrapped(|ui| {
        for n in names {
            let label = if n.is_empty() { "Automatic (fastest)".to_string() } else { n.clone() };
            if ui.add(egui::Button::selectable(sim.gpu.eq_ignore_ascii_case(&n), &label).min_size(egui::vec2(0.0, PILL_H))).on_hover_text(format!("Draw with {label} (from the next start)")).clicked() {
                sim.gpu = n;
            }
        }
    });
    let wants = !sim.gpu.is_empty() && !sim.gpu_now.to_lowercase().contains(&sim.gpu.to_lowercase());
    info(ui, &format!("In use: {}.{}", sim.gpu_now, if wants { " Restart the room to switch." } else { "" }));
    if cfg!(windows) {
        let (now, manual) = RENDERER.get().copied().unwrap_or((0, false));
        let next = match std::fs::read_to_string(getup::config_dir().join(FALLBACK)).unwrap_or_default().lines().next().unwrap_or("").trim() { "plain" => 1, "dx12" => 2, _ => 0 };
        sub(ui, "Graphics mode");
        let tips = ["Every feature: DLSS, FSR 3, XeSS, ray tracing.", "No upscalers' hooks: for a driver that crashes starting full.", "For a card whose Vulkan driver crashes either way. No FSR 3, XeSS or DLSS."];
        if let Some(k) = pills(ui, &["Full (Vulkan)", "Plain Vulkan", "DirectX 12"], next, Some(&tips)) && next != k {
            pick_renderer(k);
        }
        if now > 0 && !manual && next == now && ui.button("Try full graphics again").on_hover_text("The room fell back to a safer renderer after the graphics driver crashed starting it. This tries every feature again from the next start (if it crashes again, it falls back by itself).").clicked() {
            pick_renderer(0);
        }
        let why = if now == 0 { String::new() } else if manual { " (your pick)".into() } else { " (a safer one, after the graphics driver crashed starting the room)".into() };
        info(ui, &format!("In use: {}{why}.{}", ["Full (Vulkan)", "Plain Vulkan", "DirectX 12"][now], if next != now { " Restart the room to switch." } else { "" }));
    }
    sub(ui, "Window & frame rate");
    switch(ui, &mut sim.fullscreen, "Fullscreen (F11)", "The whole screen, no window border, on the monitor the window is on. F11 switches it any time.");
    ui.add(egui::Slider::new(&mut sim.fps_limit, FPS_VSYNC..=FPS_FREE).step_by(1.0).text("Frame rate limit")
        .custom_formatter(|x, _| if x <= FPS_VSYNC as f64 { "monitor (V-Sync)".into() } else if x >= FPS_FREE as f64 { "unlimited".into() } else { format!("{x:.0} FPS") }))
        .on_hover_text("The most frames a second it draws. Monitor matches your screen and never tears; a number caps it there (lower keeps the graphics card cooler and quieter); unlimited is as fast as it can go. With the window in the background it never goes over 60.");
    switch(ui, &mut sim.phys_thread, "Physics on its own thread", "Runs the physics at the same time as the rest of the frame's work instead of \
        before it: more frames a second with many Homers, but what you do shows one frame later. The physics itself is the same either way. Off: the least delay.");
    switch(ui, &mut sim.render_thread, "Render on its own thread (restart)", "Draws a frame while the next is worked out: more frames a second, but what you do shows a frame later. Off with \"Physics on its own thread\" on: the least delay at nearly the same frame rate. Takes effect when the room starts again.");
    sub(ui, "Resolution");
    let up = sim.fixed_size();
    ui.add_enabled(!up, egui::Slider::new(&mut sim.render_scale, RENDER_SCALE_MIN..=RENDER_SCALE_MAX).text(gpu("Resolution", 3)).custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
        .on_hover_text("How sharp the 3D view is drawn, as a share of the window's pixels. Lower is blurrier and much faster on a slow graphics card; above 100% it draws more pixels than the window has and shrinks them down, the smoothest edges there are (200% costs about four times 100%).");
    if sim.render_scale < 0.999 && !up {
        let mut u = sim.upscaler as f32;
        if ui.add(egui::Slider::new(&mut u, 0.0..=UP_FSR1 as f32).step_by(1.0).text("Upscaler").custom_formatter(|x, _| UP_NAMES[x as usize].into()))
            .on_hover_text("How the lower Resolution is stretched over the window. Basic just smooths it; FSR 1 (AMD FidelityFX Super Resolution 1, any graphics card) rebuilds sharp edges, and Sharpening sharpens after it.").changed() {
            sim.upscaler = u as u8;
        }
    }
    let v = sim.view_px.as_vec2();
    let r = match sim.aa_now() {
        AA_DLSS => [1.5, 1.72, 2.0, 3.0][sim.dlss_q as usize],
        AA_FSR3 => [1.0, 1.5, 1.7, 2.0, 3.0][sim.fsr_q as usize],
        AA_XESS => [1.0, 1.7, 2.0, 2.3, 3.0][sim.xess_q as usize],
        _ => 1.0,
    };
    let s = if (sim.render_scale - 1.0).abs() > 0.001 && !up { sim.render_scale } else { 1.0 };
    let (drawn, shown) = ((v / r).round(), (v / s).round());
    info(ui, &format!("Drawn at {}×{} pixels, shown at {}×{}.", drawn.x, drawn.y, shown.x, shown.y));
    sub(ui, "Anti-aliasing & upscaling");
    let methods: Vec<u8> = (0..=AA_TAA).chain(sim.fsr_ok.then_some(AA_FSR3)).chain(sim.xess_ok.then_some(AA_XESS)).chain(if sim.dlss_ok { vec![AA_DLAA, AA_DLSS] } else { vec![] }).collect();
    let methods: Vec<u8> = methods.into_iter().filter(|&k| sim.aa_with(k) == k).collect();
    let mut aa = sim.aa_now();
    egui::ComboBox::from_label(gpu("Anti-aliasing", 0)).width(150.0).selected_text(AA_NAMES[aa as usize]).show_ui(ui, |ui| {
        for &k in &methods {
            ui.selectable_value(&mut aa, k, AA_NAMES[k as usize]);
        }
    }).response.on_hover_text("Smooths jagged edges; one method at a time. FXAA: cheapest, a little blurry. SMAA: sharper, still cheap. MSAA: clean edges, costs more on a slow graphics card. TAA: smoothest, can smear when things move fast. FSR 3: AMD's, on any graphics card; draws the room smaller and rebuilds it sharp, much faster. XeSS: Intel's, like FSR 3, best on Intel Arc cards. DLAA: NVIDIA's, the best looking. DLSS: NVIDIA's, like FSR 3, sharper.");
    if aa != sim.aa_now() {
        sim.aa = aa;
    }
    if sim.rt_now() > 0 {
        info(ui, if sim.rt_now() == 2 { "Path tracing picks its own: DLAA (its Ray Reconstruction), else none. Ray tracing off for the rest." }
            else if sim.dlss_ok { "Ray tracing needs DLAA or DLSS (their Ray Reconstruction cleans up its noise). Ray tracing off for the rest." }
            else { "Ray tracing needs TAA or better (it's noisy). Ray tracing off for the rest." });
    }
    match sim.aa {
        AA_SMAA => {
            let mut q = sim.smaa_q as f32;
            if ui.add(egui::Slider::new(&mut q, 0.0..=3.0).step_by(1.0).text(gpu("SMAA quality", 1)).custom_formatter(|x, _| ["low", "medium", "high", "ultra"][x as usize].into()))
                .on_hover_text("How hard SMAA looks for edges. Higher catches more, costs a little more.").changed() {
                sim.smaa_q = q as u8;
            }
        }
        AA_MSAA => {
            let steps: Vec<u8> = [2, 4, 8].into_iter().filter(|&k| k <= sim.msaa_max).collect();
            let mut i = steps.iter().position(|&k| k == sim.msaa).unwrap_or(0) as f32;
            if steps.len() > 1 && ui.add(egui::Slider::new(&mut i, 0.0..=(steps.len() - 1) as f32).step_by(1.0).text(gpu("MSAA samples", 2))
                .custom_formatter(|x, _| format!("{}x", steps[x as usize])))
                .on_hover_text("More samples, smoother edges; each step costs more on a slow graphics card.").changed() {
                sim.msaa = steps[i as usize];
            }
        }
        AA_FSR3 | AA_XESS => {
            let name = AA_NAMES[sim.aa as usize];
            let field = if sim.aa == AA_FSR3 { &mut sim.fsr_q } else { &mut sim.xess_q };
            let mut q = *field as f32;
            if ui.add(egui::Slider::new(&mut q, 0.0..=4.0).step_by(1.0).text(format!("{name} mode"))
                .custom_formatter(|x, _| ["Native AA", "Quality", "Balanced", "Performance", "Ultra Performance"][x as usize].into()))
                .on_hover_text(format!("How small {name} draws the room before rebuilding it. Native AA keeps full size, only smoothing; Quality is sharpest, Ultra Performance fastest.")).changed() {
                *field = q as u8;
            }
        }
        AA_DLSS => {
            let mut q = sim.dlss_q as f32;
            if ui.add(egui::Slider::new(&mut q, 0.0..=3.0).step_by(1.0).text("DLSS mode")
                .custom_formatter(|x, _| ["Quality", "Balanced", "Performance", "Ultra Performance"][x as usize].into()))
                .on_hover_text("How small DLSS draws the room before rebuilding it. Quality is sharpest, Ultra Performance fastest.").changed() {
                sim.dlss_q = q as u8;
            }
        }
        _ => {}
    }
    ui.add(egui::Slider::new(&mut sim.sharpen, 0.0..=1.0).text(gpu("Sharpening", 1)).custom_formatter(|x, _| pct(x)))
        .on_hover_text("Crisps the picture after upscaling or a lower Resolution. Too much looks grainy. Each anti-aliasing method keeps its own.");
    let missing: Vec<&str> = [(!sim.fsr_ok, if cfg!(windows) { "FSR 3 needs amd_fidelityfx_vk.dll beside the room." } else { "FSR 3 is Windows only for now." }),
                              (!sim.xess_ok, if cfg!(windows) { "XeSS needs libxess.dll beside the room." } else { "XeSS is Windows only for now." }),
                              (!sim.dlss_ok, if cfg!(feature = "dlss") { "DLAA and DLSS need an NVIDIA RTX graphics card." } else { "DLAA and DLSS aren't in this build." })]
        .into_iter().filter(|m| m.0).map(|m| m.1).collect();
    if !missing.is_empty() {
        fold(ui, "Why some methods are missing", false, |ui| {
            for m in missing {
                info(ui, m);
            }
        });
    }
    reset_av(ui, sim);
}

fn audio(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Audio");
    *pressed = pressed.take().or(switches(ui, &[("#sound", sim.sound, "Sound".into())]));
    if ui.add_enabled(sim.sound, egui::Button::new("▶ Test sound")).on_hover_text("Play an impact sound through the current Windows output device.").clicked() { sim.sounds.push(sfx::Snd::Hit); }
    info(ui, "8-bit effects for pokes, shots, hits and crashes, and 8-bit background music. If Test sound is silent, check Delirium in the Windows volume mixer.");
    sub(ui, "Volume");
    slide(ui, "Master volume", &mut sim.volume[0], 0.0, 1.0, "").on_hover_text("Everything's loudness.");
    slide(ui, "Sound effects", &mut sim.volume[1], 0.0, 1.0, "").on_hover_text("Shots, hits, punches, explosions, crashes, fire.");
    slide(ui, "Body sounds", &mut sim.volume[4], 0.0, 1.0, "").on_hover_text("Homer's footsteps and the thumps when his body hits the floor, walls or props.");
    slide(ui, "Music", &mut sim.volume[3], 0.0, 1.0, "").on_hover_text("The room's 8-bit background music. 0: off.");
    #[cfg(feature = "portal")]
    slide(ui, "Portal 2 sounds", &mut sim.volume[2], 0.0, 1.0, "").on_hover_text("The portal gun's shots and portals opening (with Portal 2 on).");
    sub(ui, "Music theme");
    let tips = ["Slow drifting chords, a few bells, a little wind.", "A plucked synth arpeggio over soft chords.", "A low, uneasy drone and wind; no tune."];
    if let Some(k) = pills(ui, &sfx::AMB_THEMES, sim.amb_theme, Some(&tips)) {
        sim.amb_theme = k;
    }
    reset_av(ui, sim);
}

fn camera(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Camera & interface");
    sub(ui, "Interface size");
    ui.add(egui::Slider::new(&mut sim.ui_scale, UI_SCALE.0..=UI_SCALE.1).step_by(0.05).text("Interface size").custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
        .on_hover_text("Everything in the panels bigger or smaller: text, buttons, windows (Ctrl + and Ctrl - too).");
    ui.horizontal(|ui| {
        for (name, k) in [("Small", 0.85), ("Normal", 1.0), ("Large", 1.25), ("Huge", 1.5)] {
            if ui.add(egui::Button::selectable((sim.ui_scale - k).abs() < 0.01, name).min_size(egui::vec2(0.0, PILL_H))).on_hover_text(format!("Interface at {:.0}%", k * 100.0)).clicked() {
                sim.ui_scale = k;
            }
        }
    });
    sub(ui, "Sidebar");
    let tips = ["The panel hides at the left edge; the mouse there (or M) brings it out.", "The panel stays open beside the room (the pin does the same)."];
    if let Some(k) = pills(ui, &["Auto-hide", "Always open"], !sim.sidebar_auto as usize, Some(&tips)) {
        sim.sidebar_auto = k == 0;
    }
    sub(ui, "Camera");
    *pressed = pressed.take().or(switches(ui, &[("camera", sim.cam_locked, "Follow camera".into())]));
    slide(ui, "Mouse sensitivity", &mut sim.sens, 0.05, 2.0, "").on_hover_text("How far the camera turns for a move of the mouse.");
    if ui.add(egui::Button::new("📷  Enter photo mode  ·  P").min_size(egui::vec2(ui.available_width(), 44.0))).on_hover_text("Hide the interface and move the camera freely; P returns to the room.").clicked() {
        sim.photo_pending = true;
    }
}
