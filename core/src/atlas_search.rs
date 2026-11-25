//! Finds a good tree for a plan (`atlas_model::Problem`): take the path worth most per point until the points run out, then
//! cut branches and grow them again, keeping what's better. Same ask, same tree (fixed seed).

use crate::atlas_model::Problem;

/// A tree being built: which nodes (by index), which option each has, and the summed channels.
#[derive(Clone)]
pub struct Tree {
    pub has: Vec<bool>,
    pub opt: Vec<usize>,
    pub x: Vec<f64>,
    pub n: usize,
    pub u: f64,
}

pub struct Search<'a> {
    pub p: &'a Problem,
    adj: Vec<Vec<usize>>,
    start: usize,
    /// Never taken, not even to pass through.
    ban: Vec<bool>,
    /// Always taken, with a fixed option (index into the node's alts).
    locked: Vec<Option<Option<usize>>>,
    /// Which channels any node touches, and the smallest step each comes in.
    used: Vec<usize>,
    step: Vec<f64>,
    default: Vec<usize>,
    seed: u64,
    // scratch
    dist: Vec<i32>,
    prev: Vec<usize>,
    sum: Vec<f64>,
    queue: Vec<usize>,
}

impl<'a> Search<'a> {
    pub fn new(p: &'a Problem) -> Search<'a> {
        let g = crate::atlas::graph();
        let n = p.ids.len();
        let ix: std::collections::HashMap<u32, usize> = p.ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut adj: Vec<Vec<usize>> = p.ids.iter().map(|id| g.neighbours(*id).iter().filter_map(|m| ix.get(m).copied()).collect()).collect();
        adj.iter_mut().for_each(|a: &mut Vec<usize>| a.sort());
        let m = crate::atlas_model::model();
        let mut locked: Vec<Option<Option<usize>>> = vec![None; n];
        for (id, o) in &p.setup.lock {
            if let Some(&i) = ix.get(id) { locked[i] = Some(o.and_then(|o| p.alts[i].iter().position(|a| a.id == o))); }
        }
        let mut ban = vec![false; n];
        for (i, id) in p.ids.iter().enumerate() {
            ban[i] = locked[i].is_none() && (p.setup.block.contains(id) || m.never.contains_key(id));
        }
        for (a, b, _) in &m.clash {
            // Both locked: his choice. One locked: the other is out.
            if let (Some(&a), Some(&b)) = (ix.get(a), ix.get(b)) {
                if locked[a].is_some() && locked[b].is_none() { ban[b] = true; }
                if locked[b].is_some() && locked[a].is_none() { ban[a] = true; }
            }
        }
        let mut step = vec![0.0f64; p.k];
        for a in p.alts.iter().flatten() { for (k, v) in &a.d { let v = v.abs(); if v > 0.0 { step[*k] = if step[*k] == 0.0 { v } else { step[*k].min(v) }; } } }
        let used = (0..p.k).filter(|k| step[*k] > 0.0).collect();
        let default = p.ids.iter().enumerate().map(|(i, id)| m.defaults.get(id).and_then(|o| p.alts[i].iter().position(|a| a.id == *o)).unwrap_or(0)).collect();
        Search { p, adj, start: ix[&g.start], ban, locked, used, step, default, seed: 0x9E37_79B9_7F4A_7C15,
            dist: vec![-1; n], prev: vec![0; n], sum: vec![0.0; n], queue: Vec::with_capacity(n) }
    }

    fn rnd(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn empty(&self) -> Tree {
        let n = self.p.ids.len();
        let mut t = Tree { has: vec![false; n], opt: vec![0; n], x: vec![0.0; self.p.k], n: 0, u: 0.0 };
        self.add(&mut t, self.start);
        t.u = self.p.eval(&t.x);
        t
    }

    fn apply(&self, x: &mut [f64], i: usize, o: usize, sign: f64) { for (k, v) in &self.p.alts[i][o].d { x[*k] += sign * v; } }

    fn best_option(&self, t: &mut Tree, i: usize) -> usize {
        if let Some(Some(o)) = self.locked[i] { return o; }
        let n = self.p.alts[i].len();
        if n == 1 { return 0; }
        let mut best = self.default[i];
        self.apply(&mut t.x, i, best, 1.0);
        let mut u = self.p.eval(&t.x);
        self.apply(&mut t.x, i, best, -1.0);
        for o in 0..n {
            if o == self.default[i] { continue; }
            self.apply(&mut t.x, i, o, 1.0);
            let w = self.p.eval(&t.x);
            self.apply(&mut t.x, i, o, -1.0);
            if w > u + 0.003 || (best != self.default[i] && w > u + 1e-5) { u = w; best = o; }
        }
        best
    }

    pub fn add(&self, t: &mut Tree, i: usize) {
        if t.has[i] { return; }
        let o = self.best_option(t, i);
        t.has[i] = true;
        t.opt[i] = o;
        self.apply(&mut t.x, i, o, 1.0);
        t.n += 1;
    }

    pub fn remove(&self, t: &mut Tree, i: usize) {
        if !t.has[i] { return; }
        t.has[i] = false;
        let o = t.opt[i];
        self.apply(&mut t.x, i, o, -1.0);
        t.n -= 1;
    }

    /// What each node is worth to this tree (log scale): what it adds when it's out, what would be lost when it's in.
    pub fn worth(&self, t: &Tree) -> Vec<f64> {
        let mut x = t.x.clone();
        let u0 = self.p.eval(&x);
        let mut grad = vec![0.0; self.p.k];
        for &k in &self.used {
            let h = self.step[k];
            x[k] += h;
            grad[k] = (self.p.eval(&x) - u0) / h;
            x[k] -= h;
        }
        let mut w = vec![0.0; self.p.ids.len()];
        for i in 0..w.len() {
            if self.ban[i] { w[i] = -1.0; continue; }
            let alts = &self.p.alts[i];
            let mut best = f64::MIN;
            for (o, a) in alts.iter().enumerate() {
                if let Some(Some(l)) = self.locked[i] { if o != l { continue; } }
                let mut v: f64 = a.d.iter().map(|(k, d)| grad[*k] * d).sum();
                // Big or yes/no effects and choices are measured for real.
                let big = alts.len() > 1 || a.d.iter().any(|(k, d)| d.abs() > 3.0 * self.step[*k]);
                if big {
                    let sign = if t.has[i] { if o == t.opt[i] { -1.0 } else { 0.0 } } else { 1.0 };
                    if sign != 0.0 {
                        self.apply(&mut x, i, o, sign);
                        v = sign * (self.p.eval(&x) - u0);
                        self.apply(&mut x, i, o, -sign);
                    } else { continue; }
                }
                best = best.max(v);
            }
            w[i] = if best == f64::MIN { 0.0 } else { best };
        }
        w
    }

    /// Shortest ways from the tree to everything within `limit`; among equally short ones the best-valued.
    fn reach(&mut self, t: &Tree, w: &[f64], limit: i32) {
        self.dist.iter_mut().for_each(|d| *d = -1);
        self.queue.clear();
        for i in 0..t.has.len() { if t.has[i] { self.dist[i] = 0; self.sum[i] = 0.0; self.queue.push(i); } }
        let mut h = 0;
        while h < self.queue.len() {
            let a = self.queue[h];
            h += 1;
            if self.dist[a] >= limit { continue; }
            for j in 0..self.adj[a].len() {
                let b = self.adj[a][j];
                if self.ban[b] { continue; }
                let v = self.sum[a] + w[b];
                if self.dist[b] < 0 { self.dist[b] = self.dist[a] + 1; self.prev[b] = a; self.sum[b] = v; self.queue.push(b); }
                else if self.dist[b] == self.dist[a] + 1 && v > self.sum[b] + 1e-12 { self.sum[b] = v; self.prev[b] = a; }
            }
        }
    }

    fn take_path(&self, t: &mut Tree, to: usize) -> Vec<usize> {
        let mut path = vec![];
        let mut c = to;
        while !t.has[c] { path.push(c); c = self.prev[c]; }
        path.reverse();
        for i in &path { self.add(t, *i); }
        path
    }

    fn join_locked(&mut self, t: &mut Tree) {
        let flat = vec![0.0; self.p.ids.len()];
        for i in 0..self.locked.len() {
            if self.locked[i].is_none() || t.has[i] { continue; }
            self.reach(t, &flat, i32::MAX);
            if self.dist[i] >= 0 { self.take_path(t, i); }
        }
    }

    /// Keeps taking the path worth most per point. `noise` shakes the choice a little; worths are refreshed every `refresh` paths.
    fn grow(&mut self, t: &mut Tree, budget: usize, noise: f64, refresh: usize, order: Option<&mut Vec<usize>>) {
        let mut w = self.worth(t);
        let (mut k, mut taken) = (0, vec![]);
        while t.n < budget + 1 {
            let left = (budget + 1 - t.n) as i32;
            self.reach(t, &w, left);
            let (mut b, mut br) = (usize::MAX, 0.0);
            for q in 0..self.queue.len() {
                let i = self.queue[q];
                if self.dist[i] < 1 { continue; }
                let mut r = self.sum[i] / self.dist[i] as f64;
                if noise > 0.0 { r *= 1.0 - noise * self.rnd(); }
                if r > br + 1e-12 { br = r; b = i; }
            }
            if b == usize::MAX { break; }
            taken.extend(self.take_path(t, b));
            k += 1;
            if k % refresh == 0 { w = self.worth(t); }
        }
        t.u = self.p.eval(&t.x);
        if let Some(o) = order { *o = taken; }
    }

    /// Takes a node out, with everything that loses its way to the start.
    fn cut(&mut self, t: &mut Tree, i: usize) {
        self.remove(t, i);
        let mut seen = vec![false; t.has.len()];
        seen[self.start] = true;
        let mut q = vec![self.start];
        while let Some(a) = q.pop() { for &b in &self.adj[a] { if t.has[b] && !seen[b] { seen[b] = true; q.push(b); } } }
        for j in 0..t.has.len() { if t.has[j] && !seen[j] { self.remove(t, j); } }
    }

    fn degree(&self, t: &Tree, i: usize) -> usize { self.adj[i].iter().filter(|b| t.has[**b]).count() }

    fn trim(&mut self, t: &mut Tree, budget: usize) {
        while t.n > budget + 1 {
            let w = self.worth(t);
            let leaf = (0..t.has.len()).filter(|&i| t.has[i] && i != self.start && self.locked[i].is_none() && self.degree(t, i) == 1).min_by(|a, b| w[*a].total_cmp(&w[*b]));
            match leaf { Some(i) => self.remove(t, i), None => break }
        }
    }

    /// A tree for this many points; `rounds` = how often to cut and grow again.
    pub fn solve(&mut self, budget: usize, rounds: usize) -> Tree {
        let mut t = self.empty();
        self.join_locked(&mut t);
        self.grow(&mut t, budget, 0.0, 1, None);
        self.improve(t, budget, rounds)
    }

    pub fn improve(&mut self, from: Tree, budget: usize, rounds: usize) -> Tree {
        let (mut best, mut cur) = (from.clone(), from);
        for r in 0..rounds {
            let left = 1.0 - r as f64 / rounds as f64;
            let mut t = cur.clone();
            let goal = t.n.saturating_sub(4 + (self.rnd() * 26.0) as usize).max(20);
            let w = if self.rnd() < 0.5 { Some(self.worth(&t)) } else { None };
            let mut guard = 0;
            while t.n > goal && guard < 60 {
                guard += 1;
                let tries = if w.is_some() { 8 } else { 1 };
                let (mut c, mut cv) = (usize::MAX, f64::MAX);
                for _ in 0..tries {
                    let i = loop { let i = (self.rnd() * t.has.len() as f64) as usize; if t.has[i] && i != self.start { break i; } };
                    let v = w.as_ref().map_or(0.0, |w| w[i] + if self.degree(&t, i) > 1 { 0.004 } else { 0.0 });
                    if v < cv { cv = v; c = i; }
                }
                self.cut(&mut t, c);
            }
            self.join_locked(&mut t);
            self.trim(&mut t, budget);
            let noise = 0.35 * self.rnd();
            self.grow(&mut t, budget, noise, 3, None);
            let slack = 0.002 * left * self.rnd();
            if t.u > cur.u - slack { cur = t; }
            if cur.u > best.u { best = cur.clone(); }
        }
        let mut t = best.clone();
        self.join_locked(&mut t);
        self.trim(&mut t, budget);
        self.grow(&mut t, budget, 0.0, 1, None);
        t
    }

    /// The tree's nodes in the order worth taking them, start first; each joins one before it.
    pub fn order(&mut self, t: &Tree) -> Vec<u32> {
        let ban = self.ban.clone();
        for i in 0..self.ban.len() { self.ban[i] = !t.has[i]; }
        let mut s = self.empty();
        let mut order = vec![];
        self.grow(&mut s, t.n - 1, 0.0, 4, Some(&mut order));
        // Whatever has no worth of its own comes last.
        while s.n < t.n {
            let Some(i) = (0..t.has.len()).find(|&i| t.has[i] && !s.has[i] && self.adj[i].iter().any(|b| s.has[*b])) else { break };
            self.add(&mut s, i);
            order.push(i);
        }
        self.ban = ban;
        std::iter::once(self.start).chain(order).map(|i| self.p.ids[i]).collect()
    }
}
