// Fluid on the GPU (fluid_gpu.rs): fluid.rs's steps, a kernel each. The particles scatter to the faces (fixed point
// atomics: WGSL has no float ones), the cells and faces are typed, the pressure solve is Jacobi-preconditioned CG
// (each dot product summed a workgroup at a time, then by one workgroup), then the faces' speeds go back to the
// particles, which move.

struct Params {
    n: vec3u,
    np: u32,
    lo: vec3f,
    h: f32,
    dt: f32,
    g: f32,
    ncell: u32,
    nface: u32,
    // Where each axis's faces start in the face arrays (x 0).
    off: vec4u,
    // The partial sums' count (the scalars follow them in `red`).
    nred: u32,
    // The columns (n.x n.y: the water's height in each, after the cells' types and counts) and the solids.
    ncol: u32,
    nsolid: u32,
    // A new number each step (whitewater's dice).
    seed: u32,
    // Pouring: where (the spout's middle, its radius), how fast (m/s down), particles a step; whether the grid's outer two
    // columns drain (the floor's spill off its sides).
    spout: vec4f,
    pour_v: f32,
    pour: u32,
    drain: u32,
    // Water physics (fluid.rs `Physics::bits`): 1 volume keeping, 2 ghost-fluid surface, 4 vorticity confinement.
    flags: u32,
    // The room's portals (0 0 0 0: not both open): each one's centre (w 1) and rotation (a quaternion; local +z out of
    // its surface), blue then orange.
    portal: array<vec4f, 4>,
}

// x.w 1: not in the water (poured in later, or drained off).
struct P {
    x: vec4f,
    v: vec4f,
    c0: vec4f,
    c1: vec4f,
    c2: vec4f,
}

@group(0) @binding(0) var<uniform> u: Params;
@group(0) @binding(1) var<storage, read_write> ps: array<P>;
// Momentum then weight, a face each (x FIX).
@group(0) @binding(2) var<storage, read_write> acc: array<atomic<i32>>;
// The faces' speeds and whether known, then the extrapolation's copies.
@group(0) @binding(3) var<storage, read_write> vel: array<f32>;
@group(0) @binding(4) var<storage, read_write> known: array<u32>;
// Particles in each cell, then each cell's type, then each column's highest water (bits of its height over lo).
@group(0) @binding(5) var<storage, read_write> cell: array<atomic<u32>>;
// The solve's x (the pressure), r, p and Ap, a cell each.
@group(0) @binding(6) var<storage, read_write> cg: array<f32>;
// Each workgroup's partial sum, then rz, alpha, beta.
@group(0) @binding(7) var<storage, read_write> red: array<f32>;
// Moving solids (bodies, props) as capsules, 3 a capsule: an end and the radius, the other end, the speed.
@group(0) @binding(8) var<storage, read> solids: array<vec4f>;
// Whitewater (FLIP Fluids' way, cut down): spray, foam and bubbles thrown off where the water's fast at its surface
// (crests, splashes) or churned (its speed's gradient: air dragged in). A ring of them: where each is and how long it
// has left (s, 0 gone), then its speed and kind (0 spray, 1 foam, 2 bubble); then the ring's next.
@group(0) @binding(9) var<storage, read_write> ww: array<vec4f>;
@group(0) @binding(10) var<storage, read_write> wwn: array<atomic<u32>>;
// Each solid's water (the water pushing back, fluid_gpu.rs `push`): its speed round the solid, then its level there.
@group(0) @binding(11) var<storage, read_write> flow: array<vec4f>;
// Each column's ground (m): its cells under it solid (the floor round the pool, what stands on it).
@group(0) @binding(12) var<storage, read> ground: array<f32>;

const AIR: u32 = 0u;
const FLUID: u32 = 1u;
const SOLID: u32 = 2u;
const FIX: f32 = 16384.0;
const WG: u32 = 256u;
// How much of a crowded cell's extra goes a step.
const CROWD: f32 = 0.1;
// Crowded: this many times the 8 (fewer swing in and out as the water moves).
const CROWDED: f32 = 1.5;
// Volume keeping (fluid.rs `drift`): the share of the density's error put out as divergence a step, and the dead band.
const KEEP: f32 = 0.03;
const KEEP_BAND: f32 = 0.1;

var<workgroup> sh: array<f32, 256>;

// Whitewater's ring, how many a particle throws a second at most, and how long they last (s).
const WW: u32 = 32768u;
const WW_RATE: f32 = 120.0;
const WW_LIFE: f32 = 2.5;
// How fast bubbles rise (m/s) and how quickly spray's slowed by the air (/s).
const WW_RISE: f32 = 0.3;
const WW_DRAG: f32 = 0.5;
// Spray physics (flag 8): a drop's drag is quadratic and by its size (a sphere's: 3 rho_air Cd / (8 rho_water r), Cd 0.47;
// its radius 1 to 6 mm), so fine mist stops short and big drops carry. Emission follows Ihmsen et al. 2012: trapped air
// (turbulence: the flow's strain and spin) and wave crest, times kinetic energy.
const DRAG_K: f32 = 0.00021;

// The point on capsule s's spine nearest x.
fn spine(s: u32, x: vec3f) -> vec3f {
    let a = solids[3u * s].xyz;
    let ab = solids[3u * s + 1u].xyz - a;
    let t = clamp(dot(x - a, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
    return a + ab * t;
}

// The capsule x is in (the last), or -1.
fn solid_at(x: vec3f) -> i32 {
    var hit = -1;
    for (var s = 0u; s < u.nsolid; s++) {
        if distance(x, spine(s, x)) < solids[3u * s].w {
            hit = i32(s);
        }
    }
    return hit;
}

fn centre(i: vec3u) -> vec3f {
    return u.lo + (vec3f(i) + vec3f(0.5)) * u.h;
}

fn dims(a: u32) -> vec3u {
    var d = u.n;
    d[a] += 1u;
    return d;
}

fn fidx(a: u32, i: vec3u) -> u32 {
    let d = dims(a);
    return u.off[a] + i.x + d.x * (i.y + d.y * i.z);
}

fn cidx(i: vec3u) -> u32 {
    return i.x + u.n.x * (i.y + u.n.y * i.z);
}

fn cijk(c: u32) -> vec3u {
    return vec3u(c % u.n.x, c / u.n.x % u.n.y, c / (u.n.x * u.n.y));
}

fn ctype(c: u32) -> u32 {
    return atomicLoad(&cell[u.ncell + c]);
}

fn unit(a: u32) -> vec3u {
    var e = vec3u(0u);
    e[a] = 1u;
    return e;
}

fn cvec(p: P, a: u32) -> vec3f {
    if a == 0u { return p.c0.xyz; }
    if a == 1u { return p.c1.xyz; }
    return p.c2.xyz;
}

// Axis a's trilinear stencil at x: its first face and the fraction across.
struct Stencil { base: vec3u, f: vec3f }

fn stencil(a: u32, x: vec3f) -> Stencil {
    let d = dims(a);
    var g = (x - u.lo) / u.h - vec3f(0.5);
    g[a] += 0.5;
    let lo = clamp(floor(g), vec3f(0.0), vec3f(d - vec3u(2u)));
    return Stencil(vec3u(lo), clamp(g - lo, vec3f(0.0), vec3f(1.0)));
}

fn corner(s: Stencil, k: u32) -> vec3u {
    return s.base + vec3u(k & 1u, (k >> 1u) & 1u, k >> 2u);
}

fn weights(s: Stencil, k: u32) -> vec3f {
    let o = vec3f(f32(k & 1u), f32((k >> 1u) & 1u), f32(k >> 2u));
    return mix(vec3f(1.0) - s.f, s.f, o);
}

// The sum of `v` over the workgroup into red[wg] (every thread must call it).
fn reduce(lid: u32, wg: u32, v: f32) {
    sh[lid] = v;
    workgroupBarrier();
    for (var s = WG / 2u; s > 0u; s >>= 1u) {
        if lid < s {
            sh[lid] += sh[lid + s];
        }
        workgroupBarrier();
    }
    if lid == 0u {
        red[wg] = sh[0];
    }
}

// The partial sums summed, by one workgroup.
fn total(lid: u32) -> f32 {
    var s = 0.0;
    for (var i = lid; i < u.nred; i += WG) {
        s += red[i];
    }
    sh[lid] = s;
    workgroupBarrier();
    for (var k = WG / 2u; k > 0u; k >>= 1u) {
        if lid < k {
            sh[lid] += sh[lid + k];
        }
        workgroupBarrier();
    }
    return sh[0];
}

@compute @workgroup_size(256)
fn clear(@builtin(global_invocation_id) id: vec3u) {
    let i = id.x;
    if i < 2u * u.nface {
        atomicStore(&acc[i], 0);
    }
    if i < u.ncell {
        atomicStore(&cell[i], 0u);
    }
    if i < 4u * u.ncol {
        atomicStore(&cell[2u * u.ncell + i], 0u);
    }
    if i == 0u {
        atomicStore(&wwn[1], 0u);
        atomicStore(&wwn[2], 0u);
    }
}

@compute @workgroup_size(256)
fn p2g(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= u.np {
        return;
    }
    let p = ps[id.x];
    if p.x.w > 0.5 {
        return;
    }
    for (var a = 0u; a < 3u; a++) {
        let s = stencil(a, p.x.xyz);
        let c = cvec(p, a);
        for (var k = 0u; k < 8u; k++) {
            let w3 = weights(s, k);
            let w = w3.x * w3.y * w3.z;
            let i = corner(s, k);
            var fp = u.lo + (vec3f(i) + vec3f(0.5)) * u.h;
            fp[a] -= 0.5 * u.h;
            let f = fidx(a, i);
            atomicAdd(&acc[f], i32(round(w * (p.v[a] + dot(c, fp - p.x.xyz)) * FIX)));
            atomicAdd(&acc[u.nface + f], i32(round(w * FIX)));
        }
    }
    let g = clamp(vec3i(floor((p.x.xyz - u.lo) / u.h)), vec3i(0), vec3i(u.n) - vec3i(1));
    atomicAdd(&cell[cidx(vec3u(g))], 1u);
}

@compute @workgroup_size(256)
fn cells(@builtin(global_invocation_id) id: vec3u) {
    let c = id.x;
    if c >= u.ncell {
        return;
    }
    let i = cijk(c);
    var t = AIR;
    if any(i == vec3u(0u)) || any(i == u.n - vec3u(1u)) || centre(i).z < ground[i.x + u.n.x * i.y] || solid_at(centre(i)) >= 0 {
        t = SOLID;
    } else if atomicLoad(&cell[c]) > 0u {
        t = FLUID;
    }
    atomicStore(&cell[u.ncell + c], t);
}

// Face f's axis and its (i, j, k).
struct Face { a: u32, i: vec3u }

fn face(f: u32) -> Face {
    var a = 0u;
    if f >= u.off.z { a = 2u; } else if f >= u.off.y { a = 1u; }
    let d = dims(a);
    let l = f - u.off[a];
    return Face(a, vec3u(l % d.x, l / d.x % d.y, l / (d.x * d.y)));
}

// The cells either side of a face along its axis: solid past the grid.
fn solid_below(fc: Face) -> bool {
    return fc.i[fc.a] == 0u || ctype(cidx(fc.i - unit(fc.a))) == SOLID;
}

fn solid_above(fc: Face) -> bool {
    return fc.i[fc.a] == u.n[fc.a] || ctype(cidx(fc.i)) == SOLID;
}

// The faces' speeds (momentum over weight), gravity, the walls (nothing into one faster than it moves; water moving
// off it leaves). A moving solid's wall moves at its speed.
@compute @workgroup_size(256)
fn faces(@builtin(global_invocation_id) id: vec3u) {
    let f = id.x;
    if f >= u.nface {
        return;
    }
    let fc = face(f);
    let w = atomicLoad(&acc[u.nface + f]);
    var v = 0.0;
    var k = 0u;
    if w > 0 {
        v = f32(atomicLoad(&acc[f])) / f32(w);
        k = 1u;
        if fc.a == 2u {
            v -= u.g * u.dt;
        }
    }
    let lo = solid_below(fc);
    let hi = solid_above(fc);
    if lo || hi {
        var vs = 0.0;
        var s = -1;
        if lo && fc.i[fc.a] > 0u {
            s = solid_at(centre(fc.i - unit(fc.a)));
        }
        if s < 0 && hi && fc.i[fc.a] < u.n[fc.a] {
            s = solid_at(centre(fc.i));
        }
        if s >= 0 {
            vs = solids[3u * u32(s) + 2u][fc.a];
        }
        let into = select((v > vs), (v < vs), lo);
        if k == 0u || (lo && hi) || into {
            v = vs;
        }
        k = 1u;
    }
    vel[f] = v;
    known[f] = k;
}

// Ghost fluid (Gibou et al. 2002): 1 / the fraction of the way from water cell c's middle to an air neighbour's where the
// surface is, from the cell's fullness (its particles over 8): a full cell has it at its face (1/2). 1 with the option off.
const GHOST_MIN: f32 = 0.2;

fn theta_inv(c: u32) -> f32 {
    if (u.flags & 2u) == 0u {
        return 1.0;
    }
    let rho = min(f32(atomicLoad(&cell[c])) / 8.0, 1.0);
    return 1.0 / clamp((rho - 0.5) / max(rho, 1e-3), GHOST_MIN, 1.0);
}

fn diag(i: vec3u) -> f32 {
    var d = 0.0;
    let ti = theta_inv(cidx(i));
    for (var a = 0u; a < 3u; a++) {
        let e = unit(a);
        for (var s = 0u; s < 2u; s++) {
            let t = ctype(cidx(select(i - e, i + e, s == 1u)));
            if t == FLUID { d += 1.0; } else if t == AIR { d += ti; }
        }
    }
    return d;
}

// The preconditioner's 1 / diag: 0 for a water cell walled in by solids on every side (packed bodies): its pressure
// free, out of the solve (1/0 there made the whole pool NaN, all its particles in one cell).
fn inv_diag(i: vec3u) -> f32 {
    let d = diag(i);
    return select(0.0, 1.0 / d, d > 0.0);
}

// A x for x the cg array's slot s, at water cell i.
fn apply_a(i: vec3u, s: u32) -> f32 {
    var o = diag(i) * cg[s * u.ncell + cidx(i)];
    for (var a = 0u; a < 3u; a++) {
        let e = unit(a);
        let m = cidx(i - e);
        let q = cidx(i + e);
        if ctype(m) == FLUID { o -= cg[s * u.ncell + m]; }
        if ctype(q) == FLUID { o -= cg[s * u.ncell + q]; }
    }
    return o;
}

// Volume keeping: water cell i's density error (fluid.rs `drift`).
fn drift(i: vec3u) -> f32 {
    var s = 0.0;
    var m = 0.0;
    var air = false;
    for (var k = 0u; k < 27u; k++) {
        let j = min(i + vec3u(k % 3u, k / 3u % 3u, k / 9u) - vec3u(1u), u.n - vec3u(1u));
        let q = cidx(j);
        let t = ctype(q);
        if t == FLUID {
            s += f32(atomicLoad(&cell[q]));
            m += 1.0;
        } else if t == AIR {
            air = true;
        }
    }
    let e = s / m / 8.0 - 1.0;
    let b = select(sign(e) * max(abs(e) - KEEP_BAND, 0.0), max(e - KEEP_BAND, 0.0), air);
    return clamp(b, -0.5, 1.0);
}

// x = 0, r = -div (the cell's outflow), p = r / diag; rz's partial sums.
@compute @workgroup_size(256)
fn cg_init(@builtin(global_invocation_id) id: vec3u, @builtin(local_invocation_index) lid: u32, @builtin(workgroup_id) wg: vec3u) {
    let c = id.x;
    var rz = 0.0;
    if c < u.ncell {
        var r = 0.0;
        var z = 0.0;
        if ctype(c) == FLUID {
            let i = cijk(c);
            var div = 0.0;
            for (var a = 0u; a < 3u; a++) {
                div += vel[fidx(a, i + unit(a))] - vel[fidx(a, i)];
            }
            // A crowded cell (half as many again as the 8 particles it started with) pushes its extra out over a few steps: else
            // water pushed aside packs in instead of rising (FLIP's lost volume).
            let crowd = max(f32(atomicLoad(&cell[c])) / 8.0 - CROWDED, 0.0);
            var out = CROWD * crowd;
            if (u.flags & 1u) != 0u {
                out = KEEP * drift(i);
            }
            r = out * u.h / u.dt - div;
            z = r * inv_diag(i);
        }
        cg[c] = 0.0;
        cg[u.ncell + c] = r;
        cg[2u * u.ncell + c] = z;
        rz = r * z;
    }
    reduce(lid, wg.x, rz);
}

@compute @workgroup_size(256)
fn fin_rz(@builtin(local_invocation_index) lid: u32) {
    let s = total(lid);
    if lid == 0u {
        red[u.nred] = s;
    }
}

// Ap; p.Ap's partial sums.
@compute @workgroup_size(256)
fn matvec(@builtin(global_invocation_id) id: vec3u, @builtin(local_invocation_index) lid: u32, @builtin(workgroup_id) wg: vec3u) {
    let c = id.x;
    var pap = 0.0;
    if c < u.ncell {
        var ap = 0.0;
        if ctype(c) == FLUID {
            ap = apply_a(cijk(c), 2u);
        }
        cg[3u * u.ncell + c] = ap;
        pap = cg[2u * u.ncell + c] * ap;
    }
    reduce(lid, wg.x, pap);
}

@compute @workgroup_size(256)
fn fin_alpha(@builtin(local_invocation_index) lid: u32) {
    let s = total(lid);
    if lid == 0u {
        red[u.nred + 1u] = select(0.0, red[u.nred] / s, s != 0.0);
    }
}

// x += alpha p, r -= alpha Ap; the new rz's partial sums.
@compute @workgroup_size(256)
fn update(@builtin(global_invocation_id) id: vec3u, @builtin(local_invocation_index) lid: u32, @builtin(workgroup_id) wg: vec3u) {
    let c = id.x;
    var rz = 0.0;
    if c < u.ncell && ctype(c) == FLUID {
        let alpha = red[u.nred + 1u];
        cg[c] += alpha * cg[2u * u.ncell + c];
        let r = cg[u.ncell + c] - alpha * cg[3u * u.ncell + c];
        cg[u.ncell + c] = r;
        rz = r * r * inv_diag(cijk(c));
    }
    reduce(lid, wg.x, rz);
}

@compute @workgroup_size(256)
fn fin_beta(@builtin(local_invocation_index) lid: u32) {
    let s = total(lid);
    if lid == 0u {
        let rz = red[u.nred];
        red[u.nred + 2u] = select(0.0, s / rz, rz != 0.0);
        red[u.nred] = s;
    }
}

@compute @workgroup_size(256)
fn p_update(@builtin(global_invocation_id) id: vec3u) {
    let c = id.x;
    if c < u.ncell && ctype(c) == FLUID {
        cg[2u * u.ncell + c] = cg[u.ncell + c] * inv_diag(cijk(c)) + red[u.nred + 2u] * cg[2u * u.ncell + c];
    }
}

// Cell c's pressure; air beside water cell `o` has the ghost fluid's (which puts the surface at 0).
fn pressure(c: u32, o: u32) -> f32 {
    let t = ctype(c);
    if t == FLUID {
        return cg[c];
    }
    if t == AIR && (u.flags & 2u) != 0u {
        return cg[o] * (1.0 - theta_inv(o));
    }
    return 0.0;
}

// Each face between water (or water and air) loses the pressure's rise across it.
@compute @workgroup_size(256)
fn project(@builtin(global_invocation_id) id: vec3u) {
    let f = id.x;
    if f >= u.nface {
        return;
    }
    let fc = face(f);
    if fc.i[fc.a] == 0u || fc.i[fc.a] == u.n[fc.a] {
        return;
    }
    let lo = cidx(fc.i - unit(fc.a));
    let hi = cidx(fc.i);
    let tl = ctype(lo);
    let th = ctype(hi);
    if tl == SOLID || th == SOLID || (tl != FLUID && th != FLUID) {
        return;
    }
    vel[f] -= pressure(hi, lo) - pressure(lo, hi);
    known[f] = 1u;
}

// Vorticity confinement (Fedkiw, Stam & Jensen 2001, "Visual simulation of smoke"; the particle form is SPlisHSPlasH's
// VorticityConfinement, MIT, Jan Bender; fluid.rs `vorticity`): the grid's smoothing and the pressure solve wear eddies
// down, so each water cell with water all round gets a force VORT x h x (N x w) on the faces, w its spin (the speed's
// curl) and N the direction up the spin's size's slope. `curl` puts each cell's w and |w| in the solve's arrays (free
// till cg_init), `vort` pushes the faces.
const VORT: f32 = 0.5;

fn inner(i: vec3u) -> bool {
    if ctype(cidx(i)) != FLUID {
        return false;
    }
    for (var a = 0u; a < 3u; a++) {
        if ctype(cidx(i - unit(a))) != FLUID || ctype(cidx(i + unit(a))) != FLUID {
            return false;
        }
    }
    return true;
}

fn centre_vel(i: vec3u) -> vec3f {
    var v = vec3f(0.0);
    for (var a = 0u; a < 3u; a++) {
        v[a] = 0.5 * (vel[fidx(a, i)] + vel[fidx(a, i + unit(a))]);
    }
    return v;
}

@compute @workgroup_size(256)
fn curl(@builtin(global_invocation_id) id: vec3u) {
    let c = id.x;
    if c >= u.ncell || (u.flags & 4u) == 0u {
        return;
    }
    let i = cijk(c);
    var w = vec3f(0.0);
    if inner(i) {
        let dx = centre_vel(i + unit(0u)) - centre_vel(i - unit(0u));
        let dy = centre_vel(i + unit(1u)) - centre_vel(i - unit(1u));
        let dz = centre_vel(i + unit(2u)) - centre_vel(i - unit(2u));
        w = vec3f(dy.z - dz.y, dz.x - dx.z, dx.y - dy.x) / (2.0 * u.h);
    }
    cg[c] = w.x;
    cg[u.ncell + c] = w.y;
    cg[2u * u.ncell + c] = w.z;
    cg[3u * u.ncell + c] = length(w);
}

fn confine(c: u32) -> vec3f {
    let i = cijk(c);
    if !inner(i) {
        return vec3f(0.0);
    }
    var g = vec3f(0.0);
    for (var a = 0u; a < 3u; a++) {
        g[a] = (cg[3u * u.ncell + cidx(i + unit(a))] - cg[3u * u.ncell + cidx(i - unit(a))]) / (2.0 * u.h);
    }
    let w = vec3f(cg[c], cg[u.ncell + c], cg[2u * u.ncell + c]);
    return VORT * u.h * cross(g / (length(g) + 1e-3), w);
}

@compute @workgroup_size(256)
fn vort(@builtin(global_invocation_id) id: vec3u) {
    let f = id.x;
    if f >= u.nface || (u.flags & 4u) == 0u {
        return;
    }
    let fc = face(f);
    if fc.i[fc.a] == 0u || fc.i[fc.a] == u.n[fc.a] || known[f] != 1u {
        return;
    }
    let lo = cidx(fc.i - unit(fc.a));
    let hi = cidx(fc.i);
    if ctype(lo) != FLUID || ctype(hi) != FLUID {
        return;
    }
    vel[f] += u.dt * 0.5 * (confine(lo)[fc.a] + confine(hi)[fc.a]);
}

// Faces with no speed take their known neighbours' mean (into the copies).
@compute @workgroup_size(256)
fn extrapolate(@builtin(global_invocation_id) id: vec3u) {
    let f = id.x;
    if f >= u.nface {
        return;
    }
    var v = vel[f];
    var k = known[f];
    if k == 0u {
        let fc = face(f);
        let d = dims(fc.a);
        var s = 0.0;
        var m = 0u;
        for (var b = 0u; b < 3u; b++) {
            let e = unit(b);
            if fc.i[b] > 0u {
                let g = fidx(fc.a, fc.i - e);
                if known[g] == 1u { s += vel[g]; m += 1u; }
            }
            if fc.i[b] + 1u < d[b] {
                let g = fidx(fc.a, fc.i + e);
                if known[g] == 1u { s += vel[g]; m += 1u; }
            }
        }
        if m > 0u {
            v = s / f32(m);
            k = 1u;
        }
    }
    vel[u.nface + f] = v;
    known[u.nface + f] = k;
}

@compute @workgroup_size(256)
fn extrapolate_back(@builtin(global_invocation_id) id: vec3u) {
    let f = id.x;
    if f < u.nface {
        vel[f] = vel[u.nface + f];
        known[f] = known[u.nface + f];
    }
}

// The portals' size (half; portal.rs HALF), and the fastest water comes out of one (m/s: falling through a pair in a
// floor and over it for ever, Portal's own top speed; the grid's cells a substep).
const PORTAL_HALF: vec2f = vec2f(0.8128, 1.4224);
const PORTAL_FASTEST: f32 = 10.0;

fn qrot(q: vec4f, v: vec3f) -> vec3f {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// The faces' speeds back to the particles (APIC: the speed and its gradient), then they move, a quarter cell off
// the walls.
@compute @workgroup_size(256)
fn g2p(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= u.np {
        return;
    }
    var p = ps[id.x];
    if p.x.w > 0.5 {
        return;
    }
    var c: array<vec3f, 3>;
    for (var a = 0u; a < 3u; a++) {
        let s = stencil(a, p.x.xyz);
        var v = 0.0;
        var grad = vec3f(0.0);
        for (var k = 0u; k < 8u; k++) {
            let w = weights(s, k);
            let o = vec3f(f32(k & 1u), f32((k >> 1u) & 1u), f32(k >> 2u));
            let sg = (o * 2.0 - 1.0) / u.h;
            let q = vel[fidx(a, corner(s, k))];
            v += w.x * w.y * w.z * q;
            grad += vec3f(sg.x * w.y * w.z, w.x * sg.y * w.z, w.x * w.y * sg.z) * q;
        }
        p.v[a] = v;
        c[a] = grad;
    }
    var x = p.x.xyz + p.v.xyz * u.dt;
    let lo = u.lo + vec3f(1.25 * u.h);
    let hi = u.lo + (vec3f(u.n) - vec3f(1.25)) * u.h;
    for (var a = 0u; a < 3u; a++) {
        if x[a] < lo[a] {
            x[a] = lo[a];
            p.v[a] = max(p.v[a], 0.0);
        } else if x[a] > hi[a] {
            x[a] = hi[a];
            p.v[a] = min(p.v[a], 0.0);
        }
    }
    // Out of a moving solid, to its skin, not moving into it.
    let s = solid_at(x);
    if s >= 0 {
        let q = spine(u32(s), x);
        let d = x - q;
        let n = select(vec3f(0.0, 0.0, 1.0), d / length(d), length(d) > 1e-6);
        x = q + n * solids[3u * u32(s)].w;
        let vn = dot(p.v.xyz - solids[3u * u32(s) + 2u].xyz, n);
        if vn < 0.0 {
            p.v = vec4f(p.v.xyz - vn * n, 0.0);
        }
    }
    // Into a portal (within a cell of it, inside its opening, not moving out), out of the other: as far in front of it,
    // its speed turned with it (as portal.rs' objects; the APIC gradient dropped). Not where it'd come out off the grid.
    if u.portal[0].w > 0.0 {
        for (var i = 0u; i < 2u; i++) {
            let qa = u.portal[2u * i + 1u];
            let ia = vec4f(-qa.xyz, qa.w);
            let l = qrot(ia, x - u.portal[2u * i].xyz);
            let lv = qrot(ia, p.v.xyz);
            let e = l.xy / PORTAL_HALF;
            if l.z < u.h && l.z > -2.0 * u.h && dot(e, e) < 1.0 && lv.z <= 0.0 {
                // (Glued back to back: 180 degrees about local y.)
                let qb = u.portal[3u - 2u * i];
                let out = u.portal[2u - 2u * i].xyz + qrot(qb, vec3f(-l.x, l.y, max(l.z, u.h)));
                if all(out > lo) && all(out < hi) {
                    x = out;
                    let v = qrot(qb, vec3f(-lv.x, lv.y, -lv.z));
                    p.v = vec4f(v * min(1.0, PORTAL_FASTEST / max(length(v), 1e-6)), 0.0);
                    c[0] = vec3f(0.0);
                    c[1] = vec3f(0.0);
                    c[2] = vec3f(0.0);
                    break;
                }
            }
        }
    }
    // Run off the grid's side (the floor's spill): gone, to be poured again.
    let gx = (x.xy - u.lo.xy) / u.h;
    let dead = u.drain != 0u && (any(gx < vec2f(2.0)) || any(gx >= vec2f(u.n.xy) - vec2f(2.0)));
    p.x = vec4f(x, select(0.0, 1.0, dead));
    p.c0 = vec4f(c[0], 0.0);
    p.c1 = vec4f(c[1], 0.0);
    p.c2 = vec4f(c[2], 0.0);
    ps[id.x] = p;
    // The fastest particle's speed (positive floats order as their bits): the CFL substeps' measure.
    atomicMax(&wwn[2], bitcast<u32>(length(p.v.xyz)));
}

// Each column's highest particle in a cell with a few in (spray's lone drops left out). And the water on the floor's speed: each column's sheet's (within
// SHEET_UP of its ground) particles' and their sum, fixed point (TOPS_V; i32 as u32, wrapping alike).
const SHEET_UP = 0.3;
const TOPS_V = 1024.0;
@compute @workgroup_size(256)
fn tops(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= u.np || ps[id.x].x.w >= 0.5 {
        return;
    }
    let x = ps[id.x].x.xyz;
    let v = ps[id.x].v.xyz;
    let g = clamp(vec3i(floor((x - u.lo) / u.h)), vec3i(0), vec3i(u.n) - vec3i(1));
    let col = u32(g.x) + u.n.x * u32(g.y);
    if atomicLoad(&cell[cidx(vec3u(g))]) >= 3u {
        atomicMax(&cell[2u * u.ncell + col], bitcast<u32>(max(x.z - u.lo.z, 0.0)));
    }
    if x.z < ground[col] + SHEET_UP {
        let k = 2u * u.ncell + u.ncol + 3u * col;
        atomicAdd(&cell[k], bitcast<u32>(i32(clamp(v.x, -20.0, 20.0) * TOPS_V)));
        atomicAdd(&cell[k + 1u], bitcast<u32>(i32(clamp(v.y, -20.0, 20.0) * TOPS_V)));
        atomicAdd(&cell[k + 2u], 1u);
    }
}

fn whash(x: u32) -> f32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return f32((h >> 22u) ^ h) / 4294967296.0;
}

// The cell x is in (outside the grid: solid).
fn type_at(g: vec3i) -> u32 {
    if any(g < vec3i(0)) || any(g >= vec3i(u.n)) {
        return SOLID;
    }
    return ctype(cidx(vec3u(g)));
}

fn grid_cell(x: vec3f) -> vec3i {
    return vec3i(floor((x - u.lo) / u.h));
}

// The faces' speed at x.
fn grid_vel(x: vec3f) -> vec3f {
    var v = vec3f(0.0);
    for (var a = 0u; a < 3u; a++) {
        let s = stencil(a, x);
        for (var k = 0u; k < 8u; k++) {
            let w = weights(s, k);
            v[a] += w.x * w.y * w.z * vel[fidx(a, corner(s, k))];
        }
    }
    return v;
}

// A particle's whitewater: from its speed (none slower than 1 m/s, the most from 3), more at a crest (at the surface,
// going up) or churned (its speed's gradient).
@compute @workgroup_size(256)
fn ww_emit(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= u.np {
        return;
    }
    let p = ps[id.x];
    let x = p.x.xyz;
    let v = p.v.xyz;
    let g = grid_cell(x);
    if p.x.w > 0.5 || type_at(g) != FLUID {
        return;
    }
    let e = clamp((0.5 * dot(v, v) - 0.5) / 4.0, 0.0, 1.0);
    if e <= 0.0 {
        return;
    }
    let crest = select(0.0, clamp(v.z / 1.5, 0.0, 1.0), type_at(g + vec3i(0, 0, 1)) == AIR);
    let churn = clamp((length(p.c0.xyz) + length(p.c1.xyz) + length(p.c2.xyz) - 12.0) / 36.0, 0.0, 1.0);
    var mixed = min(crest + churn, 1.0);
    let spray = (u.flags & 8u) != 0u;
    if spray {
        // Trapped air: the strain (the gradient's symmetric part) and half the spin, over a floor.
        let g0 = p.c0.xyz;
        let g1 = p.c1.xyz;
        let g2 = p.c2.xyz;
        let s = vec3f(g0.x, g1.y, g2.z);
        let o = 0.5 * vec3f(g1.z + g2.y, g0.z + g2.x, g0.y + g1.x);
        let strain = sqrt(dot(s, s) + 2.0 * dot(o, o));
        let spin = length(vec3f(g2.y - g1.z, g0.z - g2.x, g1.x - g0.y));
        let trapped = clamp((strain + 0.5 * spin - 8.0) / 32.0, 0.0, 1.0);
        mixed = min(0.7 * trapped + 0.6 * crest, 1.0);
    }
    let n = mixed * e * WW_RATE * u.dt;
    let seed = id.x * 9781u + u.seed * 6271u;
    let k = min(u32(floor(n)) + select(0u, 1u, whash(seed) < fract(n)), 4u);
    for (var i = 0u; i < k; i++) {
        let j = atomicAdd(&wwn[0], 1u) % WW;
        let r = vec3f(whash(seed + 1u + i * 4u), whash(seed + 2u + i * 4u), whash(seed + 3u + i * 4u)) - 0.5;
        ww[j] = vec4f(x + r * u.h, WW_LIFE * (0.5 + whash(seed + 4u + i * 4u)));
        // (The drop's size rides in the kind's fraction: 0 to 1, over 1 to 6 mm.)
        let size = select(0.0, 0.05 + 0.9 * whash(seed * 3u + 17u + i), spray);
        ww[WW + j] = vec4f(v + r * 0.3 * length(v), size);
    }
}

// Whitewater's step: in the air it's spray (falls); in the water's top cell foam (rides it); deeper a bubble (rises).
@compute @workgroup_size(256)
fn ww_step(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= WW {
        return;
    }
    let a = ww[id.x];
    var life = a.w;
    if life <= 0.0 {
        return;
    }
    var x = a.xyz;
    var v = ww[WW + id.x].xyz;
    let size = fract(ww[WW + id.x].w);
    let g = grid_cell(x);
    let t = type_at(g);
    var kind = 0.0;
    if t == SOLID {
        life = 0.0;
    } else if t == FLUID {
        let w = grid_vel(x);
        if type_at(g + vec3i(0, 0, 1)) == FLUID {
            kind = 2.0;
            v = w + vec3f(0.0, 0.0, WW_RISE);
            life -= u.dt * 1.5;
        } else {
            kind = 1.0;
            v = w;
            // (Foam pops quickly: a second or two.)
            life -= u.dt * 2.0;
        }
    } else {
        var drag = WW_DRAG * u.dt;
        if size > 0.0 && (u.flags & 8u) != 0u {
            drag = min(DRAG_K * length(v) / (0.001 + 0.005 * size) * u.dt, 0.9);
        }
        v = (v + vec3f(0.0, 0.0, -u.g) * u.dt) * (1.0 - drag);
        life -= u.dt * 0.25;
    }
    x += v * u.dt;
    ww[id.x] = vec4f(x, life);
    ww[WW + id.x] = vec4f(v, kind + size);
}

// Each solid's water: the highest water in the columns just past its skin (round it, and its own under it: a body
// sitting in the water leaves its own column's top at its underside) and the water's mean speed there (the solid's
// own where there's none: no drag).
@compute @workgroup_size(256)
fn forces(@builtin(global_invocation_id) id: vec3u) {
    let s = id.x;
    if s == 0u {
        // (The last slot: the fastest particle's speed.)
        flow[1023] = vec4f(bitcast<f32>(atomicLoad(&wwn[2])), 0.0, 0.0, 0.0);
    }
    if s >= u.nsolid {
        return;
    }
    let a = solids[3u * s].xyz;
    let b = solids[3u * s + 1u].xyz;
    let r = solids[3u * s].w + u.h;
    var top = -1e9;
    var v = vec3f(0.0);
    var m = 0.0;
    for (var t = 0u; t < 3u; t++) {
        let q = mix(a, b, f32(t) * 0.5);
        for (var k = 0u; k < 9u; k++) {
            var x = q - vec3f(0.0, 0.0, r);
            if k < 8u {
                let d = f32(k) * 0.7853982;
                x = q + vec3f(cos(d), sin(d), 0.0) * r;
            }
            let g = grid_cell(x);
            if any(g < vec3i(0)) || any(g >= vec3i(u.n)) {
                continue;
            }
            let col = atomicLoad(&cell[2u * u.ncell + u32(g.x) + u.n.x * u32(g.y)]);
            if col != 0u {
                top = max(top, u.lo.z + bitcast<f32>(col));
            }
            if ctype(cidx(vec3u(g))) == FLUID {
                v += grid_vel(x);
                m += 1.0;
            }
        }
    }
    flow[s] = vec4f(select(solids[3u * s + 2u].xyz, v / max(m, 1.0), m > 0.0), top);
}

// Pouring: particles not in the water, as many as a step pours, start at the spout (a disc, a step's fall deep)
// falling at its speed.
@compute @workgroup_size(256)
fn pour(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= u.np || u.pour == 0u || ps[id.x].x.w < 0.5 {
        return;
    }
    let k = atomicAdd(&wwn[1], 1u);
    if k >= u.pour {
        return;
    }
    let seed = id.x * 7919u + u.seed * 104729u;
    let a = whash(seed) * 6.2831853;
    let r = sqrt(whash(seed + 1u)) * u.spout.w;
    let x = u.spout.xyz + vec3f(cos(a) * r, sin(a) * r, -whash(seed + 2u) * u.pour_v * u.dt);
    ps[id.x] = P(vec4f(x, 0.0), vec4f(0.0, 0.0, -u.pour_v, 0.0), vec4f(0.0), vec4f(0.0), vec4f(0.0));
}
