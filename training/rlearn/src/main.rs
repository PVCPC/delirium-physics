//! PPO learner in Rust (actor + critic MLPs, global clip 0.5, Adam), same maths as the Torch prototype learner.
//! `rlearn in.bin out.bin`: fixed minibatch steps for check.py. `rlearn serve`: stdin/stdout learner for train_motion.py.
use std::io::{Read, Write};
#[cfg(test)] mod coadapt_tests;

const L2PI: f32 = 0.918_938_5;

/// 3 Linear layers, torch layout W[out][in] then b[out]; tanh after layers 1, 2 (and 3 when `tanh_out`).
struct Mlp { s: [usize; 4], tanh_out: bool }

impl Mlp {
    fn len(&self) -> usize { (0..3).map(|l| self.s[l] * self.s[l + 1] + self.s[l + 1]).sum() }
    fn off(&self, l: usize) -> usize { (0..l).map(|k| self.s[k] * self.s[k + 1] + self.s[k + 1]).sum() }

    /// Forward for rows; returns per-layer activations (input first).
    fn forward(&self, p: &[f32], x: &[f32], rows: usize) -> Vec<Vec<f32>> {
        let mut acts = vec![x.to_vec()];
        for l in 0..3 {
            let (n, m, o) = (self.s[l], self.s[l + 1], self.off(l));
            let (w, b) = (&p[o..o + n * m], &p[o + n * m..o + n * m + m]);
            let a = acts.last().unwrap();
            let mut y: Vec<f32> = (0..rows).flat_map(|_| b.iter().copied()).collect();
            gemm(rows, n, m, a, n, 1, w, 1, n, 1.0, &mut y, m, 1);
            if l < 2 || self.tanh_out { for v in &mut y { *v = tanh(*v); } }
            acts.push(y);
        }
        acts
    }

    /// Backward from dL/d(output after its activation); adds into g.
    fn backward(&self, p: &[f32], acts: &[Vec<f32>], mut d: Vec<f32>, rows: usize, g: &mut [f32]) {
        for l in (0..3).rev() {
            let (n, m, o) = (self.s[l], self.s[l + 1], self.off(l));
            if l < 2 || self.tanh_out { for (d, y) in d.iter_mut().zip(&acts[l + 1]) { *d *= 1.0 - y * y; } }
            let a = &acts[l];
            let (gw, gb) = g[o..o + n * m + m].split_at_mut(n * m);
            for r in 0..rows { axpy(gb, 1.0, &d[r * m..(r + 1) * m]); }
            gemm(m, rows, n, &d, 1, m, a, n, 1, 1.0, gw, n, 1);
            if l > 0 {
                let w = &p[o..o + n * m];
                let mut dx = vec![0.0; rows * n];
                gemm(rows, m, n, &d, m, 1, w, n, 1, 0.0, &mut dx, n, 1);
                d = dx;
            }
        }
    }
}

/// C[m x n] = A[m x k] B[k x n] + beta C, with element strides (row, column).
#[allow(clippy::too_many_arguments)]
fn gemm(m: usize, k: usize, n: usize, a: &[f32], rsa: usize, csa: usize, b: &[f32], rsb: usize, csb: usize, beta: f32, c: &mut [f32], rsc: usize, csc: usize) {
    if m == 0 || n == 0 || k == 0 { return; }
    assert!(a.len() > (m - 1) * rsa + (k - 1) * csa && b.len() > (k - 1) * rsb + (n - 1) * csb && c.len() > (m - 1) * rsc + (n - 1) * csc);
    unsafe { matrixmultiply::sgemm(m, k, n, 1.0, a.as_ptr(), rsa as isize, csa as isize, b.as_ptr(), rsb as isize, csb as isize, beta, c.as_mut_ptr(), rsc as isize, csc as isize) }
}

/// Cephes tanhf (branch-free so it vectorizes): odd polynomial below 0.625, else 1 - 2/(e^2x + 1).
fn tanh(x: f32) -> f32 {
    let ax = x.abs().min(9.0); let z = x * x;
    let small = ((((-5.704_988_7e-3 * z + 2.063_908_9e-2) * z - 5.373_971_6e-2) * z + 1.333_144_2e-1) * z - 3.333_328_2e-1) * z * x + x;
    let big = (1.0 - 2.0 / (exp(2.0 * ax) + 1.0)).copysign(x);
    if ax < 0.625 { small } else { big }
}

/// Cephes expf for 0 <= x <= 18.
fn exp(x: f32) -> f32 {
    let fx = (x * std::f32::consts::LOG2_E + 0.5).floor();
    let x = x - fx * 0.693_359_4 + fx * 2.121_944_4e-4;
    let z = x * x;
    let y = (((((1.987_569_1e-4 * x + 1.398_199_9e-3) * x + 8.333_452e-3) * x + 4.166_579_6e-2) * x + 1.666_666_5e-1) * x + 5.000_000_1e-1) * z + x + 1.0;
    y * f32::from_bits(((fx as i32 + 127) << 23) as u32)
}

fn axpy(y: &mut [f32], a: f32, x: &[f32]) { for (y, x) in y.iter_mut().zip(x) { *y = a.mul_add(*x, *y); } }

/// Sum over actions of the Normal log-density, log_std clamped like the trainer.
fn log_prob(mu: &[f32], act: &[f32], ls_raw: &[f32]) -> f32 {
    mu.iter().zip(act).zip(ls_raw).map(|((m, a), l)| { let ls = l.clamp(-4.0, -0.5); let e = a - m; -e * e / (2.0 * (2.0 * ls).exp()) - ls - L2PI }).sum()
}

fn policy_start(actor:&Mlp)->usize {
    // The immutable native gait teacher replaces these28 raw outputs before filtering/physics.
    if actor.s[0]==349&&actor.s[3]==54 {28} else if actor.s[0]==333&&actor.s[3]==50 {28} else {0}
}

fn policy_log_prob(actor:&Mlp,mu:&[f32],act:&[f32],ls:&[f32])->f32 {
    let start=policy_start(actor);log_prob(&mu[start..],&act[start..],&ls[start..])
}

fn row_start(actor:&Mlp,obs:&[f32])->usize {
    if actor.s[0]==349&&actor.s[3]==54&&obs[311]<=0. {50} else {policy_start(actor)}
}
fn staged_motors(actor:&Mlp)->bool {matches!((actor.s[0],actor.s[3]),(350,90)|(351,104))}
fn row_log_prob(actor:&Mlp,mu:&[f32],act:&[f32],ls:&[f32],obs:&[f32])->f32 {
    if staged_motors(actor) {
        return (0..actor.s[3]).filter(|&i|row_active(obs,i)).map(|i|log_prob(&mu[i..i+1],&act[i..i+1],&ls[i..i+1])).sum();
    }
    let start=row_start(actor,obs);log_prob(&mu[start..],&act[start..],&ls[start..])
}
fn row_active(obs:&[f32],action:usize)->bool {
    if action<28 {false} else if action<50 {obs[311]>0.} else if action<54 {obs.len()!=351||obs[311]>0.} else if action<90 {obs[349]>0.} else {obs[350]>0.}
}
fn entropy_share(actor:&Mlp,b:&Batch,action:usize,live:&[usize])->f32 {
    if staged_motors(actor) {
        return live[action] as f32/b.old.len().max(1) as f32;
    }
    if actor.s[0]==349&&actor.s[3]==54 {
        if action<28 {0.} else if action<50 {
            b.x.chunks_exact(actor.s[0]).filter(|x|x[311]>0.).count() as f32/b.old.len().max(1) as f32
        } else {1.}
    } else {1.}
}

/// Minibatch rows (gathered, contiguous).
#[derive(Default)]
struct Batch { x: Vec<f32>, act: Vec<f32>, old: Vec<f32>, adv: Vec<f32>, ret: Vec<f32> }

/// Gradient of rows r0..r1 (loss averaged over all `bf` rows) added into g; returns their summed KL terms.
#[allow(clippy::too_many_arguments)]
fn chunk_grad(actor: &Mlp, critic: &Mlp, p: &[f32], b: &Batch, r0: usize, r1: usize, bf: f32, g: &mut [f32]) -> f32 {
    let (na, a, o, n) = (actor.len(), actor.s[3], actor.s[0], r1 - r0);
    if n == 0 { return 0.0; }
    let (pa, ls_raw, pc) = (&p[..na], &p[na..na + a], &p[na + a..]);
    let mut kl = 0.0;
    let xs = &b.x[r0 * o..r1 * o];
    let acts = actor.forward(pa, xs, n);
    let mu = acts.last().unwrap();
    let mut dmu = vec![0.0; n * a];
    for r in 0..n {
        let observation=&xs[r*o..(r+1)*o];
        let new = row_log_prob(actor,&mu[r * a..(r + 1) * a], &b.act[(r0 + r) * a..(r0 + r + 1) * a], ls_raw,observation);
        let (lo, ad) = (new - b.old[r0 + r], b.adv[r0 + r]);
        let ratio = lo.exp();
        kl += (ratio - 1.0) - lo;
        let clipped = ratio.clamp(0.8, 1.2);
        // d(-min(r*A, clip(r)*A))/d new / B
        let dnew = if ratio * ad <= clipped * ad || (0.8..=1.2).contains(&ratio) { -ad * ratio / bf } else { 0.0 };
        for k in row_start(actor,observation)..a {
            if staged_motors(actor)&&!row_active(observation,k) {continue;}
            let ls = ls_raw[k].clamp(-4.0, -0.5); let var = (2.0 * ls).exp(); let e = b.act[(r0 + r) * a + k] - mu[r * a + k];
            dmu[r * a + k] = dnew * e / var;
            if (-4.0..=-0.5).contains(&ls_raw[k]) { g[na + k] += dnew * (e * e / var - 1.0); }
        }
    }
    actor.backward(pa, &acts, dmu, n, &mut g[..na]);
    let cacts = critic.forward(pc, xs, n);
    let dv: Vec<f32> = (0..n).map(|r| (cacts[3][r] - b.ret[r0 + r]) / bf).collect();
    critic.backward(pc, &cacts, dv, n, &mut g[na + a..]);
    kl
}

/// Shared mutable buffers; each phase between barriers touches disjoint parts (or only reads).
struct Cell<T>(std::cell::UnsafeCell<T>);
unsafe impl<T> Sync for Cell<T> {}
impl<T> Cell<T> {
    fn new(v: T) -> Self { Cell(std::cell::UnsafeCell::new(v)) }
    #[allow(clippy::mut_from_ref)]
    unsafe fn get(&self) -> &mut T { &mut *self.0.get() }
}

/// Parameters (actor ++ log_std ++ critic), Adam state and a persistent pool of `threads` workers (the caller is worker 0).
struct Learner { actor: Mlp, critic: Mlp, threads: usize, learning_rate: f32, p: Cell<Vec<f32>>, m: Cell<Vec<f32>>, v: Cell<Vec<f32>>, t: Cell<i32>,
    /// Staged motors: how many of the minibatch's rows have each action active (counted once per step).
    live: Cell<Vec<usize>>,
    slots: Vec<Cell<(Vec<f32>, f32)>>, norms: Vec<Cell<f64>>, batch: Cell<Batch>, stop: Cell<bool>, barrier: std::sync::Barrier }

impl Learner {
    fn trainable(&self, index:usize)->bool {
        if staged_motors(&self.actor) {
            let head=self.actor.off(2);let bias=head+self.actor.s[3]*self.actor.s[2];let std=self.actor.len();
            if index<head||index>=std+self.actor.s[3] {return true;}
            let action=if index<bias {(index-head)/self.actor.s[2]} else if index<std {index-bias}else{index-std};
            return unsafe {self.live.get()[action]>0};
        }
        // Native immutable gait teacher owns walking/balance/arms; jogging needs adaptable contact features.
        if self.actor.s[0]==349&&self.actor.s[3]==54 {
            let head=self.actor.off(2);let bias=head+54*self.actor.s[2];let std=self.actor.len();
            if index<head||index>=std+54 {return true;}
            let leg=(head+28*self.actor.s[2]..head+50*self.actor.s[2]).contains(&index)
                ||(bias+28..bias+50).contains(&index)||(std+28..std+50).contains(&index);
            if leg {return unsafe {self.batch.get().x.chunks_exact(self.actor.s[0]).any(|x|x[311]>0.)};}
            return (head+50*self.actor.s[2]..bias).contains(&index)||(bias+50..std).contains(&index)||(std+50..std+54).contains(&index);
        }
        if self.actor.s[0]!=333 || self.actor.s[3]!=50 {return true;}
        let head=self.actor.off(2);let bias=head+50*self.actor.s[2];let std=self.actor.len();
        index<head || (head+28*self.actor.s[2]..bias).contains(&index)
            ||(bias+28..std).contains(&index)||(std+28..std+50).contains(&index)||index>=std+50
    }

    fn new(actor: Mlp, critic: Mlp, p: Vec<f32>, threads: usize) -> Self {
        let np = p.len();
        Learner { actor, critic, threads, learning_rate: 1e-4, p: Cell::new(p), m: Cell::new(vec![0.0; np]), v: Cell::new(vec![0.0; np]), t: Cell::new(0), live: Cell::new(Vec::new()),
            slots: (0..threads).map(|_| Cell::new((vec![0.0; np], 0.0))).collect(), norms: (0..threads).map(|_| Cell::new(0.0)).collect(),
            batch: Cell::new(Batch::default()), stop: Cell::new(false), barrier: std::sync::Barrier::new(threads) }
    }

    /// One step's share for worker i: (1) its rows' gradient into slot i; (2) sum its share of the parameters over all
    /// slots and its part of the squared norm; (3) clip and Adam-step its share. Returns false when told to stop.
    fn work(&self, i: usize) -> bool {
        self.barrier.wait();
        unsafe {
            if *self.stop.get() { return false; }
            let (b, np, na, a) = (self.batch.get(), self.p.get().len(), self.actor.len(), self.actor.s[3]);
            let rows = b.old.len(); let chunk = rows.div_ceil(self.threads); let share = np.div_ceil(self.threads);
            let (r0, r1) = ((i * chunk).min(rows), ((i + 1) * chunk).min(rows));
            let slot = self.slots[i].get();
            slot.0.fill(0.0);
            slot.1 = chunk_grad(&self.actor, &self.critic, self.p.get(), b, r0, r1, rows as f32, &mut slot.0);
            self.barrier.wait();
            let (lo, hi) = ((i * share).min(np), ((i + 1) * share).min(np));
            let g = &mut self.slots[i].get().0[lo..hi];
            for (k, other) in self.slots.iter().enumerate() { if k != i { axpy(g, 1.0, &other.get().0[lo..hi]); } }
            let pp = &mut self.p.get()[lo..hi];
            for k in na.max(lo)..(na + a).min(hi) { if (-4.0..=-0.5).contains(&pp[k - lo]) { g[k - lo] -= 0.001*entropy_share(&self.actor,b,k-na,self.live.get()); } }
            for k in lo..hi {if !self.trainable(k) {g[k-lo]=0.;}}
            *self.norms[i].get() = g.iter().map(|v| (v * v) as f64).sum();
            self.barrier.wait();
            let norm = self.norms.iter().map(|n| *n.get()).sum::<f64>().sqrt() as f32;
            let coef = (0.5 / (norm + 1e-6)).min(1.0);
            let t = *self.t.get();
            let (bc1, bc2) = (1.0 - 0.9f32.powi(t), 1.0 - 0.999f32.powi(t));
            let (mm, vv) = (&mut self.m.get()[lo..hi], &mut self.v.get()[lo..hi]);
            for k in 0..hi - lo {
                if !self.trainable(lo+k) {continue;}
                let gk = g[k] * coef;
                mm[k] = 0.9 * mm[k] + 0.1 * gk;
                vv[k] = 0.999 * vv[k] + 0.001 * gk * gk;
                pp[k] -= self.learning_rate / bc1 * mm[k] / (vv[k].sqrt() / bc2.sqrt() + 1e-8);
            }
        }
        self.barrier.wait();
        true
    }

    /// Run on the caller while workers 1.. loop in `work`: one step on the batch set beforehand. Returns the mean KL.
    fn step(&self) -> f32 {
        unsafe {
            *self.t.get() += 1;
            if staged_motors(&self.actor) {
                let (o, a) = (self.actor.s[0], self.actor.s[3]);
                let live = self.live.get(); live.clear(); live.resize(a, 0);
                for x in self.batch.get().x.chunks_exact(o) { for (k, n) in live.iter_mut().enumerate() { *n += row_active(x, k) as usize; } }
            }
        }
        self.work(0);
        let rows = unsafe { self.batch.get().old.len() } as f32;
        self.slots.iter().map(|s| unsafe { s.get().1 }).sum::<f32>() / rows
    }

    fn finish(&self) { unsafe { *self.stop.get() = true; } self.barrier.wait(); }
}

/// Rows of `x` through `net` (params `p`), split over threads; returns the last layer.
fn forward_rows(net: &Mlp, p: &[f32], x: &[f32], threads: usize) -> Vec<f32> {
    let o = net.s[0]; let rows = x.len() / o; let chunk = rows.div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let hs: Vec<_> = x.chunks(chunk * o).map(|xs| s.spawn(move || net.forward(p, xs, xs.len() / o).pop().unwrap())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

fn read_f32(input: &mut impl Read, n: usize) -> Option<Vec<f32>> {
    let mut b = vec![0u8; n * 4]; input.read_exact(&mut b).ok()?;
    Some(b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect())
}

const STATE_MAGIC: &[u8; 4] = b"RLS2";

/// State file (all little-endian): "RLS2", u32 obs, hidden, actions, parameter count n, i32 Adam step t, u64 RNG,
/// u64 completed updates (batches learned), u32 has_snapshot, then f32 params[n], Adam m[n], Adam v[n], snapshot[n] (only when has_snapshot).
/// Order is torch's parameters_to_vector(actor.parameters() + [log_std] + critic.parameters()); m, v = Adam exp_avg, exp_avg_sq.
fn save_state(path: &str, dims: [u32; 3], l: &Learner, seed: u64, updates: u64, snapshot: &Option<Vec<f32>>) -> std::io::Result<()> {
    let (p, m, v, t) = unsafe { (l.p.get(), l.m.get(), l.v.get(), *l.t.get()) };
    let mut b = STATE_MAGIC.to_vec();
    for d in dims.into_iter().chain([p.len() as u32]) { b.extend(d.to_le_bytes()); }
    b.extend(t.to_le_bytes()); b.extend(seed.to_le_bytes()); b.extend(updates.to_le_bytes()); b.extend((snapshot.is_some() as u32).to_le_bytes());
    for x in p.iter().chain(m.iter()).chain(v.iter()).chain(snapshot.iter().flatten()) { b.extend(x.to_le_bytes()); }
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, b)?;
    std::fs::rename(tmp, path)
}

/// Loads a state file into the learner; returns (RNG, completed updates, snapshot).
fn load_state(path: &str, dims: [u32; 3], l: &Learner) -> Result<(u64, u64, Option<Vec<f32>>), String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let np = unsafe { l.p.get().len() };
    let u = |i: usize| b.get(i..i + 4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).unwrap_or(0);
    if b.len() < 44 || &b[..4] != STATE_MAGIC || [u(4), u(8), u(12)] != dims || u(16) as usize != np {
        return Err(format!("{path}: not a learner state for these sizes"));
    }
    let has = u(40) == 1;
    if b.len() != 44 + 4 * np * (3 + has as usize) { return Err(format!("{path}: wrong length")); }
    let f: Vec<f32> = b[44..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    unsafe {
        l.p.get().copy_from_slice(&f[..np]); l.m.get().copy_from_slice(&f[np..2 * np]); l.v.get().copy_from_slice(&f[2 * np..3 * np]);
        *l.t.get() = u(20) as i32;
    }
    Ok((u64::from_le_bytes(b[24..32].try_into().unwrap()), u64::from_le_bytes(b[32..40].try_into().unwrap()), has.then(|| f[3 * np..].to_vec())))
}

/// Learner server. Header: u32 obs, hidden, actions, envs, horizon, threads, then the parameters.
/// Then commands, each a u32 tag:
///   0 batch: obs[T*E*O], actions[T*E*A], rewards[T*E], dones[T*E], last obs[E*O]; reply f32 kl, ms, parameters.
///   1 save / 2 load: u32 byte length + UTF-8 path of a state file; reply u32 0 (ok) or 1 (error, message on stderr),
///     then u64 completed updates (after the load, for load). Load replaces parameters, Adam state, RNG, update count
///     and snapshot; after it the next batch continues exactly as after the save.
fn serve(learning_rate: f32) -> Result<(), String> {
    if !learning_rate.is_finite() || learning_rate <= 0.0 || learning_rate > 1e-4 {
        return Err("learning rate must be finite and within (0, 0.0001]".into());
    }
    let mut input = std::io::stdin().lock(); let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    let mut h = [0u8; 24]; input.read_exact(&mut h).map_err(|e| e.to_string())?;
    let u = |i: usize| u32::from_le_bytes(h[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
    let (o, hid, a, envs, horizon, threads) = (u(0), u(1), u(2), u(3), u(4), u(5));
    if o!=351||a!=104 {return Err("cut-joint learner requires351 observations and104 actions".into());}
    let (actor, critic) = (Mlp { s: [o, hid, hid, a], tanh_out: true }, Mlp { s: [o, hid, hid, 1], tanh_out: false });
    let (na, np) = (actor.len(), actor.len() + a + critic.len());
    let mut learner = Learner::new(actor, critic, read_f32(&mut input, np).ok_or("no parameters")?, threads);
    learner.learning_rate = learning_rate;
    let mut seed = 18u64;
    let random = |seed: &mut u64| { *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17; *seed };
    let n = horizon * envs;
    let mut snapshot: Option<Vec<f32>> = None;
    let mut updates = 0u64;
    std::thread::scope(|s| -> Result<(), String> {
        for i in 1..threads { let l = &learner; s.spawn(move || while l.work(i) {}); }
        let result = (|| -> Result<(), String> {
            loop {
                let mut tag = [0u8; 4];
                if input.read_exact(&mut tag).is_err() { break; }
                let tag = u32::from_le_bytes(tag);
                if tag == 1 || tag == 2 {
                    let mut len = [0u8; 4]; input.read_exact(&mut len).map_err(|e| e.to_string())?;
                    let mut path = vec![0u8; u32::from_le_bytes(len) as usize]; input.read_exact(&mut path).map_err(|e| e.to_string())?;
                    let path = String::from_utf8(path).map_err(|e| e.to_string())?;
                    let dims = [o as u32, hid as u32, a as u32];
                    let done = if tag == 1 { save_state(&path, dims, &learner, seed, updates, &snapshot).map_err(|e| format!("{path}: {e}")) }
                        else { load_state(&path, dims, &learner).map(|(s, u, snap)| { seed = s; updates = u; snapshot = snap; }) };
                    if let Err(e) = &done { eprintln!("rlearn: {e}"); }
                    output.write_all(&(done.is_err() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
                    output.write_all(&updates.to_le_bytes()).map_err(|e| e.to_string())?;
                    output.flush().map_err(|e| e.to_string())?;
                    continue;
                }
                if tag != 0 { return Err(format!("unknown command {tag}")); }
                let obs = read_f32(&mut input, n * o).ok_or("short batch")?;
                let actions = read_f32(&mut input, n * a).ok_or("short batch")?;
                let rewards = read_f32(&mut input, n).ok_or("short batch")?;
                let dones = read_f32(&mut input, n).ok_or("short batch")?;
                let last = read_f32(&mut input, envs * o).ok_or("short batch")?;
                let start = std::time::Instant::now();
                let current = unsafe { learner.p.get().clone() };
                // The batch was collected by the weights this learner had when the previous batch arrived (one-update lag).
                let snap = snapshot.replace(current.clone()).unwrap_or_else(|| current.clone());
                let (actor, critic) = (&learner.actor, &learner.critic);
                let values = forward_rows(critic, &snap[na + a..], &obs, threads);
                let mu = forward_rows(actor, &snap[..na], &obs, threads);
                let old: Vec<f32> = (0..n).map(|r| row_log_prob(actor,&mu[r * a..(r + 1) * a], &actions[r * a..(r + 1) * a], &snap[na..na + a], &obs[r*o..(r+1)*o])).collect();
                let next = forward_rows(critic, &current[na + a..], &last, threads);
                let (gamma, lam) = (0.99f32, 0.95f32);
                let mut adv = vec![0.0f32; n]; let mut carry = vec![0.0f32; envs];
                for t in (0..horizon).rev() {
                    for e in 0..envs {
                        let k = t * envs + e;
                        let following = if t == horizon - 1 { next[e] } else { values[k + envs] };
                        let delta = rewards[k] + gamma * following * (1.0 - dones[k]) - values[k];
                        carry[e] = delta + gamma * lam * (1.0 - dones[k]) * carry[e]; adv[k] = carry[e];
                    }
                }
                let ret: Vec<f32> = adv.iter().zip(&values).map(|(a, v)| a + v).collect();
                let mean = adv.iter().sum::<f32>() / n as f32;
                let std = (adv.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (n - 1) as f32).sqrt();
                for v in &mut adv { *v = (*v - mean) / (std + 1e-8); }
                let mut kl = 0.0;
                'epochs: for _ in 0..4 {
                    let mut perm: Vec<usize> = (0..n).collect();
                    for i in (1..n).rev() { perm.swap(i, (random(&mut seed) % (i as u64 + 1)) as usize); }
                    for idx in perm.chunks(256) {
                        let b = unsafe { learner.batch.get() };
                        b.x.clear(); b.act.clear(); b.old.clear(); b.adv.clear(); b.ret.clear();
                        for &r in idx {
                            b.x.extend_from_slice(&obs[r * o..(r + 1) * o]); b.act.extend_from_slice(&actions[r * a..(r + 1) * a]);
                            b.old.push(old[r]); b.adv.push(adv[r]); b.ret.push(ret[r]);
                        }
                        kl = learner.step();
                        if kl > 0.04 { break 'epochs; }
                    }
                }
                updates += 1;
                output.write_all(&kl.to_le_bytes()).map_err(|e| e.to_string())?;
                output.write_all(&(start.elapsed().as_secs_f32() * 1000.0).to_le_bytes()).map_err(|e| e.to_string())?;
                for x in unsafe { learner.p.get().iter() } { output.write_all(&x.to_le_bytes()).map_err(|e| e.to_string())?; }
                output.flush().map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        learner.finish();
        result
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str)==Some("capabilities") {println!("protected-walk-ankle-anchor-v1");return;}
    if matches!(args.get(1).map(String::as_str), Some("serve" | "serve-lr")) {
        let rate = if args[1] == "serve-lr" { args.get(2).and_then(|v| v.parse().ok()).unwrap_or(f32::NAN) } else { 1e-4 };
        if let Err(e) = serve(rate) { eprintln!("rlearn: {e}"); std::process::exit(1); }
        return;
    }
    let mut b = Vec::new(); std::fs::File::open(&args[1]).unwrap().read_to_end(&mut b).unwrap();
    let u = |i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
    let (rows, o, h, a, steps, threads) = (u(0), u(1), u(2), u(3), u(4), u(5));
    let f: Vec<f32> = b[24..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    let actor = Mlp { s: [o, h, h, a], tanh_out: true }; let critic = Mlp { s: [o, h, h, 1], tanh_out: false };
    let np = actor.len() + a + critic.len();
    let p = f[..np].to_vec(); let mut at = np;
    let mut take = |n: usize| { let v = f[at..at + n].to_vec(); at += n; v };
    let batch = Batch { x: take(rows * o), act: take(rows * a), old: take(rows), adv: take(rows), ret: take(rows) };
    let learner = Learner::new(actor, critic, p, threads);
    unsafe { *learner.batch.get() = batch; }
    let t = std::time::Instant::now(); let mut kl = 0.0;
    std::thread::scope(|s| {
        for i in 1..threads { let l = &learner; s.spawn(move || while l.work(i) {}); }
        for _ in 0..steps { kl = learner.step(); }
        learner.finish();
    });
    eprintln!("{steps} steps, {threads} threads: {:.3} ms/step", t.elapsed().as_secs_f64() * 1000.0 / steps as f64);
    let mut out: Vec<u8> = learner.p.0.into_inner().iter().flat_map(|v| v.to_le_bytes()).collect(); out.extend(kl.to_le_bytes());
    std::fs::write(&args[2], out).unwrap();
}

#[cfg(test)] mod motor_tests {
    use super::*;
    #[test] fn teacher_owned_walking_ankles_keep_density_and_momentum_unchanged(){
        let actor=Mlp{s:[351,4,4,104],tanh_out:true};let critic=Mlp{s:[351,4,4,1],tanh_out:false};
        let na=actor.len();let p=vec![0.02;na+104+critic.len()];let mut x=vec![0.1;351];x[311]=0.;x[349]=0.;x[350]=0.;
        let mu=actor.forward(&p[..na],&x,1).pop().unwrap();let act=vec![0.2;104];let mut changed=act.clone();changed[50..54].fill(5.);
        let old=row_log_prob(&actor,&mu,&act,&p[na..na+104],&x);assert_eq!(old,row_log_prob(&actor,&mu,&changed,&p[na..na+104],&x));
        let learner=Learner::new(actor,critic,p.clone(),1);let head=learner.actor.off(2);
        unsafe {*learner.batch.get()=Batch{x:x.clone(),act:act.clone(),old:vec![old],adv:vec![1.],ret:vec![0.]};learner.m.get().fill(0.1);learner.v.get().fill(0.01);}
        learner.step();
        unsafe {for i in head+50*4..head+54*4 {assert_eq!(learner.p.get()[i],p[i]);assert_eq!(learner.m.get()[i],0.1);assert_eq!(learner.v.get()[i],0.01);}}
        x[311]=1.;assert_ne!(old,row_log_prob(&learner.actor,&mu,&changed,&p[na..na+104],&x));
    }
    #[test] fn cut_motor_gate_controls_density_gradient_and_momentum(){
        let actor=Mlp{s:[351,4,4,104],tanh_out:true};let critic=Mlp{s:[351,4,4,1],tanh_out:false};
        let na=actor.len();let p=vec![0.02;na+104+critic.len()];let mut x=vec![0.1;351];
        x[311]=1.;x[349]=1.;x[350]=0.;let act=vec![0.2;104];
        let mu=actor.forward(&p[..na],&x,1).pop().unwrap();let mut changed=act.clone();changed[90..].fill(5.);
        let old=row_log_prob(&actor,&mu,&act,&p[na..na+104],&x);
        assert_eq!(old,row_log_prob(&actor,&mu,&changed,&p[na..na+104],&x));
        let learner=Learner::new(actor,critic,p.clone(),1);let head=learner.actor.off(2);
        unsafe {*learner.batch.get()=Batch{x:x.clone(),act:act.clone(),old:vec![old],adv:vec![1.],ret:vec![0.]};learner.m.get().fill(0.1);learner.v.get().fill(0.01);}
        learner.step();
        unsafe {for i in head+90*4..head+104*4 {assert_eq!(learner.p.get()[i],p[i]);assert_eq!(learner.m.get()[i],0.1);assert_eq!(learner.v.get()[i],0.01);}}
        x[350]=1.;assert_ne!(old,row_log_prob(&learner.actor,&mu,&changed,&p[na..na+104],&x));
        let b=Batch{x,act,old:vec![old],adv:vec![1.],ret:vec![0.]};let mut g=vec![0.;p.len()];
        chunk_grad(&learner.actor,&learner.critic,&p,&b,0,1,1.,&mut g);
        assert!(g[head+90*4..head+104*4].iter().any(|&x|x!=0.));
        assert!(g[head..head+28*4].iter().all(|&x|x==0.));
    }
    #[test] fn whole_body_likelihood_excludes_inactive_motors_and_credits_actual_control(){
        let actor=Mlp{s:[350,4,4,90],tanh_out:true};let critic=Mlp{s:[350,4,4,1],tanh_out:false};
        let na=actor.len();let p=vec![0.02;na+90+critic.len()];let mut x=vec![0.1;350];
        x[311]=0.;x[349]=0.;
        let mu=actor.forward(&p[..na],&x,1).pop().unwrap();let act=vec![0.2;90];
        let old=row_log_prob(&actor,&mu,&act,&p[na..na+90],&x);
        let mut changed=act.clone();changed[..50].fill(5.);changed[54..].fill(5.);
        assert_eq!(old,row_log_prob(&actor,&mu,&changed,&p[na..na+90],&x));
        x[311]=1.;x[349]=1.;
        assert_ne!(row_log_prob(&actor,&mu,&act,&p[na..na+90],&x),row_log_prob(&actor,&mu,&changed,&p[na..na+90],&x));
        let b=Batch{x:x.clone(),act:act.clone(),old:vec![row_log_prob(&actor,&mu,&act,&p[na..na+90],&x)],adv:vec![1.],ret:vec![0.]};
        let mut g=vec![0.;p.len()];chunk_grad(&actor,&critic,&p,&b,0,1,1.,&mut g);
        let head=actor.off(2);assert!(g[head..head+28*4].iter().all(|&x|x==0.));
        assert!(g[head+54*4..head+90*4].iter().any(|&x|x!=0.));
        let learner=Learner::new(actor,critic,p.clone(),1);
        unsafe {let mut walk=x;walk[311]=0.;walk[349]=0.;*learner.batch.get()=Batch{x:walk,act,old:vec![old],adv:vec![1.],ret:vec![0.]};learner.m.get().fill(0.1);learner.v.get().fill(0.01);}
        learner.step();
        unsafe {for i in head+54*4..head+90*4 {assert_eq!(learner.p.get()[i],p[i]);assert_eq!(learner.m.get()[i],0.1);assert_eq!(learner.v.get()[i],0.01);}}
    }
    #[test] fn teacher_owned_outputs_do_not_change_density_or_backbone_gradient(){
        let actor=Mlp{s:[333,4,4,50],tanh_out:true};let critic=Mlp{s:[333,4,4,1],tanh_out:false};
        let na=actor.len();let p=vec![0.02;na+50+critic.len()];
        let x=vec![0.1;333];let mu=actor.forward(&p[..na],&x,1).pop().unwrap();
        let act=vec![0.2;50];let old=policy_log_prob(&actor,&mu,&act,&p[na..na+50]);
        let mut changed=act.clone();changed[..28].fill(100.);
        assert_eq!(old,policy_log_prob(&actor,&mu,&changed,&p[na..na+50]));
        assert_ne!(log_prob(&mu,&act,&p[na..na+50]),log_prob(&mu,&changed,&p[na..na+50]));
        let b=Batch{x:x.clone(),act,old:vec![old],adv:vec![1.],ret:vec![0.]};
        let c=Batch{x,act:changed,old:vec![old],adv:vec![1.],ret:vec![0.]};
        let mut g=vec![0.;p.len()];let mut h=g.clone();
        assert_eq!(chunk_grad(&actor,&critic,&p,&b,0,1,1.,&mut g),chunk_grad(&actor,&critic,&p,&c,0,1,1.,&mut h));
        assert_eq!(g,h);assert!(g[..333*4].iter().any(|&x|x!=0.));
        let head=actor.off(2);assert!(g[head..head+28*4].iter().all(|&x|x==0.));
        assert!(g[na..na+28].iter().all(|&x|x==0.));
    }
    #[test] fn upper_body_heads_and_moments_stay_exact_while_jog_features_and_legs_learn(){
        let actor=Mlp{s:[333,128,128,50],tanh_out:true};
        let critic=Mlp{s:[333,128,128,1],tanh_out:false};let na=actor.len();
        let learner=Learner::new(actor,critic,vec![0.;na+50+128*333+128+128*128+128+128+1],1);
        let head=learner.actor.off(2);
        assert!((0..head).all(|i|learner.trainable(i)));
        assert!((head..head+28*128).all(|i|!learner.trainable(i)));
        let act=vec![0.1;100];let old=policy_log_prob(&learner.actor,&[0.;50],&act[..50],&[0.;50]);
        unsafe {
            learner.m.get().fill(0.1);learner.v.get().fill(0.01);
            *learner.batch.get()=Batch{x:vec![0.1;666],act,old:vec![old;2],adv:vec![1.;2],ret:vec![2.;2]};
        }
        learner.step();
        unsafe {
            for index in 0..na+50 {
                if !learner.trainable(index) {
                    assert_eq!(learner.p.get()[index],0.);
                    assert_eq!(learner.m.get()[index],0.1);
                    assert_eq!(learner.v.get()[index],0.01);
                }
            }
            assert_ne!(learner.p.get()[na-1],0.);
            assert_ne!(learner.p.get()[332],0.);
            assert_ne!(learner.p.get()[na-50+28],0.);
            assert_ne!(*learner.p.get().last().unwrap(),0.);
        }
    }
    #[test] fn old_architectures_do_not_activate_the_specialized_mask(){
        let actor=Mlp{s:[313,128,128,46],tanh_out:true};let critic=Mlp{s:[313,128,128,1],tanh_out:false};
        let count=actor.len()+46+critic.len();let learner=Learner::new(actor,critic,vec![0.;count],1);
        assert!((0..count).all(|i|learner.trainable(i)));
    }
}
