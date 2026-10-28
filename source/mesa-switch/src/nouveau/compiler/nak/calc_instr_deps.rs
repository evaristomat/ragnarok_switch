// Copyright © 2022 Collabora, Ltd.
// SPDX-License-Identifier: MIT

use crate::api::{GetDebugFlags, DEBUG};
use crate::ir::*;
use crate::opt_instr_sched_common::estimate_block_weight;
use crate::reg_tracker::{RegRefIterable, RegTracker, SparseRegTracker};

use compiler::dataflow::{BackwardDataflow, ForwardDataflow};
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::max;
use std::hash::Hash;
use std::ops::Range;
use std::slice;

#[derive(Clone)]
enum RegUse<T: Clone> {
    None,
    Write(T),
    Reads(Vec<T>),
}

impl<T: Clone> RegUse<T> {
    pub fn deps(&self) -> &[T] {
        match self {
            RegUse::None => &[],
            RegUse::Write(dep) => slice::from_ref(dep),
            RegUse::Reads(deps) => &deps[..],
        }
    }

    pub fn clear(&mut self) -> Self {
        std::mem::replace(self, RegUse::None)
    }

    pub fn clear_write(&mut self) -> Self {
        if matches!(self, RegUse::Write(_)) {
            std::mem::replace(self, RegUse::None)
        } else {
            RegUse::None
        }
    }

    pub fn add_read(&mut self, dep: T) -> Self {
        match self {
            RegUse::None => {
                *self = RegUse::Reads(vec![dep]);
                RegUse::None
            }
            RegUse::Write(_) => {
                std::mem::replace(self, RegUse::Reads(vec![dep]))
            }
            RegUse::Reads(reads) => {
                reads.push(dep);
                RegUse::None
            }
        }
    }

    pub fn set_write(&mut self, dep: T) -> Self {
        std::mem::replace(self, RegUse::Write(dep))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum RegReadWrite {
    Read,
    Write,
}

/// Maps each register read/write to a value
/// a register can have multiple reads AND multiple writes at the same
/// point in time if it comes from a merge.
/// For edits inside a CFG block, a RegUseMap will never contain multiple
/// writes.
///
/// We need to track multiple reads as we don't know which one can cause
/// the highest latency for the interfering instruction (in RaW).  For the
/// same reason we might need to track both reads and writes in the case of
/// a CFG block with multiple successors.
/// We cannot flush writes after a read operation since we can still
/// encounter other, slower reads that could interfere with the write.
#[derive(Clone, PartialEq, Eq, Default)]
struct RegUseMap<K: Hash + Eq, V> {
    map: FxHashMap<(RegReadWrite, K), V>,
}

impl<K, V> RegUseMap<K, V>
where
    K: Copy + Default + Hash + Eq,
    V: Clone,
{
    pub fn add_read(&mut self, k: K, v: V) {
        self.map.insert((RegReadWrite::Read, k), v);
    }

    pub fn set_write(&mut self, k: K, v: V) {
        // Writes wait on all previous Reads and writes
        self.map.clear();
        self.map.insert((RegReadWrite::Write, k), v);
    }

    pub fn iter_reads(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map
            .iter()
            .filter(|(k, _v)| k.0 == RegReadWrite::Read)
            .map(|(k, v)| (&k.1, v))
    }

    pub fn iter_writes(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map
            .iter()
            .filter(|(k, _v)| k.0 == RegReadWrite::Write)
            .map(|(k, v)| (&k.1, v))
    }

    /// Merge two instances using a custom merger for value conflicts
    pub fn merge_with(
        &mut self,
        other: &Self,
        mut merger: impl FnMut(&V, &V) -> V,
    ) {
        use std::collections::hash_map::Entry;
        for (k, v) in other.map.iter() {
            match self.map.entry(*k) {
                Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(v.clone());
                }
                Entry::Occupied(mut occupied_entry) => {
                    let orig = occupied_entry.get_mut();
                    *orig = merger(orig, v);
                }
            }
        }
    }
}

struct DepNode {
    read_dep: Option<usize>,
    first_wait: Option<(usize, usize)>,
}

struct DepGraph {
    deps: Vec<DepNode>,
    instr_deps: FxHashMap<(usize, usize), (usize, usize)>,
    instr_waits: FxHashMap<(usize, usize), Vec<usize>>,
    active: FxHashSet<usize>,
}

impl DepGraph {
    pub fn new() -> Self {
        Self {
            deps: Vec::new(),
            instr_deps: Default::default(),
            instr_waits: Default::default(),
            active: Default::default(),
        }
    }

    fn add_new_dep(&mut self, read_dep: Option<usize>) -> usize {
        let dep = self.deps.len();
        self.deps.push(DepNode {
            read_dep: read_dep,
            first_wait: None,
        });
        dep
    }

    pub fn add_instr(&mut self, block_idx: usize, ip: usize) -> (usize, usize) {
        let rd = self.add_new_dep(None);
        let wr = self.add_new_dep(Some(rd));
        self.instr_deps.insert((block_idx, ip), (rd, wr));
        (rd, wr)
    }

    pub fn add_signal(&mut self, dep: usize) {
        self.active.insert(dep);
    }

    pub fn add_waits(
        &mut self,
        block_idx: usize,
        ip: usize,
        mut waits: Vec<usize>,
    ) {
        for dep in &waits {
            // A wait on a write automatically waits on the read.  By removing
            // it from the active set here we ensure that we don't record any
            // duplicate write/read waits in the retain below.
            if let Some(rd) = &self.deps[*dep].read_dep {
                self.active.remove(rd);
            }
        }

        waits.retain(|dep| {
            let node = &mut self.deps[*dep];
            if let Some(wait) = node.first_wait {
                // Someone has already waited on this dep
                debug_assert!(!self.active.contains(dep));
                debug_assert!((block_idx, ip) >= wait);
                false
            } else if !self.active.contains(dep) {
                // Even if it doesn't have a use, it may still be deactivated.
                // This can happen if we depend the the destination before any
                // of its sources.
                false
            } else {
                self.deps[*dep].first_wait = Some((block_idx, ip));
                self.active.remove(dep);
                true
            }
        });

        // Sort for stability.  The list of waits may come from a HashSet (see
        // add_barrier()) and so it's not guaranteed stable across Rust
        // versions.  This also ensures that everything always waits on oldest
        // dependencies first.
        waits.sort();

        let _old = self.instr_waits.insert((block_idx, ip), waits);
        debug_assert!(_old.is_none());
    }

    pub fn add_barrier(&mut self, block_idx: usize, ip: usize) {
        let waits = self.active.iter().cloned().collect();
        self.add_waits(block_idx, ip, waits);
        debug_assert!(self.active.is_empty());
    }

    pub fn dep_is_waited_after(
        &self,
        dep: usize,
        block_idx: usize,
        ip: usize,
    ) -> bool {
        if let Some(wait) = self.deps[dep].first_wait {
            wait > (block_idx, ip)
        } else {
            false
        }
    }

    pub fn get_instr_deps(
        &self,
        block_idx: usize,
        ip: usize,
    ) -> (usize, usize) {
        *self.instr_deps.get(&(block_idx, ip)).unwrap()
    }

    pub fn get_instr_waits(&self, block_idx: usize, ip: usize) -> &[usize] {
        if let Some(waits) = self.instr_waits.get(&(block_idx, ip)) {
            &waits[..]
        } else {
            &[]
        }
    }
}

struct BarAlloc {
    num_bars: u8,
    bar_dep: [usize; 6],
}

impl BarAlloc {
    pub fn new() -> BarAlloc {
        BarAlloc {
            num_bars: 6,
            bar_dep: [usize::MAX; 6],
        }
    }

    pub fn bar_is_free(&self, bar: u8) -> bool {
        debug_assert!(bar < self.num_bars);
        self.bar_dep[usize::from(bar)] == usize::MAX
    }

    pub fn set_bar_dep(&mut self, bar: u8, dep: usize) {
        debug_assert!(self.bar_is_free(bar));
        self.bar_dep[usize::from(bar)] = dep;
    }

    pub fn free_bar(&mut self, bar: u8) {
        debug_assert!(!self.bar_is_free(bar));
        self.bar_dep[usize::from(bar)] = usize::MAX;
    }

    pub fn try_find_free_bar(&self) -> Option<u8> {
        (0..self.num_bars).find(|&bar| self.bar_is_free(bar))
    }

    pub fn free_some_bar(&mut self) -> u8 {
        // Get the oldest by looking for the one with the smallest dep
        let mut bar = 0;
        for b in 1..self.num_bars {
            if self.bar_dep[usize::from(b)] < self.bar_dep[usize::from(bar)] {
                bar = b;
            }
        }
        self.free_bar(bar);
        bar
    }

    pub fn get_bar_for_dep(&self, dep: usize) -> Option<u8> {
        (0..self.num_bars).find(|&bar| self.bar_dep[usize::from(bar)] == dep)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct TexQueueSimulationEntry {
    min_pos: u8,
}

impl TexQueueSimulationEntry {
    const INVALID: Self = TexQueueSimulationEntry { min_pos: u8::MAX };

    // First element on the queue
    const FIRST: Self = TexQueueSimulationEntry { min_pos: 0 };

    fn is_valid(&self) -> bool {
        if *self == Self::INVALID {
            false
        } else {
            debug_assert!(self.min_pos <= OpTexDepBar::MAX_TEXTURES_LEFT);
            true
        }
    }

    fn push(&mut self) {
        if self.is_valid() {
            self.min_pos += 1;
        }
    }

    fn flush_after(&mut self, pos: u8) -> bool {
        if self.min_pos < pos {
            true
        } else {
            // This entry is either invalid or higher than the cull level
            *self = Self::INVALID;
            false
        }
    }

    fn merge(&mut self, other: &Self) {
        self.min_pos = self.min_pos.min(other.min_pos);
    }
}

/// Simulate the state of a register in the queue, in buckets of 4
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct TexQueueSimulationBucket {
    entries: [TexQueueSimulationEntry; 4],
}

impl TexQueueSimulationBucket {
    const EMPTY: Self = TexQueueSimulationBucket {
        entries: [TexQueueSimulationEntry::INVALID; 4],
    };

    fn min_queue_position(&self, range: Range<usize>) -> Option<u8> {
        self.entries[range]
            .iter()
            .filter(|x| x.is_valid())
            .map(|x| x.min_pos)
            .min()
    }

    fn set_as_first(&mut self, range: Range<usize>) {
        for i in range {
            debug_assert!(!self.entries[i].is_valid());
            self.entries[i] = TexQueueSimulationEntry::FIRST;
        }
    }

    fn push(&mut self) {
        for entry in &mut self.entries {
            entry.push();
        }
    }

    fn flush_after(&mut self, pos: u8) -> bool {
        debug_assert!(pos <= OpTexDepBar::MAX_TEXTURES_LEFT);

        let mut retain = false;
        for x in &mut self.entries {
            retain |= x.flush_after(pos);
        }
        retain
    }

    fn merge(&mut self, other: &Self) {
        for (x, y) in self.entries.iter_mut().zip(other.entries.iter()) {
            x.merge(y);
        }
    }
}

/// This state simulates the texture queue for each destination.
///
/// For example, at the start the queue is always empty, but if we encounter a
/// tex operation that writes in r4..r8, that is pushed on the queue at
/// position 0.  If we encounter another tex operation that only writes r5,
/// that will be pushed at position 0 and the old tex instruction will be in
/// position 1.  This data-structure keeps track of the position of the queue
/// for each destination register present in the queue, push operations
/// correspond to new texture instructions, while flush operations correspond to
/// the usage of registers which may still be on the queue.
///
/// Since all Kepler texture operations use at most 4 registers, and many
/// instruction use more than one destination at a time, we group registers in
/// buckets of 4.  With this optimization each RegRef only accesses a single
/// bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexQueueSimulationState {
    /// Min position of the destination register in the queue,
    /// in buckets of 4 (indexed by register_index / 4).
    queue_pos: FxHashMap<u8, TexQueueSimulationBucket>,
    /// Max length of the queue, needed to check for overflows
    max_queue_len: u8,
}

impl TexQueueSimulationState {
    pub fn new() -> Self {
        TexQueueSimulationState {
            queue_pos: Default::default(),
            max_queue_len: 0,
        }
    }

    /// Translate from RegRef to bucket_index + bucket_range
    #[inline]
    fn reg_ref_to_coords(reg: RegRef) -> (u8, Range<usize>) {
        debug_assert!(reg.base_idx() <= u8::MAX.into());
        let idx = reg.base_idx() as u8 / 4;
        let sub = (reg.base_idx() % 4) as usize;

        let range = sub..(sub + reg.comps() as usize);
        assert!(range.end <= 4);
        (idx, range)
    }

    fn min_queue_position(&self, reg: RegRef) -> Option<u8> {
        let (idx, range) = Self::reg_ref_to_coords(reg);

        self.queue_pos
            .get(&idx)
            .and_then(|x| x.min_queue_position(range))
    }

    fn is_queue_full(&self) -> bool {
        // MAX_TEXTURES_LEFT describes the maximum number encodable
        // in the texdepbar, but the queue must have an element more.
        self.max_queue_len > OpTexDepBar::MAX_TEXTURES_LEFT
    }

    /// Flush every element whose position >= pos
    ///
    /// Effectively simulates the execution of a `texdepbar pos`
    fn flush_after(&mut self, pos: u8) {
        self.max_queue_len = self.max_queue_len.min(pos);
        self.queue_pos.retain(|_, v| v.flush_after(pos));
    }

    pub fn push(&mut self, reg: RegRef) -> Option<u8> {
        // Assert we are not on the queue
        debug_assert!(self.min_queue_position(reg).is_none());

        // Check that the push operation does not overflow the queue,
        // if it does, we must insert a barrier
        let mut tex_bar = None;
        if self.is_queue_full() {
            // The queue is full, there are 64 in-flight tex-ops.
            // make space by making removing 1 texture.
            tex_bar = Some(OpTexDepBar::MAX_TEXTURES_LEFT);
            self.flush_after(OpTexDepBar::MAX_TEXTURES_LEFT);
            // Now the queue is not full anymore
            debug_assert!(!self.is_queue_full());
        }

        self.max_queue_len += 1;
        // Every entry is pushed by 1
        for x in self.queue_pos.values_mut() {
            x.push();
        }

        // Put us on the queue as first
        let (idx, range) = Self::reg_ref_to_coords(reg);
        self.queue_pos
            .entry(idx)
            .or_insert(TexQueueSimulationBucket::EMPTY)
            .set_as_first(range);

        tex_bar
    }

    pub fn flush(&mut self, reg: RegRef) -> Option<u8> {
        let queue_pos = self.min_queue_position(reg);

        let Some(queue_pos) = queue_pos else {
            return None; // Not in queue
        };

        // Cut the queue
        self.flush_after(queue_pos);
        debug_assert!(self.min_queue_position(reg).is_none());

        Some(queue_pos)
    }

    pub fn merge(&mut self, other: &Self) {
        self.max_queue_len = self.max_queue_len.max(other.max_queue_len);
        for (key, y) in other.queue_pos.iter() {
            let x = self
                .queue_pos
                .entry(*key)
                .or_insert(TexQueueSimulationBucket::EMPTY);
            x.merge(y);
        }
    }

    /// Simulates the execution of an instruction and returns the
    /// barrier level needed.
    pub fn visit_instr(&mut self, instr: &Instr) -> Option<u8> {
        // Flush register reads and writes
        // (avoid write-after-write and read-after-write hazards)
        // Compute the minimum required flush level (for barriers)
        let flush_level = if !self.queue_pos.is_empty() {
            let src_refs =
                instr.srcs().iter().filter_map(|x| x.src_ref.as_reg());
            let dst_refs = instr.dsts().iter().filter_map(|x| x.as_reg());

            src_refs
                .chain(dst_refs)
                .filter_map(|reg_ref| self.flush(*reg_ref))
                .reduce(|a, b| a.min(b))
        } else {
            // The queue is empty, no need to check the instruction
            None
        };

        // Push registers (if we are a tex instruction)
        // We might need to insert a barrier if the queue is full
        let push_level = if instr_needs_texbar(instr) {
            let dst = instr.dsts()[0].as_reg().unwrap();
            self.push(*dst)
        } else {
            None
        };

        // If the flush needs a barrier, the queue will not be full,
        // therefore the push will not need a barrier.
        debug_assert!(flush_level.is_none() || push_level.is_none());
        flush_level.or(push_level)
    }
}

fn instr_needs_texbar(instr: &Instr) -> bool {
    matches!(
        instr.op,
        Op::Tex(_)
            | Op::Tld(_)
            | Op::Tmml(_)
            | Op::Tld4(_)
            | Op::Txd(_)
            | Op::Txq(_)
    )
}

/// Hardware has a FIFO queue of texture that are still fetching,
/// when the oldest tex finishes executing, it's written to the reg,
/// removed from the queue and it begins executing the new one.
/// The problem arises when a texture is read while it is still being fetched
/// to avoid it, we have a `texdepbar {i}` instruction that stalls until
/// the texture fetch queue has at most {i} elements.
/// e.g. the most simple solution is to have texdepbar 0 after each texture
/// instruction, but this would stall the pipeline until the texture fetch
/// finishes executing.
/// This algorithm inserts `texdepbar` at each use of the texture results,
/// simulating the texture queue execution.
///
/// Note that the texture queue has for each entry (texture data, register output)
/// and each register can be on the queue only once (we don't want to have multiple texture
/// operations in-flight that write to the same registers).
/// This can lead to a neat algorithm:
/// instead of tracking the queue directly, which can exponentially explode in complexity,
/// track the position of each register, which needs at most 255/63 positions.
/// For branches the state is duplicated in each basic block,
/// for joins instead we want to keep both the minimum position of each
/// entry and the maximum length og the queue to avoid overflows.
///
/// TODO: IF this pass is too slow, there are still optimizations left:
/// - Our data-flow computes barrier levels and discards them,
///   but since most CFG blocks do not need recomputation, we could save
///   the barrier levels in a vec and save a pass later.
/// - Instead of pushing by 1 each element in the queue on a `push` op,
///   we could keep track of an in-flight range and use a wrapping timestamp
///   this improves performance but needs careful implementation to avoid bugs
fn insert_texture_barriers(f: &mut Function, sm: &ShaderModelInfo) {
    assert!(sm.is_kepler()); // Only kepler has texture barriers!

    let mut state_in: Vec<_> = (0..f.blocks.len())
        .map(|_| TexQueueSimulationState::new())
        .collect();
    let mut state_out: Vec<_> = (0..f.blocks.len())
        .map(|_| TexQueueSimulationState::new())
        .collect();
    ForwardDataflow {
        cfg: &f.blocks,
        block_in: &mut state_in[..],
        block_out: &mut state_out[..],
        transfer: |_block_idx, block, sim_out, sim_in| {
            let mut sim = sim_in.clone();

            for instr in block.instrs.iter() {
                // Ignore the barrier, we will recompute this later
                let _bar = sim.visit_instr(instr);
            }

            if *sim_out == sim {
                false
            } else {
                *sim_out = sim;
                true
            }
        },
        join: |sim_in, pred_sim_out| {
            sim_in.merge(pred_sim_out);
        },
    }
    .solve();

    for (block, mut sim) in f.blocks.iter_mut().zip(state_in.into_iter()) {
        block.map_instrs(|instr| {
            if let Some(textures_left) = sim.visit_instr(&instr) {
                let bar = Instr::new(OpTexDepBar { textures_left });
                [bar, instr].into()
            } else {
                [instr].into()
            }
        });
    }
}

fn assign_barriers(f: &mut Function, sm: &ShaderModelInfo) {
    let mut uses = Box::new(RegTracker::new_with(&|| RegUse::None));
    let mut deps = DepGraph::new();

    for (bi, b) in f.blocks.iter().enumerate() {
        for (ip, instr) in b.instrs.iter().enumerate() {
            if instr.is_branch() {
                deps.add_barrier(bi, ip);
            } else {
                // Execution predicates are handled immediately and we don't
                // need barriers for them, regardless of whether or not it's a
                // fixed-latency instruction.
                let mut waits = Vec::new();
                uses.for_each_instr_pred_mut(instr, |u| {
                    let u = u.clear_write();
                    waits.extend_from_slice(u.deps());
                });

                if sm.op_needs_scoreboard(&instr.op) {
                    let (rd, wr) = deps.add_instr(bi, ip);
                    uses.for_each_instr_src_mut(instr, |_, u| {
                        // Only mark a dep as signaled if we actually have
                        // something that shows up in the register file as
                        // needing scoreboarding
                        deps.add_signal(rd);
                        let u = u.add_read(rd);
                        waits.extend_from_slice(u.deps());
                    });
                    uses.for_each_instr_dst_mut(instr, |_, u| {
                        // Only mark a dep as signaled if we actually have
                        // something that shows up in the register file as
                        // needing scoreboarding
                        deps.add_signal(wr);
                        let u = u.set_write(wr);
                        for dep in u.deps() {
                            // Don't wait on ourselves
                            if *dep != rd {
                                waits.push(*dep);
                            }
                        }
                    });
                } else {
                    // Delays will cover us here.  We just need to make sure
                    // that we wait on any uses that we consume.
                    uses.for_each_instr_src_mut(instr, |_, u| {
                        let u = u.clear_write();
                        waits.extend_from_slice(u.deps());
                    });
                    uses.for_each_instr_dst_mut(instr, |_, u| {
                        let u = u.clear();
                        waits.extend_from_slice(u.deps());
                    });
                }
                deps.add_waits(bi, ip, waits);
            }
        }
    }

    let mut bars = BarAlloc::new();

    for (bi, b) in f.blocks.iter_mut().enumerate() {
        for (ip, instr) in b.instrs.iter_mut().enumerate() {
            let mut wait_mask = 0_u8;
            for dep in deps.get_instr_waits(bi, ip) {
                if let Some(bar) = bars.get_bar_for_dep(*dep) {
                    wait_mask |= 1 << bar;
                    bars.free_bar(bar);
                }
            }
            instr.deps.add_wt_bar_mask(wait_mask);

            if instr.needs_yield() {
                instr.deps.set_yield(true);
            }

            if !sm.op_needs_scoreboard(&instr.op) {
                continue;
            }

            let (rd_dep, wr_dep) = deps.get_instr_deps(bi, ip);
            if deps.dep_is_waited_after(rd_dep, bi, ip) {
                let rd_bar = bars.try_find_free_bar().unwrap_or_else(|| {
                    let bar = bars.free_some_bar();
                    instr.deps.add_wt_bar(bar);
                    bar
                });
                bars.set_bar_dep(rd_bar, rd_dep);
                instr.deps.set_rd_bar(rd_bar);
            }
            if deps.dep_is_waited_after(wr_dep, bi, ip) {
                let wr_bar = bars.try_find_free_bar().unwrap_or_else(|| {
                    let bar = bars.free_some_bar();
                    instr.deps.add_wt_bar(bar);
                    bar
                });
                bars.set_bar_dep(wr_bar, wr_dep);
                instr.deps.set_wr_bar(wr_bar);
            }
        }
    }
}

// Cross-block scoreboards for SM50-SM6x (NAK_DEBUG=xbar).
//
// assign_barriers() waits on every outstanding scoreboard at every branch,
// including the SYNC that ends each arm of an if/else.  A fragment shader
// that samples inside if/else or switch arms therefore pays the full latency
// of each sample at that arm's SYNC.  This pass keeps scoreboards in flight
// across forward BRA and SYNC and waits where the result is used.  It relies
// on two hardware properties:
//
//  - Scoreboards count outstanding operations (DEPBAR.LE SBn, k), so two
//    operations may share one and a wait covers both.  nv50_ir's GM107
//    emitter shares SB5 when it runs out.
//  - An operation issued under a divergent mask writes only the lanes that
//    were active, so the other side of a divergent branch, which may run
//    straight after a SYNC, can't see its writes before the lanes reconverge.
//    That's why the analysis only follows CFG edges.
//
// Back-edges, CONT and EXIT still wait on everything, so the forward edges
// form a DAG that one pass in block order covers exactly.  So does BRK,
// unless NAK_DEBUG=xbrk is also set.  A BRK leaves its loop for the lanes
// that take it, and with xbrk it may leave their scoreboards in flight like
// a forward BRA: the lanes still looping write only their own lanes, as on
// the other side of a divergent branch, and the BRK's edge goes forward to
// the loop's exit.  It is what lets a lookup made in a loop, such as the
// one NVK makes of a texture whose descriptor may differ across the warp,
// overlap whatever follows the loop rather than wait out its latency at
// the BRK.  The result is checked by xbar_verify(), an independent hazard
// check over the whole CFG.  The caller keeps assign_barriers()'s result if
// the check fails, or if it rejects assign_barriers()'s own result.
//
// NAK_DEBUG=share changes what happens when all six scoreboards are taken.
// Without it, the new operation waits for the oldest, as assign_barriers()
// does.  A texture fetch or memory load usually needs two, one for reading
// its sources and one for writing its result, so a run of loads whose
// results are used after the run, such as a min over a block of texels,
// has only three in flight: from the fourth on, each waits for the result of
// the one three before it.  With share, a texture fetch or memory load takes
// a scoreboard held only by operations of its kind (texture fetches, or
// loads from the same memory space) for the same purpose (reading or
// writing), at most XBAR_SHARE_MAX of them.  Of those it takes the one first
// waited on latest, so it has the longest time in flight before a wait
// covers it.  The others still wait for the oldest.  This relies on the
// first property above and on loads of one kind going out in order: maxas
// found Maxwell's memory loads share a scoreboard reliably and S2Rs, which
// can be processed out of order, don't.

/// The kind of operation that may share a scoreboard with others of its
/// kind under NAK_DEBUG=share: texture fetches, and loads from each memory
/// space.
fn xbar_share_class(op: &Op) -> Option<u8> {
    match op {
        Op::Tex(_)
        | Op::Tld(_)
        | Op::Tld4(_)
        | Op::Tmml(_)
        | Op::Txd(_)
        | Op::Txq(_) => Some(0),
        Op::Ld(ld) => Some(match ld.access.space {
            MemSpace::Global(_) => 1,
            MemSpace::Local => 2,
            MemSpace::Shared => 3,
        }),
        _ => None,
    }
}

/// The most deps one scoreboard holds under NAK_DEBUG=share.  maxas
/// measured a warp's queue of global loads at about eight, so more on one
/// scoreboard would mostly wait in that queue.
const XBAR_SHARE_MAX: usize = 8;

/// Scoreboarded accesses to one register that may still be in flight,
/// merged over all forward paths (sorted dep indices).
#[derive(Clone, Default, PartialEq, Eq)]
struct XbarUse {
    writes: Vec<usize>,
    reads: Vec<usize>,
}

fn xbar_union(a: &mut Vec<usize>, b: &[usize]) {
    if !b.is_empty() {
        a.extend_from_slice(b);
        a.sort_unstable();
        a.dedup();
    }
}

#[derive(Clone, Default)]
struct XbarDepState {
    uses: SparseRegTracker<XbarUse>,
    active: FxHashSet<usize>,
}

impl XbarDepState {
    fn merge(&mut self, other: &Self) {
        self.uses.merge_with(&other.uses, |a, b| {
            xbar_union(&mut a.writes, &b.writes);
            xbar_union(&mut a.reads, &b.reads);
        });
        self.active.extend(other.active.iter().copied());
    }
}

#[derive(Default)]
struct XbarDeps {
    /// For a write dep, the read dep of the same instruction
    read_dep: Vec<Option<usize>>,
    /// Whether some path waits on the dep
    waited: Vec<bool>,
    instr_deps: FxHashMap<(usize, usize), (usize, usize)>,
    instr_waits: FxHashMap<(usize, usize), Vec<usize>>,
    /// For NAK_DEBUG=share, each dep's xbar_share_class() times two, plus
    /// one for a write.  Deps with the same key may share a scoreboard.
    share_key: Vec<Option<u8>>,
}

impl XbarDeps {
    fn add_instr(
        &mut self,
        bi: usize,
        ip: usize,
        class: Option<u8>,
    ) -> (usize, usize) {
        let rd = self.read_dep.len();
        let wr = rd + 1;
        self.read_dep.extend([None, Some(rd)]);
        self.waited.extend([false, false]);
        self.share_key
            .extend([class.map(|c| c * 2), class.map(|c| c * 2 + 1)]);
        self.instr_deps.insert((bi, ip), (rd, wr));
        (rd, wr)
    }

    fn add_waits(
        &mut self,
        active: &mut FxHashSet<usize>,
        bi: usize,
        ip: usize,
        mut waits: Vec<usize>,
    ) {
        // As in DepGraph::add_waits(), a wait on a write covers the read.
        for dep in &waits {
            if let Some(rd) = self.read_dep[*dep] {
                active.remove(&rd);
            }
        }
        waits.sort_unstable();
        waits.dedup();
        waits.retain(|dep| active.remove(dep));
        for dep in &waits {
            self.waited[*dep] = true;
        }
        if !waits.is_empty() {
            self.instr_waits.insert((bi, ip), waits);
        }
    }

    fn instr_waits(&self, bi: usize, ip: usize) -> &[usize] {
        self.instr_waits.get(&(bi, ip)).map_or(&[], |w| &w[..])
    }
}

/// Whether `instr`, at `ip` in block `bi`, may leave scoreboards in flight:
/// a BRA or SYNC, or with `keep_brk` a BRK, that ends its block and only
/// goes forward.
fn xbar_branch_keeps_bars(
    f: &Function,
    bi: usize,
    ip: usize,
    instr: &Instr,
    keep_brk: bool,
) -> bool {
    (matches!(&instr.op, Op::Bra(_) | Op::Sync(_))
        || keep_brk && matches!(&instr.op, Op::Brk(_)))
        && ip + 1 == f.blocks[bi].instrs.len()
        && f.blocks.succ_indices(bi).iter().all(|&s| s > bi)
}

/// Every edge to an earlier block must leave through a branch, which then
/// waits on everything.
fn xbar_check_cfg(f: &Function) -> Result<(), String> {
    for bi in 0..f.blocks.len() {
        if f.blocks.succ_indices(bi).iter().any(|&s| s <= bi)
            && !f.blocks[bi].instrs.last().is_some_and(|i| i.is_branch())
        {
            return Err(format!("block {bi} has a back-edge without a branch"));
        }
    }
    Ok(())
}

/// Pass 1: the deps each instruction waits on.  Mirrors the loop in
/// assign_barriers(), with per-block state merged at forward joins.
fn xbar_calc_waits(
    f: &Function,
    sm: &ShaderModelInfo,
    keep_brk: bool,
) -> XbarDeps {
    let mut deps = XbarDeps::default();
    let mut block_in: Vec<Option<XbarDepState>> =
        (0..f.blocks.len()).map(|_| None).collect();

    for bi in 0..f.blocks.len() {
        let mut st = block_in[bi].take().unwrap_or_default();
        for (ip, instr) in f.blocks[bi].instrs.iter().enumerate() {
            let mut waits = Vec::new();
            if instr.is_branch()
                && !xbar_branch_keeps_bars(f, bi, ip, instr, keep_brk)
            {
                waits.extend(st.active.iter().copied());
            } else {
                st.uses.for_each_instr_pred_mut(instr, |u| {
                    waits.append(&mut u.writes);
                });
                if sm.op_needs_scoreboard(&instr.op) {
                    let (rd, wr) =
                        deps.add_instr(bi, ip, xbar_share_class(&instr.op));
                    let mut has_src = false;
                    let mut has_dst = false;
                    st.uses.for_each_instr_src_mut(instr, |_, u| {
                        has_src = true;
                        waits.append(&mut u.writes);
                        if !u.reads.contains(&rd) {
                            u.reads.push(rd);
                        }
                    });
                    st.uses.for_each_instr_dst_mut(instr, |_, u| {
                        has_dst = true;
                        waits.append(&mut u.writes);
                        waits.extend(u.reads.drain(..).filter(|&d| d != rd));
                        u.writes.push(wr);
                    });
                    if has_src {
                        st.active.insert(rd);
                    }
                    if has_dst {
                        st.active.insert(wr);
                    }
                } else {
                    st.uses.for_each_instr_src_mut(instr, |_, u| {
                        waits.append(&mut u.writes);
                    });
                    st.uses.for_each_instr_dst_mut(instr, |_, u| {
                        waits.append(&mut u.writes);
                        waits.append(&mut u.reads);
                    });
                }
            }
            deps.add_waits(&mut st.active, bi, ip, waits);
        }

        for &s in f.blocks.succ_indices(bi) {
            if s > bi {
                match &mut block_in[s] {
                    Some(s_in) => s_in.merge(&st),
                    None => block_in[s] = Some(st.clone()),
                }
            }
        }
    }
    deps
}

#[derive(Clone, Copy, Default)]
struct XbarPlan {
    wt_bar_mask: u8,
    rd_bar: Option<u8>,
    wr_bar: Option<u8>,
}

/// Deps that may hold each scoreboard, merged over all forward paths
#[derive(Clone, Default)]
struct XbarBarState {
    bars: [Vec<usize>; 6],
}

impl XbarBarState {
    fn merge(&mut self, other: &Self) {
        for (a, b) in self.bars.iter_mut().zip(other.bars.iter()) {
            xbar_union(a, b);
        }
    }

    fn wait(&mut self, mask: u8, read_dep: &[Option<usize>]) {
        let mut done_reads = Vec::new();
        for (b, bar) in self.bars.iter_mut().enumerate() {
            if mask & (1 << b) != 0 {
                done_reads.extend(bar.iter().filter_map(|d| read_dep[*d]));
                bar.clear();
            }
        }
        if !done_reads.is_empty() {
            for bar in &mut self.bars {
                bar.retain(|d| !done_reads.contains(d));
            }
        }
    }

    /// Pick a scoreboard that is free on every path, one that `dep` can
    /// share, or wait on the one holding the oldest dep.
    fn alloc(
        &mut self,
        plan: &mut XbarPlan,
        other: Option<u8>,
        dep: usize,
        read_dep: &[Option<usize>],
        share: Option<&XbarShare>,
    ) -> u8 {
        let mut oldest: Option<u8> = None;
        for b in 0..6_u8 {
            if Some(b) == other {
                continue;
            }
            let bar = &self.bars[usize::from(b)];
            if bar.is_empty() {
                return b;
            }
            if oldest.is_none_or(|o| bar[0] < self.bars[usize::from(o)][0]) {
                oldest = Some(b);
            }
        }
        if let Some(b) = share.and_then(|s| self.shareable(s, other, dep)) {
            return b;
        }
        let b = oldest.unwrap();
        plan.wt_bar_mask |= 1 << b;
        self.wait(1 << b, read_dep);
        b
    }

    /// Of the scoreboards holding only deps with `dep`'s share key, and
    /// fewer than XBAR_SHARE_MAX, the one first waited on latest.  Called
    /// when none is free, so each one considered holds a dep.
    fn shareable(
        &self,
        share: &XbarShare,
        other: Option<u8>,
        dep: usize,
    ) -> Option<u8> {
        let key = share.key[dep]?;
        let mut best: Option<(u8, usize)> = None;
        for b in 0..6_u8 {
            let bar = &self.bars[usize::from(b)];
            if Some(b) == other
                || bar.len() >= XBAR_SHARE_MAX
                || bar.iter().any(|d| share.key[*d] != Some(key))
            {
                continue;
            }
            let need = bar.iter().map(|d| share.first_wait[*d]).min().unwrap();
            if best.is_none_or(|(_, n)| need > n) {
                best = Some((b, need));
            }
        }
        best.map(|(b, _)| b)
    }
}

/// What NAK_DEBUG=share needs to pick a scoreboard to share
struct XbarShare<'a> {
    key: &'a [Option<u8>],
    /// Where each dep is first waited on, counting instructions in block
    /// order
    first_wait: Vec<usize>,
}

impl<'a> XbarShare<'a> {
    fn new(f: &Function, deps: &'a XbarDeps) -> Self {
        let mut first_wait = vec![usize::MAX; deps.read_dep.len()];
        let mut at = 0;
        for (bi, b) in f.blocks.iter().enumerate() {
            for ip in 0..b.instrs.len() {
                for &d in deps.instr_waits(bi, ip) {
                    first_wait[d] = first_wait[d].min(at);
                }
                at += 1;
            }
        }
        XbarShare {
            key: &deps.share_key,
            first_wait,
        }
    }
}

/// Pass 2: scoreboard numbers and wait masks.
fn xbar_assign_bars(
    f: &Function,
    sm: &ShaderModelInfo,
    deps: &XbarDeps,
    keep_brk: bool,
    share: bool,
) -> Vec<Vec<XbarPlan>> {
    let share = share.then(|| XbarShare::new(f, deps));
    let mut plans: Vec<Vec<XbarPlan>> = f
        .blocks
        .iter()
        .map(|b| vec![XbarPlan::default(); b.instrs.len()])
        .collect();
    let mut block_in: Vec<Option<XbarBarState>> =
        (0..f.blocks.len()).map(|_| None).collect();

    for bi in 0..f.blocks.len() {
        let mut st = block_in[bi].take().unwrap_or_default();
        for (ip, instr) in f.blocks[bi].instrs.iter().enumerate() {
            let plan = &mut plans[bi][ip];
            let mut mask = 0_u8;
            if instr.is_branch()
                && !xbar_branch_keeps_bars(f, bi, ip, instr, keep_brk)
            {
                for (b, bar) in st.bars.iter().enumerate() {
                    if !bar.is_empty() {
                        mask |= 1 << b;
                    }
                }
            } else {
                let waits = deps.instr_waits(bi, ip);
                for (b, bar) in st.bars.iter().enumerate() {
                    if bar.iter().any(|d| waits.binary_search(d).is_ok()) {
                        mask |= 1 << b;
                    }
                }
            }
            st.wait(mask, &deps.read_dep);
            plan.wt_bar_mask = mask;

            if sm.op_needs_scoreboard(&instr.op) {
                let (rd, wr) = deps.instr_deps[&(bi, ip)];
                if deps.waited[rd] {
                    let b = st.alloc(
                        plan,
                        None,
                        rd,
                        &deps.read_dep,
                        share.as_ref(),
                    );
                    st.bars[usize::from(b)].push(rd);
                    plan.rd_bar = Some(b);
                }
                if deps.waited[wr] {
                    let rd_bar = plan.rd_bar;
                    let b = st.alloc(
                        plan,
                        rd_bar,
                        wr,
                        &deps.read_dep,
                        share.as_ref(),
                    );
                    st.bars[usize::from(b)].push(wr);
                    plan.wr_bar = Some(b);
                }
            }
        }

        for &s in f.blocks.succ_indices(bi) {
            if s > bi {
                match &mut block_in[s] {
                    Some(s_in) => s_in.merge(&st),
                    None => block_in[s] = Some(st.clone()),
                }
            }
        }
    }
    plans
}

/// The registers an instruction touches, as a bit set
#[derive(Clone, Copy, Default)]
struct XbarRegSet {
    bits: [u64; 7],
}

impl XbarRegSet {
    fn intersects(&self, other: &Self) -> bool {
        self.bits.iter().zip(other.bits.iter()).any(|(a, b)| a & b != 0)
    }
}

impl RegRefIterable<()> for XbarRegSet {
    fn for_each_ref_mut(&mut self, reg: RegRef, mut f: impl FnMut(&mut ())) {
        let base = match reg.file() {
            RegFile::GPR => 0,
            RegFile::UGPR => 256,
            RegFile::Pred => 384,
            RegFile::UPred => 392,
            RegFile::Carry => 400,
            RegFile::Bar => return, // Barriers have a HW scoreboard
            RegFile::Mem => panic!("Not a register"),
        };
        for i in reg.idx_range() {
            let bit = base + usize::try_from(i).unwrap();
            self.bits[bit / 64] |= 1 << (bit % 64);
            f(&mut ());
        }
    }
}

struct XbarEvent {
    bar: Option<u8>,
    is_write: bool,
    /// For a write, the read event of the same instruction
    read_event: Option<usize>,
    regs: XbarRegSet,
    at: (usize, usize),
}

/// Checks that, on every CFG path, no instruction touches a register that a
/// scoreboarded instruction may still read or write.  A wait on a scoreboard
/// completes everything on it; finishing a write also finishes the reads of
/// the same instruction.  Independent of how the plan was made.
fn xbar_verify(
    f: &Function,
    sm: &ShaderModelInfo,
    plans: &[Vec<XbarPlan>],
) -> Result<(), String> {
    let mut events = Vec::new();
    let mut instr_events = Vec::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        let mut block_events = Vec::new();
        for (ip, instr) in b.instrs.iter().enumerate() {
            let mut new_events = Vec::new();
            if sm.op_needs_scoreboard(&instr.op) {
                let plan = &plans[bi][ip];
                let mut srcs = XbarRegSet::default();
                let mut src_count = 0;
                srcs.for_each_instr_src_mut(instr, |_, _| src_count += 1);
                let mut dsts = XbarRegSet::default();
                let mut dst_count = 0;
                dsts.for_each_instr_dst_mut(instr, |_, _| dst_count += 1);
                let mut read_event = None;
                if src_count > 0 {
                    read_event = Some(events.len());
                    new_events.push(events.len());
                    events.push(XbarEvent {
                        bar: plan.rd_bar,
                        is_write: false,
                        read_event: None,
                        regs: srcs,
                        at: (bi, ip),
                    });
                }
                if dst_count > 0 {
                    new_events.push(events.len());
                    events.push(XbarEvent {
                        bar: plan.wr_bar,
                        is_write: true,
                        read_event,
                        regs: dsts,
                        at: (bi, ip),
                    });
                }
            }
            block_events.push(new_events);
        }
        instr_events.push(block_events);
    }

    let words = events.len().div_ceil(64);
    let step = |bi: usize,
                ip: usize,
                pending: &mut Vec<u64>,
                check: bool|
     -> Result<(), String> {
        let instr = &f.blocks[bi].instrs[ip];
        let mask = plans[bi][ip].wt_bar_mask;
        if mask != 0 {
            for e in 0..events.len() {
                if pending[e / 64] & (1 << (e % 64)) == 0 {
                    continue;
                }
                let ev = &events[e];
                if ev.bar.is_some_and(|b| mask & (1 << b) != 0) {
                    pending[e / 64] &= !(1 << (e % 64));
                    if let Some(r) = ev.read_event {
                        pending[r / 64] &= !(1 << (r % 64));
                    }
                }
            }
        }

        if check {
            let mut reads = XbarRegSet::default();
            reads.for_each_instr_pred_mut(instr, |_| ());
            reads.for_each_instr_src_mut(instr, |_, _| ());
            let mut writes = XbarRegSet::default();
            writes.for_each_instr_dst_mut(instr, |_, _| ());
            for (e, ev) in events.iter().enumerate() {
                if pending[e / 64] & (1 << (e % 64)) == 0 {
                    continue;
                }
                let hazard = if ev.is_write && ev.regs.intersects(&reads) {
                    "read after write"
                } else if ev.is_write && ev.regs.intersects(&writes) {
                    "write after write"
                } else if !ev.is_write && ev.regs.intersects(&writes) {
                    "write after read"
                } else {
                    continue;
                };
                let (pb, pi) = ev.at;
                return Err(format!(
                    "{hazard} hazard: block {bi} instr {ip} ({}) and \
                     block {pb} instr {pi} ({})",
                    instr, f.blocks[pb].instrs[pi],
                ));
            }
        }

        for &e in &instr_events[bi][ip] {
            pending[e / 64] |= 1 << (e % 64);
        }
        Ok(())
    };

    let mut state_in = vec![vec![0_u64; words]; f.blocks.len()];
    let mut state_out = vec![vec![0_u64; words]; f.blocks.len()];
    ForwardDataflow {
        cfg: &f.blocks,
        block_in: &mut state_in[..],
        block_out: &mut state_out[..],
        transfer: |bi, block, out, pending_in| {
            let mut pending = pending_in.clone();
            for ip in 0..block.instrs.len() {
                step(bi, ip, &mut pending, false).unwrap();
            }
            if *out == pending {
                false
            } else {
                *out = pending;
                true
            }
        },
        join: |pending_in, pred_out| {
            for (a, b) in pending_in.iter_mut().zip(pred_out.iter()) {
                *a |= *b;
            }
        },
    }
    .solve();

    for (bi, b) in f.blocks.iter().enumerate() {
        let mut pending = state_in[bi].clone();
        for ip in 0..b.instrs.len() {
            step(bi, ip, &mut pending, true)?;
        }
    }
    Ok(())
}

fn xbar_plans_from_deps(f: &Function) -> Vec<Vec<XbarPlan>> {
    f.blocks
        .iter()
        .map(|b| {
            b.instrs
                .iter()
                .map(|i| XbarPlan {
                    wt_bar_mask: i.deps.wt_bar_mask,
                    rd_bar: i.deps.rd_bar(),
                    wr_bar: i.deps.wr_bar(),
                })
                .collect()
        })
        .collect()
}

/// Runs assign_barriers(), then replaces its result with the cross-block
/// one if both pass xbar_verify().  Returns why it kept the legacy result.
fn assign_barriers_xbar(
    f: &mut Function,
    sm: &ShaderModelInfo,
    keep_brk: bool,
    share: bool,
) -> Result<(), String> {
    let saved: Vec<Vec<(u8, bool, u8, u8)>> = f
        .blocks
        .iter()
        .map(|b| {
            b.instrs
                .iter()
                .map(|i| {
                    let d = &i.deps;
                    (d.delay, d.yld, d.wt_bar_mask, d.reuse_mask)
                })
                .collect()
        })
        .collect();

    assign_barriers(f, sm);
    xbar_verify(f, sm, &xbar_plans_from_deps(f))
        .map_err(|e| format!("verifier rejected legacy barriers: {e}"))?;

    xbar_check_cfg(f)?;
    let deps = xbar_calc_waits(f, sm, keep_brk);
    let plans = xbar_assign_bars(f, sm, &deps, keep_brk, share);
    xbar_verify(f, sm, &plans)?;

    for (bi, b) in f.blocks.iter_mut().enumerate() {
        for (ip, instr) in b.instrs.iter_mut().enumerate() {
            let (delay, yld, wt_bar_mask, reuse_mask) = saved[bi][ip];
            let plan = &plans[bi][ip];
            instr.deps = InstrDeps::new();
            instr.deps.delay = delay;
            instr.deps.yld = yld || instr.needs_yield();
            instr.deps.wt_bar_mask = wt_bar_mask;
            instr.deps.reuse_mask = reuse_mask;
            instr.deps.add_wt_bar_mask(plan.wt_bar_mask);
            if let Some(bar) = plan.rd_bar {
                instr.deps.set_rd_bar(bar);
            }
            if let Some(bar) = plan.wr_bar {
                instr.deps.set_wr_bar(bar);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RegOrigin {
    loc: InstrIdx,
    // Index of the src (for reads) or dst (for writes) in the instruction.
    src_dst_idx: u16,
}

impl Default for RegOrigin {
    fn default() -> Self {
        // Lower bound
        Self {
            loc: InstrIdx::new(0, 0),
            src_dst_idx: 0,
        }
    }
}

// Delay accumulated from the blocks it passed, used to check for cross-block hazards.
type AccumulatedDelay = u8;
type DelayRegTracker = SparseRegTracker<RegUseMap<RegOrigin, AccumulatedDelay>>;

struct BlockDelayScheduler<'a> {
    sm: &'a ShaderModelInfo,
    f: &'a Function,
    // Map from barrier to last waited cycle
    bars: [u32; 6],
    // Current cycle count until end-of-block.
    current_cycle: u32,
    // Map from idx (block, instr) to block-relative cycle
    instr_cycles: &'a mut Vec<Vec<u32>>,
}

impl BlockDelayScheduler<'_> {
    /// Compute the starting cycle for an instruction to avoid a dependency hazard.
    fn dependency_to_cycle(
        &self,
        curr_loc: InstrIdx, // Location of the current instruction
        reg: &RegOrigin, // Register and location of instruction that will be executed later
        delay: AccumulatedDelay, // Delay between the end of the current block and the later instruction
        latency: u32, // Latency between current and later instruction
    ) -> u32 {
        debug_assert!(latency <= self.sm.latency_upper_bound());

        let same_block = reg.loc.block_idx == curr_loc.block_idx
            && reg.loc.instr_idx > curr_loc.instr_idx;

        if same_block {
            // Created this transfer pass
            self.instr_cycles[reg.loc.block_idx as usize]
                [reg.loc.instr_idx as usize]
                + latency
        } else {
            // Remember that cycles are always counted from the end of a block.
            // The next instruction happens after `delay` cycles after the
            // current block is complete, so it is effectively executed at cycle
            // `0 - delay`, adding the latency we get `latency - delay`
            // Underflow means that the instruction is already done (delay > latency).
            latency.saturating_sub(delay.into())
        }
    }

    fn process_instr(&mut self, loc: InstrIdx, reg_uses: &mut DelayRegTracker) {
        let instr = &self.f[loc];

        let mut min_start =
            self.current_cycle + self.sm.exec_latency(&instr.op);

        // Wait on rd/wr barriers
        if let Some(bar) = instr.deps.rd_bar() {
            min_start = max(min_start, self.bars[usize::from(bar)] + 2);
        }
        if let Some(bar) = instr.deps.wr_bar() {
            min_start = max(min_start, self.bars[usize::from(bar)] + 2);
        }

        reg_uses.for_each_instr_dst_mut(instr, |i, u| {
            for (orig, delay) in u.iter_writes() {
                let l = self.sm.waw_latency(
                    &instr.op,
                    i,
                    !instr.pred.pred_ref.is_none(),
                    &self.f[orig.loc].op,
                    orig.src_dst_idx as usize,
                );
                let s = self.dependency_to_cycle(loc, orig, *delay, l);
                min_start = max(min_start, s);
            }
            for (orig, delay) in u.iter_reads() {
                let l = if orig.src_dst_idx == u16::MAX {
                    self.sm.paw_latency(&instr.op, i)
                } else {
                    self.sm.raw_latency(
                        &instr.op,
                        i,
                        &self.f[orig.loc].op,
                        orig.src_dst_idx as usize,
                    )
                };
                let s = self.dependency_to_cycle(loc, orig, *delay, l);
                min_start = max(min_start, s);
            }

            u.set_write(
                RegOrigin {
                    loc,
                    src_dst_idx: i as u16,
                },
                0,
            );
        });

        reg_uses.for_each_instr_pred_mut(instr, |c| {
            // WaP does not exist
            c.add_read(
                RegOrigin {
                    loc,
                    src_dst_idx: u16::MAX,
                },
                0,
            );
        });
        reg_uses.for_each_instr_src_mut(instr, |i, u| {
            for (orig, delay) in u.iter_writes() {
                let l = self.sm.war_latency(
                    &instr.op,
                    i,
                    &self.f[orig.loc].op,
                    orig.src_dst_idx as usize,
                );
                let s = self.dependency_to_cycle(loc, orig, *delay, l);
                min_start = max(min_start, s);
            }

            u.add_read(
                RegOrigin {
                    loc,
                    src_dst_idx: i as u16,
                },
                0,
            );
        });

        self.instr_cycles[loc.block_idx as usize][loc.instr_idx as usize] =
            min_start;

        // Kepler A membar conflicts with predicate writes
        if self.sm.is_kepler_a() && matches!(&instr.op, Op::MemBar(_)) {
            let read_origin = RegOrigin {
                loc,
                src_dst_idx: u16::MAX,
            };
            reg_uses.for_each_pred(|c| {
                c.add_read(read_origin, 0);
            });
            reg_uses.for_each_carry(|c| {
                c.add_read(read_origin, 0);
            });
        }

        // "Issue" barriers other instructions will wait on.
        for (bar, c) in self.bars.iter_mut().enumerate() {
            if instr.deps.wt_bar_mask & (1 << bar) != 0 {
                *c = min_start;
            }
        }

        self.current_cycle = min_start;
    }
}

fn calc_delays(f: &mut Function, sm: &ShaderModelInfo) -> u64 {
    let mut instr_cycles: Vec<Vec<u32>> =
        f.blocks.iter().map(|b| vec![0; b.instrs.len()]).collect();

    let mut state_in: Vec<_> = vec![DelayRegTracker::default(); f.blocks.len()];
    let mut state_out: Vec<_> =
        vec![DelayRegTracker::default(); f.blocks.len()];

    let latency_upper_bound: u8 = sm
        .latency_upper_bound()
        .try_into()
        .expect("Latency upper bound too large!");

    // Compute instruction delays using an optimistic backwards data-flow
    // algorithm.  For back-cycles we assume the best and recompute when
    // new data is available.  This is yields correct results as long as
    // the data flow analysis is run until completion.
    BackwardDataflow {
        cfg: &f.blocks,
        block_in: &mut state_in[..],
        block_out: &mut state_out[..],
        transfer: |block_idx, block, reg_in, reg_out| {
            let mut uses = reg_out.clone();

            let mut sched = BlockDelayScheduler {
                sm,
                f,
                // Barriers are handled by `assign_barriers`, and it does
                // not handle cross-block barrier signal/wait.
                // We can safely assume that no barrier is active at the
                // start and end of the block
                bars: [0_u32; 6],
                current_cycle: 0_u32,
                instr_cycles: &mut instr_cycles,
            };

            for ip in (0..block.instrs.len()).rev() {
                let loc = InstrIdx::new(block_idx, ip);
                sched.process_instr(loc, &mut uses);
            }

            // Update accumulated delay
            let block_cycles = sched.current_cycle;
            uses.retain(|reg_use| {
                reg_use.map.retain(|(_rw, k), v| {
                    let overcount = if k.loc.block_idx as usize == block_idx {
                        // Only instrs before instr_idx must be counted
                        instr_cycles[k.loc.block_idx as usize]
                            [k.loc.instr_idx as usize]
                    } else {
                        0
                    };
                    let instr_executed = (block_cycles - overcount)
                        .try_into()
                        .unwrap_or(u8::MAX);
                    // We only care about the accumulated delay until it
                    // is bigger than the maximum delay of an instruction.
                    // after that, it cannot cause hazards.
                    let (added, overflow) =
                        (*v).overflowing_add(instr_executed);
                    *v = added;
                    // Stop keeping track of entries that happened too
                    // many cycles "in the future", and cannot affect
                    // scheduling anymore
                    !overflow && added <= latency_upper_bound
                });
                !reg_use.map.is_empty()
            });

            if *reg_in == uses {
                false
            } else {
                *reg_in = uses;
                true
            }
        },
        join: |curr_in, succ_out| {
            // We start with an optimistic assumption and gradually make it
            // less optimistic.  So in the join operation we need to keep
            // the "worst" accumulated latency, that is the lowest one.
            // i.e. if an instruction has an accumulated latency of 2 cycles,
            // it can interfere with the next block, while if it had 200 cycles
            // it's highly unlikely that it could interfere.
            curr_in.merge_with(succ_out, |a, b| {
                a.merge_with(b, |ai, bi| (*ai).min(*bi))
            });
        },
    }
    .solve();

    // Update the deps.delay for each instruction and compute
    for (bi, b) in f.blocks.iter_mut().enumerate() {
        let cycles = &instr_cycles[bi];
        for (ip, i) in b.instrs.iter_mut().enumerate() {
            let delay = cycles[ip] - cycles.get(ip + 1).copied().unwrap_or(0);
            let delay: u8 = delay.try_into().expect("Delay overflow");
            i.deps.delay = delay.max(MIN_INSTR_DELAY);
        }
    }

    let min_num_static_cycles = instr_cycles
        .iter()
        .enumerate()
        .map(|(block_idx, cycles)| {
            let cycles = cycles.last().copied().unwrap_or(0);
            let block_weight = estimate_block_weight(&f.blocks, block_idx);
            u64::from(cycles)
                .checked_mul(block_weight)
                .expect("Cycle count estimate overflow")
        })
        .reduce(|a, b| a.checked_add(b).expect("Cycle count estimate overflow"))
        .unwrap_or(0);

    let max_instr_delay = sm.max_instr_delay();
    f.map_instrs(|mut instr, _| {
        if instr.deps.delay > max_instr_delay {
            let mut delay = instr.deps.delay - max_instr_delay;
            instr.deps.set_delay(max_instr_delay);
            let mut instrs = MappedInstrs::from([instr]);
            while delay > 0 {
                let mut nop = Instr::new(OpNop { label: None });
                nop.deps.set_delay(delay.min(max_instr_delay));
                delay -= nop.deps.delay;
                instrs.push(nop);
            }
            instrs
        } else if matches!(instr.op, Op::SrcBar(_)) {
            instr.op = Op::Nop(OpNop { label: None });
            [instr].into()
        } else if sm.exec_latency(&instr.op) > 1 {
            // It's unclear exactly why but the blob inserts a Nop with a delay
            // of 2 after every instruction which has an exec latency.  Perhaps
            // it has something to do with .yld?  In any case, the extra 2
            // cycles aren't worth the chance of weird bugs.
            let mut nop = Instr::new(OpNop { label: None });
            nop.deps.set_delay(2);
            [instr, nop].into()
        } else {
            [instr].into()
        }
    });

    min_num_static_cycles
}

impl Shader<'_> {
    pub fn assign_deps_serial(&mut self) {
        for f in &mut self.functions {
            for b in &mut f.blocks.iter_mut().rev() {
                let mut wt = 0_u8;
                for instr in &mut b.instrs {
                    if matches!(&instr.op, Op::Bar(_))
                        || matches!(&instr.op, Op::BClear(_))
                        || matches!(&instr.op, Op::BSSy(_))
                        || matches!(&instr.op, Op::BSync(_))
                    {
                        instr.deps.set_yield(true);
                    } else if instr.is_branch() {
                        instr.deps.add_wt_bar_mask(0x3f);
                    } else {
                        instr.deps.add_wt_bar_mask(wt);
                        if instr.dsts().len() > 0 {
                            instr.deps.set_wr_bar(0);
                            wt |= 1 << 0;
                        }
                        if !instr.pred.pred_ref.is_none()
                            || instr.srcs().len() > 0
                        {
                            instr.deps.set_rd_bar(1);
                            wt |= 1 << 1;
                        }
                    }
                }
            }
        }
    }

    pub fn calc_instr_deps(&mut self) {
        if self.sm.is_kepler() {
            for f in &mut self.functions {
                insert_texture_barriers(f, self.sm);
            }
        }

        if DEBUG.serial() {
            self.assign_deps_serial();
        } else {
            let xbar =
                DEBUG.xbar() && (self.sm.is_maxwell() || self.sm.is_pascal());
            let mut min_num_static_cycles = 0u64;
            for f in &mut self.functions {
                if xbar {
                    if let Err(e) = assign_barriers_xbar(
                        f,
                        self.sm,
                        DEBUG.xbrk(),
                        DEBUG.share(),
                    ) {
                        eprintln!("NAK xbar: kept legacy barriers: {e}");
                    }
                } else {
                    assign_barriers(f, self.sm);
                }
                min_num_static_cycles += calc_delays(f, self.sm);
            }

            if DEBUG.cycles() {
                // This is useful for debugging differences in the scheduler
                // cycle count model and the calc_delays() model.  However, it
                // isn't totally valid since assign_barriers() can add extra
                // dependencies for barrier re-use and those may add cycles.
                // The chances of it doing this are low, thanks to our LRU
                // allocation strategy, but it's still not an assert we want
                // running in production.
                assert!(self.info.num_static_cycles >= min_num_static_cycles);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg_gpr(range: Range<usize>) -> RegRef {
        RegRef::new(
            RegFile::GPR,
            range.start as u32,
            (range.end - range.start) as u8,
        )
    }

    #[test]
    fn test_texdepbar_basic() {
        let mut sim = TexQueueSimulationState::new();

        // RaW
        assert_eq!(sim.push(reg_gpr(0..4)), None);
        assert_eq!(sim.flush(reg_gpr(2..3)), Some(0));

        // 2 entries in the queue
        assert_eq!(sim.push(reg_gpr(0..2)), None); // [A]
        assert_eq!(sim.push(reg_gpr(2..4)), None); // [B, A]
        assert_eq!(sim.flush(reg_gpr(0..1)), Some(1)); // [B]
        assert_eq!(sim.flush(reg_gpr(3..4)), Some(0)); // []

        // Test bucket conflicts
        assert_eq!(sim.push(reg_gpr(0..1)), None);
        assert_eq!(sim.flush(reg_gpr(1..3)), None);
        assert_eq!(sim.flush(reg_gpr(0..3)), Some(0));

        // Bucket conflict part 2: Electric Boogaloo
        assert_eq!(sim.push(reg_gpr(1..2)), None);
        assert_eq!(sim.push(reg_gpr(0..1)), None);
        assert_eq!(sim.flush(reg_gpr(1..2)), Some(1));
        assert_eq!(sim.flush(reg_gpr(0..1)), Some(0));

        // Interesting CFG case that the old pass got wrong.
        // CFG: A -> [B, C] -> D
        // A pushes
        assert_eq!(sim.push(reg_gpr(0..4)), None);
        // B: pushes a tex then flushes it
        let mut b_sim = sim.clone();
        assert_eq!(b_sim.push(reg_gpr(4..8)), None);
        assert_eq!(b_sim.flush(reg_gpr(4..8)), Some(0));
        // C: pushes 3 tex and never flishes them
        let mut c_sim = sim.clone();
        assert_eq!(c_sim.push(reg_gpr(4..5)), None);
        assert_eq!(c_sim.push(reg_gpr(5..6)), None);
        assert_eq!(c_sim.push(reg_gpr(6..7)), None);
        // D: flushes the tex pushed by A
        let mut d_sim = b_sim;
        d_sim.merge(&c_sim);
        assert_eq!(c_sim.flush(reg_gpr(0..4)), Some(3));
        // the "shortest push path" would pass by B but in fact
        // by passing in B our texture is flushed off the queue.
        // (old algorithm would insert a texdepbar 1)
    }

    #[test]
    fn test_texdepbar_overflow() {
        let mut sim = TexQueueSimulationState::new();

        // Fill the texture queue
        for i in 0..(usize::from(OpTexDepBar::MAX_TEXTURES_LEFT) + 1) {
            assert_eq!(sim.push(reg_gpr(i..(i + 1))), None);
        }
        // The new push would overflow the queue, we NEED a barrier
        assert_eq!(
            sim.push(reg_gpr(64..65)),
            Some(OpTexDepBar::MAX_TEXTURES_LEFT)
        );
        assert_eq!(
            sim.push(reg_gpr(65..66)),
            Some(OpTexDepBar::MAX_TEXTURES_LEFT)
        );
    }

    use compiler::cfg::CFGBuilder;
    use rustc_hash::FxBuildHasher;

    fn xbar_mufu(dst: usize, src: usize) -> Instr {
        Instr::new(OpMuFu {
            dst: reg_gpr(dst..dst + 1).into(),
            op: MuFuOp::Rcp,
            src: reg_gpr(src..src + 1).into(),
            op_type: FloatType::F32,
        })
    }

    fn xbar_mov(dst: usize, src: usize) -> Instr {
        Instr::new(OpMov {
            dst: reg_gpr(dst..dst + 1).into(),
            src: reg_gpr(src..src + 1).into(),
            quad_lanes: 0xf,
        })
    }

    fn xbar_bra(target: Label) -> Instr {
        Instr::new(OpBra {
            target,
            cond: RegRef::new(RegFile::Pred, 0, 1).into(),
        })
    }

    /// Builds a function from each block's instructions and successors, in
    /// that block order.  `build` gets the block labels for branch targets.
    fn xbar_function(
        num_blocks: usize,
        build: impl FnOnce(&[Label]) -> Vec<(Vec<Instr>, Vec<usize>)>,
    ) -> Function {
        let mut label_alloc = LabelAllocator::new();
        let labels: Vec<Label> =
            (0..num_blocks).map(|_| label_alloc.alloc()).collect();
        let blocks = build(&labels);
        assert_eq!(blocks.len(), num_blocks);
        let mut cfg = CFGBuilder::<_, _, FxBuildHasher>::new();
        let mut edges = Vec::new();
        for (bi, (instrs, succs)) in blocks.into_iter().enumerate() {
            let label = labels[bi];
            let uniform = false;
            cfg.add_node(bi, BasicBlock { label, uniform, instrs });
            edges.extend(succs.into_iter().map(|s| (bi, s)));
        }
        for (p, s) in edges {
            cfg.add_edge(p, s);
        }
        Function {
            ssa_alloc: SSAValueAllocator::new(),
            phi_alloc: PhiAllocator::new(),
            blocks: cfg.as_cfg(false),
        }
    }

    fn xbar_sm() -> ShaderModelInfo {
        ShaderModelInfo::new(53, 64)
    }

    fn xbar_mask(f: &Function, bi: usize, ip: usize) -> u8 {
        f.blocks[bi].instrs[ip].deps.wt_bar_mask
    }

    fn xbar_bit(f: &Function, bi: usize, ip: usize, write: bool) -> u8 {
        let deps = &f.blocks[bi].instrs[ip].deps;
        let bar = if write { deps.wr_bar() } else { deps.rd_bar() };
        1 << bar.unwrap()
    }

    // if (p0) { r1 = rcp(r0) } else { r1 = rcp(r2) }; r0 = r4; r3 = r1
    fn xbar_if_else() -> Function {
        xbar_function(4, |l| {
            vec![
                (vec![Instr::new(OpSSy { target: l[3] }), xbar_bra(l[2])], {
                    vec![1, 2]
                }),
                (
                    vec![xbar_mufu(1, 0), Instr::new(OpSync { target: l[3] })],
                    vec![3],
                ),
                (
                    vec![xbar_mufu(1, 2), Instr::new(OpSync { target: l[3] })],
                    vec![3],
                ),
                (
                    vec![xbar_mov(0, 4), xbar_mov(3, 1), Instr::new(OpExit {})],
                    vec![],
                ),
            ]
        })
    }

    #[test]
    fn test_xbar_if_else() {
        let sm = xbar_sm();

        let mut legacy = xbar_if_else();
        assign_barriers(&mut legacy, &sm);
        assert_ne!(xbar_mask(&legacy, 1, 1), 0);
        assert_ne!(xbar_mask(&legacy, 2, 1), 0);
        xbar_verify(&legacy, &sm, &xbar_plans_from_deps(&legacy)).unwrap();

        let mut f = xbar_if_else();
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        // Neither SYNC waits
        assert_eq!(xbar_mask(&f, 1, 1), 0);
        assert_eq!(xbar_mask(&f, 2, 1), 0);
        // r0 = r4 waits for the then-side rcp to read r0
        assert_eq!(xbar_mask(&f, 3, 0), xbar_bit(&f, 1, 0, false));
        // By r3 = r1, both rcps have written r1
        let waited = xbar_mask(&f, 3, 0) | xbar_mask(&f, 3, 1);
        assert_ne!(waited & xbar_bit(&f, 1, 0, true), 0);
        assert_ne!(waited & xbar_bit(&f, 2, 0, true), 0);
        assert!(f.blocks[2].instrs[0].deps.rd_bar().is_none());
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }

    #[test]
    fn test_xbar_verify_rejects_missing_waits() {
        let sm = xbar_sm();
        let mut f = xbar_if_else();
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        let plans = xbar_plans_from_deps(&f);

        let mut no_raw = plans.clone();
        no_raw[3][1].wt_bar_mask = 0;
        let err = xbar_verify(&f, &sm, &no_raw).unwrap_err();
        assert!(err.starts_with("read after write"), "{err}");

        let mut no_war = plans;
        no_war[3][0].wt_bar_mask = 0;
        let err = xbar_verify(&f, &sm, &no_war).unwrap_err();
        assert!(err.starts_with("write after read"), "{err}");

        // Dropping assign_barriers()'s wait at a SYNC, without waiting after
        // the join instead, is unsafe.
        let mut legacy = xbar_if_else();
        assign_barriers(&mut legacy, &sm);
        let mut no_sync = xbar_plans_from_deps(&legacy);
        no_sync[1][1].wt_bar_mask = 0;
        assert!(xbar_verify(&legacy, &sm, &no_sync).is_err());
    }

    #[test]
    fn test_xbar_join_keeps_unrelated_bars() {
        // if (p0) { r1 = rcp(r0) }; r4 = rcp(r5); r6 = r4; r7 = r1
        let sm = xbar_sm();
        let mut f = xbar_function(4, |l| {
            vec![
                (vec![Instr::new(OpSSy { target: l[3] }), xbar_bra(l[2])], {
                    vec![1, 2]
                }),
                (
                    vec![xbar_mufu(1, 0), Instr::new(OpSync { target: l[3] })],
                    vec![3],
                ),
                (vec![Instr::new(OpSync { target: l[3] })], vec![3]),
                (
                    vec![
                        xbar_mufu(4, 5),
                        xbar_mov(6, 4),
                        xbar_mov(7, 1),
                        Instr::new(OpExit {}),
                    ],
                    vec![],
                ),
            ]
        });
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        assert_eq!(xbar_mask(&f, 1, 1), 0);
        // The rcp after the join can't reuse the scoreboard that the
        // then-side rcp may still hold, so r6 = r4 doesn't wait for it.
        assert_ne!(xbar_bit(&f, 3, 0, true), xbar_bit(&f, 1, 0, true));
        assert_eq!(xbar_mask(&f, 3, 1), xbar_bit(&f, 3, 0, true));
        assert_eq!(xbar_mask(&f, 3, 2), xbar_bit(&f, 1, 0, true));
    }

    #[test]
    fn test_xbar_back_edge_waits() {
        // r1 = rcp(r0); do { r2 = r1; r1 = rcp(r2) } while (p0); r3 = r1
        let sm = xbar_sm();
        let mut f = xbar_function(3, |l| {
            vec![
                (vec![xbar_mufu(1, 0)], vec![1]),
                (
                    vec![xbar_mov(2, 1), xbar_mufu(1, 2), xbar_bra(l[1])],
                    vec![2, 1],
                ),
                (vec![xbar_mov(3, 1), Instr::new(OpExit {})], vec![]),
            ]
        });
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        assert_eq!(xbar_mask(&f, 1, 0), xbar_bit(&f, 0, 0, true));
        assert_eq!(xbar_mask(&f, 1, 2), xbar_bit(&f, 1, 1, true));
    }

    #[test]
    fn test_xbar_matches_legacy_in_one_block() {
        // Eight rcps in flight, more than there are scoreboards
        let build = || {
            xbar_function(1, |_| {
                let mut instrs: Vec<Instr> =
                    (10..18).map(|r| xbar_mufu(r, 0)).collect();
                instrs.extend((10..18).map(|r| xbar_mov(r + 10, r)));
                instrs.push(Instr::new(OpExit {}));
                vec![(instrs, vec![])]
            })
        };
        let sm = xbar_sm();
        let mut legacy = build();
        assign_barriers(&mut legacy, &sm);
        let mut f = build();
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        for (a, b) in legacy.blocks[0].instrs.iter().zip(&f.blocks[0].instrs) {
            assert_eq!(a.deps.wt_bar_mask, b.deps.wt_bar_mask, "{b}");
            assert_eq!(a.deps.rd_bar(), b.deps.rd_bar(), "{b}");
            assert_eq!(a.deps.wr_bar(), b.deps.wr_bar(), "{b}");
        }
    }

    // The shape of NVK's loop over the descriptors of a non-uniform lookup,
    // laid out as NAK lays it out, with the loop's exit before its CONT:
    // loop { r0 = r6; if (p0) { r1 = rcp(r0); break } }; r3 = r1
    fn xbar_break_loop() -> Function {
        xbar_function(6, |l| {
            vec![
                (vec![Instr::new(OpPBk { target: l[3] })], vec![1]),
                (
                    vec![
                        Instr::new(OpPCnt { target: l[1] }),
                        xbar_mov(0, 6),
                        Instr::new(OpSSy { target: l[5] }),
                        xbar_bra(l[4]),
                    ],
                    vec![2, 4],
                ),
                (
                    vec![xbar_mufu(1, 0), Instr::new(OpBrk { target: l[3] })],
                    vec![3],
                ),
                (vec![xbar_mov(3, 1), Instr::new(OpExit {})], vec![]),
                (vec![Instr::new(OpSync { target: l[5] })], vec![5]),
                (vec![Instr::new(OpCont { target: l[1] })], vec![1]),
            ]
        })
    }

    #[test]
    fn test_xbar_brk() {
        let sm = xbar_sm();

        let mut f = xbar_break_loop();
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        assert_ne!(xbar_mask(&f, 2, 1) & xbar_bit(&f, 2, 0, true), 0);
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();

        let mut f = xbar_break_loop();
        assign_barriers_xbar(&mut f, &sm, true, false).unwrap();
        // The BRK doesn't wait for the rcp; r3 = r1 after the loop does.
        assert_eq!(xbar_mask(&f, 2, 1), 0);
        assert_eq!(xbar_mask(&f, 3, 0), xbar_bit(&f, 2, 0, true));
        // Lanes still in the loop write r0 for themselves while the rcp of
        // lanes that have left may still be reading theirs.
        assert_eq!(xbar_mask(&f, 1, 1), 0);
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }

    #[test]
    fn test_xbar_brk_backward_waits() {
        // A loop with two breaks and its exit laid out between them, so the
        // second BRK goes back and must still wait on everything:
        // loop { if (p0) { r1 = rcp(r0); break } r1 = rcp(r2); break }; r3 = r1
        let sm = xbar_sm();
        let mut f = xbar_function(5, |l| {
            vec![
                (vec![Instr::new(OpPBk { target: l[3] })], vec![1]),
                (
                    vec![Instr::new(OpPCnt { target: l[1] }), xbar_bra(l[4])],
                    vec![2, 4],
                ),
                (
                    vec![xbar_mufu(1, 0), Instr::new(OpBrk { target: l[3] })],
                    vec![3],
                ),
                (vec![xbar_mov(3, 1), Instr::new(OpExit {})], vec![]),
                (
                    vec![xbar_mufu(1, 2), Instr::new(OpBrk { target: l[3] })],
                    vec![3],
                ),
            ]
        });
        assign_barriers_xbar(&mut f, &sm, true, false).unwrap();
        assert_eq!(xbar_mask(&f, 2, 1), 0);
        assert_eq!(xbar_mask(&f, 3, 0), xbar_bit(&f, 2, 0, true));
        assert_ne!(xbar_mask(&f, 4, 1) & xbar_bit(&f, 4, 0, true), 0);
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }

    fn xbar_ld(dst: usize, addr: usize, space: MemSpace) -> Instr {
        Instr::new(OpLd {
            dst: reg_gpr(dst..dst + 1).into(),
            addr: reg_gpr(addr..addr + 1).into(),
            uniform_addr: Src::ZERO,
            pred: true.into(),
            offset: 0,
            stride: OffsetStride::X1,
            access: MemAccess {
                mem_type: MemType::B32,
                space,
                order: MemOrder::Weak,
                eviction_priority: MemEvictionPriority::Normal,
            },
        })
    }

    const XBAR_GLOBAL: MemSpace = MemSpace::Global(MemAddrType::A64);

    // n loads from [r0] into r10.., then a use of each, in order
    fn xbar_load_run(n: usize) -> Function {
        xbar_function(1, |_| {
            let mut instrs: Vec<Instr> =
                (0..n).map(|i| xbar_ld(10 + i, 0, XBAR_GLOBAL)).collect();
            instrs.extend((0..n).map(|i| xbar_mov(100 + i, 10 + i)));
            instrs.push(Instr::new(OpExit {}));
            vec![(instrs, vec![])]
        })
    }

    #[test]
    fn test_xbar_share_load_run() {
        let sm = xbar_sm();

        // Without share, the seventh load waits for the first
        let mut f = xbar_load_run(8);
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        assert_eq!(xbar_mask(&f, 0, 6), xbar_bit(&f, 0, 0, true));

        let mut f = xbar_load_run(8);
        assign_barriers_xbar(&mut f, &sm, false, true).unwrap();
        for ip in 0..8 {
            assert_eq!(xbar_mask(&f, 0, ip), 0, "{}", f.blocks[0].instrs[ip]);
        }
        // The seventh and eighth share the sixth's scoreboard, the one
        // first waited on latest, and the use of the sixth's result covers
        // all three.
        assert_eq!(xbar_bit(&f, 0, 6, true), xbar_bit(&f, 0, 5, true));
        assert_eq!(xbar_bit(&f, 0, 7, true), xbar_bit(&f, 0, 5, true));
        assert_eq!(xbar_mask(&f, 0, 13), xbar_bit(&f, 0, 5, true));
        assert_eq!(xbar_mask(&f, 0, 14), 0);
        assert_eq!(xbar_mask(&f, 0, 15), 0);
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }

    #[test]
    fn test_xbar_share_max() {
        // Six scoreboards hold 48 loads at most; the 49th waits
        let sm = xbar_sm();
        let n = 6 * XBAR_SHARE_MAX + 1;
        let mut f = xbar_load_run(n);
        assign_barriers_xbar(&mut f, &sm, false, true).unwrap();
        for ip in 0..n - 1 {
            assert_eq!(xbar_mask(&f, 0, ip), 0, "{}", f.blocks[0].instrs[ip]);
        }
        assert_ne!(xbar_mask(&f, 0, n - 1), 0);
        for b in 0..6 {
            let held = (0..n - 1)
                .filter(|&ip| f.blocks[0].instrs[ip].deps.wr_bar() == Some(b))
                .count();
            assert_eq!(held, XBAR_SHARE_MAX);
        }
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }

    #[test]
    fn test_xbar_share_reads_and_writes_apart() {
        // r10..r13 = ld [r0..r3]; r0..r3 = r9; r20..r23 = r10..r13
        let sm = xbar_sm();
        let build = || {
            xbar_function(1, |_| {
                let mut instrs: Vec<Instr> =
                    (0..4).map(|i| xbar_ld(10 + i, i, XBAR_GLOBAL)).collect();
                instrs.extend((0..4).map(|i| xbar_mov(i, 9)));
                instrs.extend((0..4).map(|i| xbar_mov(20 + i, 10 + i)));
                instrs.push(Instr::new(OpExit {}));
                vec![(instrs, vec![])]
            })
        };

        let mut f = build();
        assign_barriers_xbar(&mut f, &sm, false, false).unwrap();
        assert_ne!(xbar_mask(&f, 0, 3), 0);

        let mut f = build();
        assign_barriers_xbar(&mut f, &sm, false, true).unwrap();
        assert_eq!(xbar_mask(&f, 0, 3), 0);
        assert_eq!(xbar_bit(&f, 0, 3, false), xbar_bit(&f, 0, 2, false));
        assert_eq!(xbar_bit(&f, 0, 3, true), xbar_bit(&f, 0, 2, true));
        // Overwriting r2 waits for both loads to read their addresses, and
        // using r12 for both to write; r3 and r13 then need no wait.
        assert_eq!(xbar_mask(&f, 0, 6), xbar_bit(&f, 0, 2, false));
        assert_eq!(xbar_mask(&f, 0, 7), 0);
        assert_eq!(xbar_mask(&f, 0, 10), xbar_bit(&f, 0, 2, true));
        assert_eq!(xbar_mask(&f, 0, 11), 0);
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }

    #[test]
    fn test_xbar_share_keeps_kinds_apart() {
        // Six global loads in flight, then a shared-memory load and an rcp:
        // neither may share the global loads' scoreboards.
        let sm = xbar_sm();
        let mut f = xbar_function(1, |_| {
            let mut instrs: Vec<Instr> =
                (0..6).map(|i| xbar_ld(10 + i, 0, XBAR_GLOBAL)).collect();
            instrs.push(xbar_ld(16, 0, MemSpace::Shared));
            instrs.push(xbar_mufu(17, 0));
            instrs.extend((0..8).map(|i| xbar_mov(20 + i, 10 + i)));
            instrs.push(Instr::new(OpExit {}));
            vec![(instrs, vec![])]
        });
        assign_barriers_xbar(&mut f, &sm, false, true).unwrap();
        assert_eq!(xbar_mask(&f, 0, 6), xbar_bit(&f, 0, 0, true));
        assert_eq!(xbar_mask(&f, 0, 7), xbar_bit(&f, 0, 1, true));
        xbar_verify(&f, &sm, &xbar_plans_from_deps(&f)).unwrap();
    }
}
