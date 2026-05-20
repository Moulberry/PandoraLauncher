pub(crate) mod ppmd7;

pub(crate) mod ppmd8;

mod tagged_offset;

pub(crate) use tagged_offset::*;

const PPMD_INT_BITS: u32 = 7;
const PPMD_PERIOD_BITS: u32 = 7;
const PPMD_BIN_SCALE: u32 = 1 << (PPMD_INT_BITS + PPMD_PERIOD_BITS);

const fn ppmd_get_mean_spec(summ: u32, shift: u32, round: u32) -> u32 {
    (summ + (1 << (shift - round))) >> shift
}

const fn ppmd_get_mean(summ: u32) -> u32 {
    ppmd_get_mean_spec(summ, PPMD_PERIOD_BITS, 2)
}

const fn ppmd_update_prob_1(prob: u32) -> u32 {
    prob - ppmd_get_mean(prob)
}

const PPMD_N1: u32 = 4;
const PPMD_N2: u32 = 4;
const PPMD_N3: u32 = 4;
const PPMD_N4: u32 = (128 + 3 - PPMD_N1 - 2 * PPMD_N2 - 3 * PPMD_N3) / 4;
const PPMD_NUM_INDEXES: u32 = PPMD_N1 + PPMD_N2 + PPMD_N3 + PPMD_N4;

enum SeeSource {
    Dummy,
    Table(usize, usize),
}

#[derive(Copy, Clone, Default)]
#[repr(C, packed)]
struct See {
    summ: u16,
    shift: u8,
    count: u8,
}

impl See {
    fn update(&mut self) {
        if (self.shift as i32) < 7 && {
            self.count = self.count.wrapping_sub(1);
            self.count as i32 == 0
        } {
            self.summ = ((self.summ as i32) << 1) as u16;
            let fresh = self.shift;
            self.shift = self.shift.wrapping_add(1);
            self.count = (3 << fresh as i32) as u8;
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct State {
    symbol: u8,
    freq: u8,
    successor_0: u16,
    successor_1: u16,
}

impl Pointee for State {
    const TAG: u32 = TAG_STATE;
}

impl State {
    fn set_successor(&mut self, v: TaggedOffset) {
        let raw = v.as_raw();
        self.successor_0 = raw as u16;
        self.successor_1 = (raw >> 16) as u16;
    }

    fn get_successor(&self) -> TaggedOffset {
        let raw = self.successor_0 as u32 + ((self.successor_1 as u32) << 16);
        TaggedOffset::from_raw(raw)
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct State2 {
    symbol: u8,
    freq: u8,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct State4 {
    successor_0: u16,
    successor_1: u16,
}

impl State4 {
    fn set_successor(&mut self, v: TaggedOffset) {
        let raw = v.as_raw();
        self.successor_0 = raw as u16;
        self.successor_1 = (raw >> 16) as u16;
    }

    fn get_successor(&self) -> TaggedOffset {
        let raw = self.successor_0 as u32 + ((self.successor_1 as u32) << 16);
        TaggedOffset::from_raw(raw)
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
union Union2 {
    summ_freq: u16,
    state2: State2,
}

#[derive(Copy, Clone)]
#[repr(C)]
union Union4 {
    stats: TaggedOffset,
    state4: State4,
}
