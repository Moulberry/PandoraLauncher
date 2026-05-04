mod decoder;
mod encoder;
mod range_coding;

use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    io::{Read, Write},
    mem::{swap, ManuallyDrop},
    ptr::NonNull,
};

pub(crate) use range_coding::{RangeDecoder, RangeEncoder};

use super::*;
use crate::{Error, RestoreMethod, PPMD8_MAX_ORDER};

const MAX_FREQ: u8 = 124;
const UNIT_SIZE: isize = 12;
const K_TOP_VALUE: u32 = 1 << 24;
const K_BOT_VALUE: u32 = 1 << 15;
const EMPTY_NODE: u32 = u32::MAX;
const FLAG_RESCALED: u8 = 1 << 2;
const FLAG_PREV_HIGH: u8 = 1 << 4;

static K_EXP_ESCAPE: [u8; 16] = [25, 14, 9, 7, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 2];

static K_INIT_BIN_ESC: [u16; 8] = [
    0x3CDD, 0x1F3F, 0x59BF, 0x48F3, 0x64A1, 0x5ABC, 0x6632, 0x6051,
];

#[derive(Copy, Clone)]
#[repr(C)]
struct Node {
    stamp: u32,
    next: TaggedOffset,
    nu: u32,
}

impl Pointee for Node {
    const TAG: u32 = TAG_NODE;
}

#[derive(Copy, Clone)]
#[repr(C)]
struct Context {
    num_stats: u8,
    flags: u8,
    union2: Union2,
    union4: Union4,
    suffix: TaggedOffset,
}

impl Pointee for Context {
    const TAG: u32 = TAG_CONTEXT;
}

pub(crate) struct PPMd8<RC> {
    min_context: NonNull<Context>,
    max_context: NonNull<Context>,
    found_state: NonNull<State>,
    order_fall: u32,
    init_esc: u32,
    prev_success: u32,
    max_order: u32,
    restore_method: RestoreMethod,
    run_length: i32,
    init_rl: i32,
    size: u32,
    glue_count: u32,
    align_offset: u32,
    lo_unit: NonNull<u8>,
    hi_unit: NonNull<u8>,
    text: NonNull<u8>,
    units_start: NonNull<u8>,
    index2units: [u8; 40],
    units2index: [u8; 128],
    free_list: [TaggedOffset; 38],
    stamps: [u32; 38],
    ns2bs_index: [u8; 256],
    ns2index: [u8; 260],
    exp_escape: [u8; 16],
    dummy_see: See,
    see: [[See; 32]; 24],
    bin_summ: [[u16; 64]; 25],
    memory_ptr: NonNull<u8>,
    memory_layout: Layout,
    rc: RC,
}

impl<RC> Drop for PPMd8<RC> {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.memory_ptr.as_ptr(), self.memory_layout);
        }
    }
}

impl<RC> MemoryAllocator for PPMd8<RC> {
    fn base_memory_ptr(&self) -> NonNull<u8> {
        self.memory_ptr
    }

    #[cfg(not(feature = "unstable-tagged-offsets"))]
    fn units_start(&self) -> NonNull<u8> {
        self.units_start
    }

    #[cfg(feature = "unstable-tagged-offsets")]
    fn size(&self) -> u32 {
        self.size
    }
}

impl<RC> PPMd8<RC> {
    fn construct(
        rc: RC,
        max_order: u32,
        mem_size: u32,
        restore_method: RestoreMethod,
    ) -> crate::Result<Self> {
        let mut units2index = [0u8; 128];
        let mut index2units = [0u8; 40];

        let mut k = 0;
        for i in 0..PPMD_NUM_INDEXES {
            let mut step = if i >= 12 { 4 } else { (i >> 2) + 1 };
            loop {
                units2index[k as usize] = i as u8;
                k += 1;

                step -= 1;
                if step == 0 {
                    break;
                }
            }
            index2units[i as usize] = k as u8;
        }

        let mut ns2bs_index = [0u8; 256];
        ns2bs_index[0] = (0 << 1) as u8;
        ns2bs_index[1] = (1 << 1) as u8;
        ns2bs_index[2..11].fill((2 << 1) as u8);
        ns2bs_index[11..256].fill((3 << 1) as u8);

        let mut ns2index = [0u8; 260];
        for i in 0..5 {
            ns2index[i as usize] = i as u8;
        }

        let mut m = 5;
        let mut k = 1;
        for i in 5..260 {
            ns2index[i as usize] = m as u8;
            k -= 1;
            if k == 0 {
                m += 1;
                k = m - 4;
            }
        }

        let align_offset = (4u32.wrapping_sub(mem_size)) & 3;
        let total_size = (align_offset + mem_size) as usize;

        let memory_layout = Layout::from_size_align(total_size, align_of::<usize>())
            .expect("Failed to create memory layout");

        let allocation = unsafe {
            assert_ne!(total_size, 0);
            NonNull::new(alloc_zeroed(memory_layout))
        };

        let Some(memory_ptr) = allocation else {
            return Err(Error::MemoryAllocation);
        };

        let mut ppmd = Self {
            min_context: NonNull::dangling(),
            max_context: NonNull::dangling(),
            found_state: NonNull::dangling(),
            order_fall: 0,
            init_esc: 0,
            prev_success: 0,
            max_order,
            restore_method,
            run_length: 0,
            init_rl: 0,
            size: mem_size,
            glue_count: 0,
            align_offset,
            lo_unit: NonNull::dangling(),
            hi_unit: NonNull::dangling(),
            text: NonNull::dangling(),
            units_start: NonNull::dangling(),
            index2units,
            units2index,
            free_list: [TaggedOffset::null(); 38],
            stamps: [0; 38],
            ns2bs_index,
            ns2index,
            exp_escape: K_EXP_ESCAPE,
            dummy_see: See::default(),
            see: [[See::default(); 32]; 24],
            bin_summ: [[0; 64]; 25],
            memory_ptr,
            memory_layout,
            rc,
        };

        unsafe { ppmd.restart_model() };

        Ok(ppmd)
    }

    unsafe fn offset_for_ptr<T: Pointee>(&self, ptr: NonNull<T>) -> TaggedOffset {
        unsafe { TaggedOffset::from_ptr(self, ptr) }
    }

    unsafe fn insert_node(&mut self, mut node: NonNull<Node>, index: u32) {
        unsafe {
            node.as_mut().stamp = 0xFFFFFFFF;
            node.as_mut().next = self.free_list[index as usize];
            node.as_mut().nu = self.index2units[index as usize] as u32;
            self.free_list[index as usize] = self.offset_for_ptr(node);
            self.stamps[index as usize] = self.stamps[index as usize].wrapping_add(1);
        }
    }

    unsafe fn remove_node(&mut self, index: u32) -> NonNull<Node> {
        let index = index as usize;
        let node_offset = self.free_list[index];
        let node = node_offset.as_ptr::<Node, _>(self);
        self.free_list[index] = node.as_ref().next;
        self.stamps[index] = self.stamps[index].wrapping_sub(1);
        node
    }

    unsafe fn split_block(&mut self, mut ptr: NonNull<u8>, old_index: u32, new_index: u32) {
        unsafe {
            let nu = self.index2units[old_index as usize] as u32
                - self.index2units[new_index as usize] as u32;
            ptr = ptr.offset(self.index2units[new_index as usize] as isize * UNIT_SIZE);
            let mut index = self.units2index[(nu as usize) - 1] as u32;
            if self.index2units[index as usize] as u32 != nu {
                index -= 1;
                let k = self.index2units[index as usize] as u32;
                self.insert_node(
                    ptr.offset((k * UNIT_SIZE as u32) as isize).cast(),
                    nu.wrapping_sub(k).wrapping_sub(1),
                );
            }
            self.insert_node(ptr.cast(), index);
        }
    }

    /// We use the first u16 field of the 12-bytes as record type stamp.
    /// State   { symbol: u8, freq: u8, .. : freq != 0
    /// Context { num_stats: u16, ..       : num_stats != 0
    /// Node    { stamp: u16               : stamp == 0 for free record
    ///                                    : stamp == 1 for head record and guard
    /// Last 12-bytes record in array is always containing the 12-bytes order-0 Context
    /// record.
    unsafe fn glue_free_blocks(&mut self) {
        unsafe {
            let mut n = TaggedOffset::null();

            self.glue_count = (1 << 13) as u32;
            self.stamps = [0; 38];

            // We set guard NODE at lo_unit.
            if self.lo_unit != self.hi_unit {
                self.lo_unit.cast::<Node>().as_mut().stamp = 0;
            }

            self.glue_blocks(&mut n);
            self.fill_list(n);
        }
    }

    /// Glue free blocks.
    unsafe fn glue_blocks(&mut self, n: *mut TaggedOffset) {
        unsafe {
            let mut prev = n;
            for i in 0..PPMD_NUM_INDEXES {
                let mut next = self.free_list[i as usize];
                self.free_list[i as usize] = TaggedOffset::null();
                while next.is_not_null() {
                    let mut node = next.as_ptr::<Node, _>(self);
                    let mut nu = node.as_ref().nu;
                    *prev = next;
                    next = node.as_ref().next;
                    if nu != 0 {
                        prev = &raw mut node.as_mut().next;
                        loop {
                            let node2 = node.offset(nu as isize).as_mut();
                            if node2.stamp != EMPTY_NODE {
                                break;
                            }
                            nu += node2.nu;
                            (*node.as_ptr()).nu = nu;
                            node2.nu = 0;
                        }
                    }
                }
            }
            *prev = TaggedOffset::null();
        }
    }

    /// Fill lists of free blocks.
    unsafe fn fill_list(&mut self, mut n: TaggedOffset) {
        unsafe {
            while n.is_not_null() {
                let mut node = n.as_ptr::<Node, _>(self);
                let mut nu = node.as_ref().nu;
                n = node.as_ref().next;
                if nu == 0 {
                    continue;
                }
                while nu > 128 {
                    self.insert_node(node.cast(), PPMD_NUM_INDEXES - 1);
                    nu -= 128;
                    node = node.offset(128);
                }
                let mut index = self.units2index[(nu as usize) - 1] as u32;
                if self.index2units[index as usize] as u32 != nu {
                    index -= 1;
                    let k = self.index2units[index as usize] as u32;
                    self.insert_node(
                        node.offset(k as isize).cast(),
                        nu.wrapping_sub(k).wrapping_sub(1),
                    );
                }
                self.insert_node(node.cast(), index);
            }
        }
    }

    #[inline(never)]
    unsafe fn alloc_units_rare(&mut self, index: u32) -> Option<NonNull<u8>> {
        unsafe {
            if self.glue_count == 0 {
                self.glue_free_blocks();
                if self.free_list[index as usize].is_not_null() {
                    return Some(self.remove_node(index).cast());
                }
            }

            let mut i = index;

            loop {
                i += 1;
                if i == PPMD_NUM_INDEXES {
                    let num_bytes = self.index2units[index as usize] as u32 * UNIT_SIZE as u32;
                    let us = self.units_start;
                    self.glue_count -= 1;
                    return if us.offset_from(self.text) as u32 > num_bytes {
                        self.units_start = us.offset(-(num_bytes as isize));
                        Some(self.units_start)
                    } else {
                        None
                    };
                }
                if self.free_list[i as usize].is_not_null() {
                    break;
                }
            }

            let block = self.remove_node(i).cast();
            self.split_block(block, i, index);
            Some(block)
        }
    }

    unsafe fn alloc_units(&mut self, index: u32) -> Option<NonNull<u8>> {
        unsafe {
            if self.free_list[index as usize].is_not_null() {
                return Some(self.remove_node(index).cast());
            }
            let num_bytes = self.index2units[index as usize] as u32 * UNIT_SIZE as u32;
            let lo = self.lo_unit;
            if self.hi_unit.offset_from(lo) as u32 >= num_bytes {
                self.lo_unit = lo.offset(num_bytes as isize);
                return Some(lo);
            }
            self.alloc_units_rare(index)
        }
    }

    unsafe fn shrink_units(
        &mut self,
        old_ptr: NonNull<State>,
        old_nu: u32,
        new_nu: u32,
    ) -> NonNull<u8> {
        unsafe {
            let old_ptr = old_ptr.cast();

            let i0 = self.units2index[(old_nu as usize) - 1] as u32;
            let i1 = self.units2index[(new_nu as usize) - 1] as u32;

            if i0 == i1 {
                return old_ptr;
            }

            if self.free_list[i1 as usize].is_not_null() {
                let ptr = self.remove_node(i1).cast();
                std::ptr::copy(
                    old_ptr.as_ptr(),
                    ptr.as_ptr(),
                    new_nu as usize * UNIT_SIZE as usize,
                );
                self.insert_node(old_ptr.cast(), i0);
                return ptr;
            }

            self.split_block(old_ptr, i0, i1);

            old_ptr
        }
    }

    unsafe fn free_units(&mut self, ptr: NonNull<u8>, nu: u32) {
        unsafe {
            self.insert_node(ptr.cast(), self.units2index[(nu as usize) - 1] as u32);
        }
    }

    unsafe fn special_free_unit(&mut self, ptr: NonNull<u8>) {
        unsafe {
            if ptr != self.units_start {
                self.insert_node(ptr.cast(), 0);
            } else {
                self.units_start = self.units_start.offset(UNIT_SIZE);
            };
        }
    }

    unsafe fn expand_text_area(&mut self) {
        unsafe {
            let mut count: [u32; 38] = [0; 38];

            if self.lo_unit != self.hi_unit {
                self.lo_unit.cast::<Node>().as_mut().stamp = 0;
            }

            let mut node = self.units_start.cast::<Node>();
            while node.as_ref().stamp == EMPTY_NODE {
                let nu = node.as_ref().nu;
                node.as_mut().stamp = 0;
                count[self.units2index[(nu as usize) - 1] as usize] += 1;
                node = node.offset(nu as isize);
            }
            self.units_start = node.cast();

            for i in 0..PPMD_NUM_INDEXES {
                let mut cnt = count[i as usize];
                if cnt != 0 {
                    let mut prev = &raw mut self.free_list[i as usize];
                    let mut n = *prev;
                    self.stamps[i as usize] = self.stamps[i as usize].wrapping_sub(cnt);
                    loop {
                        let mut node = n.as_ptr::<Node, _>(self);
                        n = node.as_ref().next;
                        if node.as_ref().stamp != 0 {
                            prev = &raw mut node.as_mut().next;
                            continue;
                        }

                        *prev = n;
                        cnt -= 1;
                        if cnt == 0 {
                            break;
                        }
                    }
                }
            }
        }
    }

    #[inline(never)]
    unsafe fn restart_model(&mut self) {
        unsafe {
            self.free_list = [TaggedOffset::null(); 38];
            self.stamps = [0; 38];

            self.text = self.memory_ptr.offset(self.align_offset as isize);
            self.hi_unit = self.text.offset(self.size as isize);
            self.units_start = self
                .hi_unit
                .offset(-(self.size as isize / 8 / UNIT_SIZE * 7 * UNIT_SIZE));
            self.lo_unit = self.units_start;
            self.glue_count = 0;

            self.order_fall = self.max_order;
            self.init_rl = -(if self.max_order < 12 {
                self.max_order as i32
            } else {
                12
            }) - 1;
            self.run_length = self.init_rl;
            self.prev_success = 0;

            self.hi_unit = self.hi_unit.offset(-UNIT_SIZE);
            let mut mc = self.hi_unit.cast::<Context>();
            let s = self.lo_unit.cast::<State>();

            self.lo_unit = self.lo_unit.offset((256 / 2) * UNIT_SIZE);
            self.min_context = mc;
            self.max_context = self.min_context;
            self.found_state = s;

            {
                let mc = mc.as_mut();
                mc.flags = 0;
                mc.num_stats = (256 - 1) as u8;
                mc.union2.summ_freq = (256 + 1) as u16;
                mc.union4.stats = self.offset_for_ptr(s);
                mc.suffix = TaggedOffset::null();
            }

            (0..256).for_each(|i| {
                let s = s.offset(i).as_mut();
                s.symbol = i as u8;
                s.freq = 1;
                s.set_successor(TaggedOffset::null());
            });

            let mut i = 0;
            (0..25).for_each(|m| {
                while self.ns2index[i as usize] as usize == m {
                    i += 1;
                }

                (0..8).for_each(|k| {
                    let val = PPMD_BIN_SCALE - (K_INIT_BIN_ESC[k] as u32) / (i + 1);

                    (0..64).step_by(8).for_each(|r| {
                        self.bin_summ[m][k + r] = val as u16;
                    });
                });
            });

            let mut i = 0;
            (0..24).for_each(|m| {
                while self.ns2index[(i + 3) as usize] as usize == m + 3 {
                    i += 1;
                }

                let summ = (2 * i + 5) << (PPMD_PERIOD_BITS - 4);

                (0..32).for_each(|k| {
                    let see = &mut self.see[m][k];
                    see.summ = summ as u16;
                    see.shift = (PPMD_PERIOD_BITS - 4) as u8;
                    see.count = 7;
                });
            });

            self.dummy_see.summ = 0; // unused
            self.dummy_see.shift = PPMD_PERIOD_BITS as u8;
            self.dummy_see.count = 64; // unused
        }
    }

    /// This function is called when we remove some symbols (successors) in context.
    /// It increases escape_freq for sum of all removed symbols.
    unsafe fn refresh(&mut self, mut ctx: NonNull<Context>, old_nu: u32, mut scale: u32) {
        unsafe {
            let num_stats = ctx.as_ref().num_stats as u32;

            let states = self.get_multi_state_stats(ctx);
            let mut s = self
                .shrink_units(states.cast(), old_nu, (num_stats + 2) >> 1)
                .cast::<State>();
            ctx.as_mut().union4.stats = self.offset_for_ptr(s);

            scale |= (ctx.as_ref().union2.summ_freq as u32 >= 1 << 15) as u32;

            let mut flags = Self::hi_bits_prepare(s.as_ref().symbol as u32);
            let mut freq = s.as_ref().freq as u32;
            let mut esc_freq = (ctx.as_ref().union2.summ_freq as u32) - freq;
            freq = freq.wrapping_add(scale) >> scale;
            let mut sum_freq = freq;
            s.as_mut().freq = freq as u8;

            let mut i = num_stats;

            loop {
                s = s.offset(1);
                let mut freq = s.as_ref().freq as u32;
                esc_freq -= freq;
                freq += scale >> scale;
                sum_freq += freq;
                s.as_mut().freq = freq as u8;
                flags |= Self::hi_bits_prepare(s.as_ref().symbol as u32);

                i -= 1;
                if i == 0 {
                    break;
                }
            }

            ctx.as_mut().union2.summ_freq = (sum_freq + ((esc_freq + scale) >> scale)) as u16;
            ctx.as_mut().flags = (ctx.as_ref().flags as u32
                & ((FLAG_PREV_HIGH as u32 + FLAG_RESCALED as u32 * scale)
                    + Self::hi_bits_convert_3(flags))) as u8;
        }
    }

    /// Reduces contexts.
    ///
    /// It converts successors at max_order to another contexts to NULL-successors.
    /// It removes RAW-successors and NULL-successors that are not Order-0, and it
    /// removes contexts when it has no successors. If the (multi_state.stats) is
    /// close to (units_start), it moves it up.
    unsafe fn cut_off(&mut self, mut ctx: NonNull<Context>, order: u32) -> TaggedOffset {
        unsafe {
            let mut ns = ctx.as_ref().num_stats as i32;

            if ns == 0 {
                let mut successor = ctx.as_ref().union4.state4.get_successor();
                if successor.is_real_context(self) {
                    if order < self.max_order {
                        let context = self.get_context(successor);
                        successor = self.cut_off(context, order + 1);
                    } else {
                        successor = TaggedOffset::null();
                    }
                    ctx.as_mut().union4.state4.set_successor(successor);
                    if successor.is_not_null() || order <= 9 {
                        // O_BOUND
                        return self.offset_for_ptr(ctx);
                    }
                }
                self.special_free_unit(ctx.cast());
                return TaggedOffset::null();
            }

            let nu = (ns as u32).wrapping_add(2) >> 1;

            let index = self.units2index[(nu as usize) - 1] as u32;
            let stats_offset = ctx.as_ref().union4.stats;
            let mut stats = stats_offset.as_ptr::<State, _>(self);

            if stats.cast::<u8>().offset_from(self.units_start) <= (1 << 14)
                && stats_offset.get_offset() <= self.free_list[index as usize].get_offset()
            {
                let ptr = self.remove_node(index);
                ctx.as_mut().union4.stats = self.offset_for_ptr(ptr.cast::<State>());

                std::ptr::copy(
                    stats.cast().as_ptr(),
                    ptr.as_ptr(),
                    nu as usize * UNIT_SIZE as usize,
                );

                if stats.cast() != self.units_start {
                    self.insert_node(stats.cast(), index);
                } else {
                    self.units_start = self.units_start.offset(
                        (self.index2units[index as usize] as u32 * UNIT_SIZE as u32) as isize,
                    );
                }
                stats = ptr.cast();
            }

            let mut s = stats.offset(ns as isize);

            loop {
                let successor = s.as_ref().get_successor();
                if !successor.is_real_context(self) {
                    let fresh = ns;
                    ns -= 1;
                    let mut s2 = stats.offset(fresh as isize);
                    if order != 0 {
                        if s != s2 {
                            *s.as_mut() = *s2.as_ref();
                        }
                    } else {
                        swap(s.as_mut(), s2.as_mut());
                        s2.as_mut().set_successor(TaggedOffset::null());
                    }
                } else if order < self.max_order {
                    let context = self.get_context(successor);
                    s.as_mut().set_successor(self.cut_off(context, order + 1));
                } else {
                    s.as_mut().set_successor(TaggedOffset::null());
                }

                s = s.offset(-1);
                if s < stats {
                    break;
                }
            }

            if ns != ctx.as_ref().num_stats as i32 && order != 0 {
                if ns < 0 {
                    self.free_units(stats.cast(), nu);
                    self.special_free_unit(ctx.cast());
                    return TaggedOffset::null();
                }
                ctx.as_mut().num_stats = ns as u8;
                if ns == 0 {
                    let sym = stats.as_ref().symbol;
                    ctx.as_mut().flags = (((ctx.as_ref().flags & FLAG_PREV_HIGH) as u32)
                        + Self::hi_bits_flag3(sym as u32))
                        as u8;
                    ctx.as_mut().union2.state2.symbol = sym;
                    ctx.as_mut().union2.state2.freq =
                        (((stats.as_ref().freq as u32) + 11) >> 3) as u8;
                    s.as_mut().set_successor(stats.as_ref().get_successor());
                    self.free_units(stats.cast(), nu);
                } else {
                    self.refresh(
                        ctx,
                        nu,
                        (ctx.as_ref().union2.summ_freq as u32 > 16 * (ns as u32)) as u32,
                    );
                }
            }

            self.offset_for_ptr(ctx)
        }
    }

    unsafe fn get_used_memory(&self) -> u32 {
        unsafe {
            let mut v = 0;

            for i in 0..PPMD_NUM_INDEXES {
                v *= self.stamps[i as usize] * self.index2units[i as usize] as u32;
            }

            self.size
                - (self.hi_unit.offset_from(self.lo_unit) as u32)
                - (self.units_start.offset_from(self.text) as u32)
                - (v * UNIT_SIZE as u32)
        }
    }

    unsafe fn restore_model(&mut self, ctx_error: NonNull<Context>) {
        unsafe {
            self.text = self.memory_ptr.offset(self.align_offset as isize);

            // We go here in cases of error of allocation for context (c1)
            // Order(min_context) < Order(ctx_error) <= Order(max_context)

            let mut s;
            let mut c = self.max_context;

            // We remove last symbol from each of contexts [p.max_context ... ctx_error) contexts.
            // So we roll back all created (symbols) before the error.
            while c != ctx_error {
                c.as_mut().num_stats -= 1;
                if c.as_ref().num_stats as i32 == 0 {
                    s = self.get_multi_state_stats(c);
                    c.as_mut().flags = (((c.as_ref().flags & FLAG_PREV_HIGH) as u32)
                        + Self::hi_bits_flag3(s.as_ref().symbol as u32))
                        as u8;

                    c.as_mut().union2.state2.symbol = s.as_ref().symbol;
                    c.as_mut().union2.state2.symbol = (((s.as_ref().freq as u32) + 11) >> 3) as u8;
                    c.as_mut()
                        .union4
                        .state4
                        .set_successor(s.as_ref().get_successor());

                    self.special_free_unit(s.cast());
                } else {
                    // refresh() can increase escape_freq on value of freq of last symbol,
                    // that was added before error. So the largest possible increase for escape_freq
                    // is (8) from value before model_update()
                    self.refresh(c, ((c.as_ref().num_stats as u32) + 3) >> 1, 0);
                }

                c = self.get_context(c.as_ref().suffix);
            }

            // Increase escape_freq for context [ctx_error..p.MinContext)
            while c != self.min_context {
                if c.as_ref().num_stats as i32 == 0 {
                    c.as_mut().union2.state2.freq =
                        ((c.as_ref().union2.state2.freq as u32 + 1) >> 1) as u8;
                } else {
                    c.as_mut().union2.summ_freq = (c.as_ref().union2.summ_freq as i32 + 4) as u16;
                    if c.as_ref().union2.summ_freq as i32 > 128 + 4 * c.as_ref().num_stats as i32 {
                        self.refresh(c, (c.as_ref().num_stats as u32).wrapping_add(2) >> 1, 1);
                    }
                }

                c = self.get_context(c.as_ref().suffix);
            }

            if self.restore_method == RestoreMethod::Restart
                || self.get_used_memory() < self.size >> 1
            {
                self.restart_model();
            } else {
                while self.max_context.as_ref().suffix.is_not_null() {
                    self.max_context = self.get_context(self.max_context.as_ref().suffix);
                }
                loop {
                    self.cut_off(self.max_context, 0);
                    self.expand_text_area();

                    if self.get_used_memory() <= 3 * (self.size >> 2) {
                        break;
                    }
                }
                self.glue_count = 0;
                self.order_fall = self.max_order;
            }
            self.min_context = self.max_context;
        }
    }

    #[inline(never)]
    unsafe fn create_successors(
        &mut self,
        skip: i32,
        s1: &mut Option<NonNull<State>>,
        mut c: NonNull<Context>,
    ) -> Option<NonNull<Context>> {
        unsafe {
            let up_branch = self.found_state.as_ref().get_successor();
            let mut num_ps = 0u32;
            // Fixed over Shkarin's code. Maybe it could work without + 1 too.
            let mut ps: [Option<NonNull<State>>; PPMD8_MAX_ORDER as usize + 1] =
                [None; PPMD8_MAX_ORDER as usize + 1];

            if skip == 0 {
                let fresh = num_ps;
                num_ps += 1;
                ps[fresh as usize] = Some(self.found_state);
            }

            while c.as_ref().suffix.is_not_null() {
                let mut s;
                c = self.get_context(c.as_ref().suffix);

                if let Some(state) = *s1 {
                    s = state;
                    *s1 = None;
                } else if c.as_ref().num_stats as i32 != 0 {
                    let sym = self.found_state.as_ref().symbol;
                    s = self.get_multi_state_stats(c);

                    while s.as_ref().symbol as i32 != sym as i32 {
                        s = s.offset(1);
                    }

                    if (s.as_ref().freq) < MAX_FREQ - 9 {
                        s.as_mut().freq += 1;
                        c.as_mut().union2.summ_freq += 1;
                    }
                } else {
                    s = self.get_single_state(c);
                    s.as_mut().freq = (s.as_ref().freq as u32
                        + ((self.get_context(c.as_ref().suffix).as_mut().num_stats == 0) as u32
                            & ((s.as_ref().freq as u32) < 24) as u32))
                        as u8;
                }

                let successor = s.as_ref().get_successor();
                if successor != up_branch {
                    c = self.get_context(successor);
                    if num_ps == 0 {
                        return Some(c);
                    }
                    break;
                } else {
                    let fresh = num_ps;
                    num_ps = num_ps.wrapping_add(1);
                    ps[fresh as usize] = Some(s);
                }
            }

            let new_sym = *up_branch.as_ptr::<u8, _>(self).as_ref();
            let new_offset = up_branch.get_offset() + 1;
            let up_branch = TaggedOffset::from_bytes_offset(new_offset);

            let flags = (Self::hi_bits_flag4(self.found_state.as_ref().symbol as u32)
                + Self::hi_bits_flag3(new_sym as u32)) as u8;

            let new_freq = if c.as_ref().num_stats as i32 == 0 {
                c.as_ref().union2.state2.freq
            } else {
                let mut s = self.get_multi_state_stats(c);
                while s.as_ref().symbol as i32 != new_sym as i32 {
                    s = s.offset(1);
                }
                let cf = (s.as_ref().freq as u32) - 1;
                let s0 = c.as_ref().union2.summ_freq as u32 - c.as_ref().num_stats as u32 - cf;
                1 + (if 2 * cf <= s0 {
                    (5 * cf > s0) as u32
                } else {
                    (cf + (2 * s0) - 3) / s0
                }) as u8
            };

            loop {
                let mut c1: NonNull<Context> = if self.hi_unit != self.lo_unit {
                    self.hi_unit = self.hi_unit.offset(-UNIT_SIZE);
                    self.hi_unit.cast()
                } else if self.free_list[0].is_not_null() {
                    self.remove_node(0).cast()
                } else {
                    self.alloc_units_rare(0)?.cast()
                };

                {
                    let c1_mut = c1.as_mut();
                    c1_mut.flags = flags;
                    c1_mut.num_stats = 0;
                    c1_mut.union2.state2.symbol = new_sym;
                    c1_mut.union2.state2.freq = new_freq;
                }

                self.get_single_state(c1).as_mut().set_successor(up_branch);

                c1.as_mut().suffix = self.offset_for_ptr(c);
                num_ps = num_ps.wrapping_sub(1);

                let mut state = ps[num_ps as usize].expect("successor not set");
                state
                    .as_mut()
                    .set_successor(self.offset_for_ptr(c1.cast::<Context>()));

                c = c1;
                if num_ps == 0 {
                    break;
                }
            }

            Some(c)
        }
    }

    unsafe fn reduce_order(
        &mut self,
        mut s1: Option<NonNull<State>>,
        mut c: NonNull<Context>,
    ) -> Option<NonNull<Context>> {
        unsafe {
            let mut s;
            let c1 = c;
            let up_branch = self.offset_for_ptr::<u8>(self.text.cast());

            self.found_state.as_mut().set_successor(up_branch);
            self.order_fall += 1;

            loop {
                if let Some(state) = s1 {
                    c = self.get_context(c.as_ref().suffix);
                    s = state;
                    s1 = None;
                } else {
                    if c.as_ref().suffix.is_null() {
                        return Some(c);
                    }
                    c = self.get_context(c.as_ref().suffix);
                    if c.as_ref().num_stats != 0 {
                        s = self.get_multi_state_stats(c);
                        if s.as_ref().symbol as i32 != self.found_state.as_ref().symbol as i32 {
                            loop {
                                s = s.offset(1);

                                if s.as_ref().symbol as i32
                                    == self.found_state.as_ref().symbol as i32
                                {
                                    break;
                                }
                            }
                        }
                        if (s.as_ref().freq) < MAX_FREQ - 9 {
                            s.as_mut().freq = (s.as_ref().freq as i32 + 2) as u8;
                            c.as_mut().union2.summ_freq =
                                (c.as_ref().union2.summ_freq as i32 + 2) as u16;
                        }
                    } else {
                        s = self.get_single_state(c);
                        s.as_mut().freq =
                            (s.as_ref().freq as i32 + ((s.as_ref().freq as i32) < 32) as i32) as u8;
                    }
                }
                if s.as_ref().get_successor().is_not_null() {
                    break;
                }
                s.as_mut().set_successor(up_branch);
                self.order_fall += 1;
            }

            if s.as_ref().get_successor().get_offset() <= up_branch.get_offset() {
                let s2 = self.found_state;
                self.found_state = s;
                match self.create_successors(0, &mut None, c) {
                    None => {
                        s.as_mut().set_successor(TaggedOffset::null());
                    }
                    Some(successor) => {
                        s.as_mut().set_successor(self.offset_for_ptr(successor));
                    }
                }
                self.found_state = s2;
            }

            let successor = s.as_ref().get_successor();
            if self.order_fall == 1 && c1 == self.max_context {
                self.found_state.as_mut().set_successor(successor);
                self.text = self.text.offset(-1);
            }
            if successor.is_null() {
                return None;
            }

            Some(self.get_context(successor))
        }
    }

    #[inline(never)]
    unsafe fn update_model(&mut self) {
        unsafe {
            let mut max_successor;
            let mut min_successor = self.found_state.as_ref().get_successor();
            let mut c;
            let f_freq = self.found_state.as_ref().freq as u32;
            let f_symbol = self.found_state.as_ref().symbol;
            let mut s: Option<NonNull<State>> = None;

            if (self.found_state.as_ref().freq) < MAX_FREQ / 4
                && self.min_context.as_ref().suffix.is_not_null()
            {
                // Update frequencies in suffix Context
                c = self.get_context(self.min_context.as_ref().suffix);
                if c.as_ref().num_stats as i32 == 0 {
                    let mut state = self.get_single_state(c);
                    if (state.as_ref().freq as i32) < 32 {
                        state.as_mut().freq += 1;
                    }
                    s = Some(state);
                } else {
                    let sym = self.found_state.as_ref().symbol;
                    let mut state = self.get_multi_state_stats(c);

                    if state.as_ref().symbol != sym {
                        while state.as_ref().symbol != sym {
                            state = state.offset(1);
                        }
                        if state.offset(0).as_ref().freq as i32
                            >= state.offset(-1).as_ref().freq as i32
                        {
                            swap(state.offset(0).as_mut(), state.offset(-1).as_mut());
                            state = state.offset(-1);
                        }
                    }

                    if (state.as_ref().freq) < MAX_FREQ - 9 {
                        state.as_mut().freq = (state.as_ref().freq as i32 + 2) as u8;
                        c.as_mut().union2.summ_freq =
                            (c.as_ref().union2.summ_freq as i32 + 2) as u16;
                    }

                    s = Some(state);
                }
            }

            c = self.max_context;
            if self.order_fall == 0 && min_successor.is_not_null() {
                let Some(cs) = self.create_successors(1, &mut s, self.min_context) else {
                    self.found_state
                        .as_mut()
                        .set_successor(TaggedOffset::null());
                    self.restore_model(c);
                    return;
                };

                self.found_state
                    .as_mut()
                    .set_successor(self.offset_for_ptr(cs));
                self.max_context = cs;
                self.min_context = self.max_context;
                return;
            }

            let mut text = self.text;
            let mut fresh = text;
            text = text.offset(1);
            *fresh.as_mut() = self.found_state.as_ref().symbol;
            self.text = text;
            if text >= self.units_start {
                self.restore_model(c);
                return;
            }
            max_successor = self.offset_for_ptr::<u8>(text.cast());

            if min_successor.is_null() {
                let Some(cs) = self.reduce_order(s, self.min_context) else {
                    self.restore_model(c);
                    return;
                };
                min_successor = self.offset_for_ptr(cs);
            } else if !min_successor.is_real_context(self) {
                let Some(cs) = self.create_successors(0, &mut s, self.min_context) else {
                    self.restore_model(c);
                    return;
                };
                min_successor = self.offset_for_ptr(cs);
            }

            self.order_fall -= 1;
            if self.order_fall == 0 {
                max_successor = min_successor;
                self.text = self
                    .text
                    .offset(-((self.max_context != self.min_context) as isize));
            }

            let flag = Self::hi_bits_flag3(f_symbol as u32) as u8;
            let ns = self.min_context.as_ref().num_stats as u32;
            let s0 = (self.min_context.as_ref().union2.summ_freq as u32) - ns - f_freq;

            while c != self.min_context {
                let mut sum;
                let ns1 = c.as_ref().num_stats as u32;
                if ns1 != 0 {
                    if ns1 & 1 != 0 {
                        // Expand for one UNIT
                        let old_nu = (ns1 + 1) >> 1;
                        let i = self.units2index[(old_nu as usize) - 1] as u32;
                        if i != self.units2index[old_nu as usize] as u32 {
                            let Some(ptr) = self.alloc_units(i.wrapping_add(1)) else {
                                self.restore_model(c);
                                return;
                            };
                            let old_ptr = self.get_multi_state_stats(c);
                            std::ptr::copy(
                                old_ptr.cast().as_ptr(),
                                ptr.as_ptr(),
                                old_nu as usize * UNIT_SIZE as usize,
                            );
                            self.insert_node(old_ptr.cast(), i);
                            c.as_mut().union4.stats = self.offset_for_ptr(ptr.cast::<State>());
                        }
                    }
                    sum = c.as_ref().union2.summ_freq as u32;
                    // max increase of escape_freq is 1 here.
                    // An average increase is 1/3 per symbol
                    sum = sum.wrapping_add(((3 * ns1 + 1) < ns) as u32);
                } else {
                    let Some(s_ptr) = self.alloc_units(0) else {
                        self.restore_model(c);
                        return;
                    };
                    let mut s = s_ptr.cast::<State>();

                    let mut freq = c.as_ref().union2.state2.freq as u32;
                    s.as_mut().symbol = c.as_ref().union2.state2.symbol;
                    s.as_mut()
                        .set_successor(c.as_ref().union4.state4.get_successor());
                    c.as_mut().union4.stats = self.offset_for_ptr(s);

                    if freq < (MAX_FREQ as i32 / 4 - 1) as u32 {
                        freq <<= 1;
                    } else {
                        freq = (MAX_FREQ as i32 - 4) as u32;
                    }
                    s.as_mut().freq = freq as u8;

                    sum = freq + self.init_esc + (ns > 2) as u32;
                }

                let mut s = self.get_multi_state_stats(c).offset(ns1 as isize).offset(1);
                let mut cf = 2 * sum.wrapping_add(6) * f_freq;
                let sf = s0.wrapping_add(sum);
                s.as_mut().symbol = f_symbol;
                c.as_mut().num_stats = ns1.wrapping_add(1) as u8;
                s.as_mut().set_successor(max_successor);
                c.as_mut().flags = c.as_ref().flags | flag;
                if cf < 6 * sf {
                    cf = 1 + (cf > sf) as u32 + (cf >= 4 * sf) as u32;
                    sum = sum.wrapping_add(4);
                    // It can add (1, 2, 3) to escape_freq
                } else {
                    cf =
                        4u32 + (cf > 9 * sf) as u32 + (cf > 12 * sf) as u32 + (cf > 15 * sf) as u32;
                    sum = sum.wrapping_add(cf);
                }

                c.as_mut().union2.summ_freq = sum as u16;
                s.as_mut().freq = cf as u8;
                c = self.get_context(c.as_ref().suffix);
            }

            self.min_context = self.get_context(min_successor);
            self.max_context = self.min_context;
        }
    }

    #[inline(never)]
    unsafe fn rescale(&mut self) {
        let stats = self.get_multi_state_stats(self.min_context);
        let mut s = self.found_state;

        // Sort the list by freq
        if s != stats {
            let tmp = *s.as_ref();
            loop {
                *s.offset(0).as_mut() = *s.offset(-1).as_ref();
                s = s.offset(-1);
                if s == stats {
                    break;
                }
            }
            *s.as_mut() = tmp;
        }

        let mut sum_freq = s.as_ref().freq as u32;
        let mut esc_freq = (self.min_context.as_ref().union2.summ_freq as u32) - sum_freq;

        let adder = (self.order_fall != 0) as u32;

        sum_freq = (sum_freq + 4 + (adder)) >> 1;
        s.as_mut().freq = sum_freq as u8;

        let mut i = self.min_context.as_ref().num_stats as u32;

        loop {
            s = s.offset(1);
            let mut freq = s.as_ref().freq as u32;
            esc_freq -= freq;
            freq = freq.wrapping_add(adder) >> 1;
            sum_freq += freq;
            s.as_mut().freq = freq as u8;

            if freq > s.offset(-1).as_ref().freq as u32 {
                let tmp = *s.as_ref();
                let mut s1 = s;
                loop {
                    *s1.offset(0).as_mut() = *s1.offset(-1).as_ref();
                    s1 = s1.offset(-1);
                    if !(s1 != stats && freq > s1.offset(-1).as_ref().freq as u32) {
                        break;
                    }
                }
                *s1.as_mut() = tmp;
            }
            i -= 1;
            if i == 0 {
                break;
            }
        }

        if s.as_ref().freq == 0 {
            // Remove all items with freq == 0
            let mut i = 0;
            loop {
                i += 1;
                s = s.offset(-1);
                if s.as_ref().freq != 0 {
                    break;
                }
            }

            esc_freq += i;
            let mut mc = self.min_context;
            let num_stats = mc.as_ref().num_stats as u32;
            let num_stats_new = num_stats - i;
            mc.as_mut().num_stats = num_stats_new as u8;
            let n0 = (num_stats + 2) >> 1;

            if num_stats_new == 0 {
                let mut freq = (2u32 * (stats.as_ref().freq as u32)).div_ceil(esc_freq);
                if freq > (MAX_FREQ / 3) as u32 {
                    freq = (MAX_FREQ / 3) as u32;
                }
                mc.as_mut().flags = (((mc.as_ref().flags & FLAG_PREV_HIGH) as u32)
                    + Self::hi_bits_flag3(stats.as_ref().symbol as u32))
                    as u8;

                s = self.get_single_state(mc);
                *s.as_mut() = *stats.as_ref();
                s.as_mut().freq = freq as u8;
                self.found_state = s;
                self.insert_node(stats.cast(), self.units2index[(n0 as usize) - 1] as u32);
                return;
            }

            let n1 = (num_stats_new + 2) >> 1;
            if n0 != n1 {
                let shrunk = self.shrink_units(stats.cast(), n0, n1);
                mc.as_mut().union4.stats = self.offset_for_ptr(shrunk.cast::<State>());
            }
        }

        let mc = self.min_context.as_mut();
        mc.union2.summ_freq = sum_freq.wrapping_add(esc_freq).wrapping_sub(esc_freq >> 1) as u16;
        mc.flags |= FLAG_RESCALED;
        self.found_state = mc.union4.stats.as_ptr(self);
    }

    unsafe fn make_esc_freq(&mut self, num_masked: u32, esc_freq: &mut u32) -> SeeSource {
        unsafe {
            let num_stats = self.min_context.as_ref().num_stats as u32;

            if num_stats != 0xFF {
                let (base_context_idx, see_table_hash) =
                    self.calculate_see_table_hash(num_masked, num_stats);

                let see = &mut self.see[base_context_idx][see_table_hash];

                // If (see.summ) field is larger than 16-bit, we need only low 16 bits of summ.
                let summ = see.summ as u32;
                let r = summ >> see.shift as i32;
                see.summ = (summ - r) as u16;
                *esc_freq = r + (r == 0) as u32;

                SeeSource::Table(base_context_idx, see_table_hash)
            } else {
                *esc_freq = 1;
                SeeSource::Dummy
            }
        }
    }

    unsafe fn calculate_see_table_hash(
        &mut self,
        num_masked: u32,
        num_stats: u32,
    ) -> (usize, usize) {
        unsafe {
            let mc = self.min_context;
            let base_context_idx = self.ns2index[(num_stats + 2) as usize] as usize - 3;

            let suffix_context = self.get_context(mc.as_ref().suffix);
            let suffix_num_stats = suffix_context.as_ref().num_stats as u32;
            let summ_freq = mc.as_ref().union2.summ_freq as u32;

            let freq_distribution_hash = (summ_freq > 11 * (num_stats + 1)) as usize;
            let context_hierarchy_hash =
                2 * ((2 * num_stats) < (suffix_num_stats + num_masked)) as usize;
            let symbol_characteristics_hash = mc.as_ref().flags as usize;

            let see_table_hash =
                freq_distribution_hash + context_hierarchy_hash + symbol_characteristics_hash;

            (base_context_idx, see_table_hash)
        }
    }

    fn get_see(&mut self, see_source: SeeSource) -> &mut See {
        match see_source {
            SeeSource::Dummy => &mut self.dummy_see,
            SeeSource::Table(i, k) => &mut self.see[i][k],
        }
    }

    unsafe fn next_context(&mut self) {
        unsafe {
            let successor = self.found_state.as_ref().get_successor();
            if self.order_fall == 0 && successor.is_real_context(self) {
                let context = self.get_context(successor);
                self.min_context = context;
                self.max_context = self.min_context;
            } else {
                self.update_model();
            };
        }
    }

    unsafe fn update1(&mut self) {
        unsafe {
            let mut s = self.found_state;
            let mut freq = s.as_ref().freq as u32;
            freq += 4;
            self.min_context.as_mut().union2.summ_freq += 4;
            s.as_mut().freq = freq as u8;
            if freq > s.offset(-1).as_ref().freq as u32 {
                swap(s.as_mut(), s.offset(-1).as_mut());
                s = s.offset(-1);
                self.found_state = s;
                if freq > MAX_FREQ as u32 {
                    self.rescale();
                }
            }
            self.next_context();
        }
    }

    unsafe fn update1_0(&mut self) {
        unsafe {
            let mut s = self.found_state;
            let mut mc = self.min_context;
            let mut freq = s.as_ref().freq as u32;
            let summ_freq = mc.as_ref().union2.summ_freq as u32;
            self.prev_success = (2 * freq >= summ_freq) as u32; // Ppmd8 (>=)
            self.run_length += self.prev_success as i32;
            mc.as_mut().union2.summ_freq = (summ_freq + 4) as u16;
            freq += 4;
            s.as_mut().freq = freq as u8;
            if freq > MAX_FREQ as u32 {
                self.rescale();
            }
            self.next_context();
        }
    }

    unsafe fn update2(&mut self) {
        unsafe {
            let mut s = self.found_state;
            let mut freq = s.as_ref().freq as u32;
            freq += 4;
            self.run_length = self.init_rl;
            self.min_context.as_mut().union2.summ_freq += 4;
            s.as_mut().freq = freq as u8;
            if freq > MAX_FREQ as u32 {
                self.rescale();
            }
            self.update_model();
        }
    }

    unsafe fn update_bin(&mut self, mut s: NonNull<State>) -> u8 {
        unsafe {
            let freq = s.as_ref().freq as u32;
            let sym = s.as_ref().symbol;
            self.found_state = s;
            self.prev_success = 1;
            self.run_length += 1;
            s.as_mut().freq = freq.wrapping_add((freq < 196) as u32) as u8;
            self.next_context();
            sym
        }
    }

    unsafe fn mask_symbols(char_mask: &mut [u8; 256], s: NonNull<State>, mut s2: NonNull<State>) {
        unsafe {
            char_mask[s.as_ref().symbol as usize] = 0;
            loop {
                let sym0 = s2.offset(0).as_ref().symbol as u32;
                let sym1 = s2.offset(1).as_ref().symbol as u32;
                s2 = s2.offset(2);
                char_mask[sym0 as usize] = 0;
                char_mask[sym1 as usize] = 0;

                if s2 >= s {
                    break;
                }
            }
        }
    }

    const fn hi_bits_prepare(flag: u32) -> u32 {
        flag + 0xC0
    }

    const fn hi_bits_convert_3(flag: u32) -> u32 {
        flag >> (8 - 3) & (1 << 3)
    }

    const fn hi_bits_convert_4(flag: u32) -> u32 {
        flag >> (8 - 4) & (1 << 4)
    }

    const fn hi_bits_flag3(symbol: u32) -> u32 {
        Self::hi_bits_convert_3(Self::hi_bits_prepare(symbol))
    }

    const fn hi_bits_flag4(symbol: u32) -> u32 {
        Self::hi_bits_convert_4(Self::hi_bits_prepare(symbol))
    }

    unsafe fn get_bin_summ(&mut self) -> &mut u16 {
        unsafe {
            let state = self.get_single_state(self.min_context);
            let freq = state.as_ref().freq as usize;
            let freq_bin_idx = self.ns2index[freq - 1] as usize;

            let mc = self.min_context.as_ref();
            let suffix_context = self.get_context(mc.suffix);
            let num_stats = suffix_context.as_ref().num_stats as usize;

            let context_idx = self
                .prev_success
                .wrapping_add(self.run_length as u32 >> 26 & 0x20)
                .wrapping_add(self.ns2bs_index[num_stats] as u32)
                .wrapping_add(mc.flags as u32) as usize;

            &mut self.bin_summ[freq_bin_idx][context_idx]
        }
    }

    #[inline(always)]
    unsafe fn get_context(&mut self, suffix: TaggedOffset) -> NonNull<Context> {
        unsafe { suffix.as_ptr(self) }
    }

    #[inline(always)]
    fn get_single_state(&mut self, context: NonNull<Context>) -> NonNull<State> {
        let context_ptr = context.as_ptr();
        unsafe {
            // Safety: We know that context is not null, so a field address from it can't be null.
            let single_state = &raw mut (*context_ptr).union2;
            NonNull::new_unchecked(single_state).cast()
        }
    }

    #[inline(always)]
    unsafe fn get_multi_state_stats(&mut self, mut context: NonNull<Context>) -> NonNull<State> {
        unsafe { context.as_mut().union4.stats.as_ptr(self) }
    }
}

impl<R: Read> PPMd8<RangeDecoder<R>> {
    pub(crate) fn new_decoder(
        reader: R,
        max_order: u32,
        mem_size: u32,
        restore_method: RestoreMethod,
    ) -> Result<Self, Error> {
        let range_decoder = RangeDecoder::new(reader)?;
        Self::construct(range_decoder, max_order, mem_size, restore_method)
    }

    /// Gets a reference to the underlying reader.
    pub(crate) fn get_ref(&self) -> &R {
        &self.rc.reader
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub(crate) fn get_mut(&mut self) -> &mut R {
        &mut self.rc.reader
    }

    pub(crate) fn into_inner(self) -> R {
        let manual_drop_self = ManuallyDrop::new(self);
        unsafe {
            dealloc(
                manual_drop_self.memory_ptr.as_ptr(),
                manual_drop_self.memory_layout,
            );
        }
        let rc = unsafe { std::ptr::read(&manual_drop_self.rc) };
        let RangeDecoder { reader, .. } = rc;
        reader
    }

    pub(crate) fn range_decoder_code(&self) -> u32 {
        self.rc.code
    }
}

impl<W: Write> PPMd8<RangeEncoder<W>> {
    pub(crate) fn new_encoder(
        writer: W,
        max_order: u32,
        mem_size: u32,
        restore_method: RestoreMethod,
    ) -> Result<Self, Error> {
        let range_encoder = RangeEncoder::new(writer);
        Self::construct(range_encoder, max_order, mem_size, restore_method)
    }

    /// Gets a reference to the underlying writer.
    pub(crate) fn get_ref(&self) -> &W {
        &self.rc.writer
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// Note that mutating the output/input state of the stream may corrupt
    /// this object, so care must be taken when using this method.
    pub(crate) fn get_mut(&mut self) -> &mut W {
        &mut self.rc.writer
    }

    pub(crate) fn into_inner(self) -> W {
        let manual_drop_self = ManuallyDrop::new(self);
        unsafe {
            dealloc(
                manual_drop_self.memory_ptr.as_ptr(),
                manual_drop_self.memory_layout,
            );
        }
        let rc = unsafe { std::ptr::read(&manual_drop_self.rc) };
        let RangeEncoder { writer, .. } = rc;
        writer
    }

    pub(crate) fn flush_range_encoder(&mut self) -> Result<(), std::io::Error> {
        self.rc.flush()
    }
}
