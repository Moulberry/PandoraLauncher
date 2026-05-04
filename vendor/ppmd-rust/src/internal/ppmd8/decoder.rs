use super::*;
use crate::{SYM_END, SYM_ERROR};

impl<R: Read> PPMd8<RangeDecoder<R>> {
    pub(crate) fn decode_symbol(&mut self) -> Result<i32, std::io::Error> {
        unsafe {
            let mut char_mask: [u8; 256];

            if self.min_context.as_ref().num_stats != 0 {
                let mut s = self.get_multi_state_stats(self.min_context);
                let mut summ_freq = self.min_context.as_ref().union2.summ_freq as u32;

                summ_freq = self.rc.correct_sum_range(summ_freq);

                let mut count = self.rc.get_threshold(summ_freq);
                let mut hi_cnt = count;

                count = count.wrapping_sub(s.as_ref().freq as u32);
                if (count as i32) < 0 {
                    self.rc.decode_final(0, s.as_ref().freq as u32)?;
                    self.found_state = s;
                    let sym = s.as_ref().symbol;
                    self.update1_0();
                    return Ok(sym as i32);
                }

                self.prev_success = 0;
                let mut i = self.min_context.as_ref().num_stats as u32;

                loop {
                    s = s.offset(1);
                    count = count.wrapping_sub(s.as_ref().freq as u32);
                    if (count as i32) < 0 {
                        let freq = s.as_ref().freq as u32;
                        self.rc
                            .decode_final(hi_cnt.wrapping_sub(count).wrapping_sub(freq), freq)?;
                        self.found_state = s;
                        let sym = s.as_ref().symbol;
                        self.update1();
                        return Ok(sym as i32);
                    }

                    i -= 1;
                    if i == 0 {
                        break;
                    }
                }

                if hi_cnt >= summ_freq {
                    return Ok(SYM_ERROR);
                }

                hi_cnt = hi_cnt.wrapping_sub(count);
                self.rc.decode(hi_cnt, summ_freq.wrapping_sub(hi_cnt));

                char_mask = [u8::MAX; 256];

                let s2 = self.get_multi_state_stats(self.min_context);
                Self::mask_symbols(&mut char_mask, s, s2);
            } else {
                let s = self.get_single_state(self.min_context);
                let range = self.rc.range;
                let code = self.rc.code;
                let prob = self.get_bin_summ();

                let mut pr = *prob as u32;
                let size0 = (range >> 14) * pr;
                pr = ppmd_update_prob_1(pr);

                if code < size0 {
                    *prob = pr.wrapping_add((1 << PPMD_INT_BITS) as u32) as u16;
                    self.rc.range = size0;
                    self.rc.normalize_remote()?;

                    let sym = self.update_bin(s);
                    return Ok(sym as i32);
                }

                *prob = pr as u16;
                self.init_esc = self.exp_escape[(pr >> 10) as usize] as u32;

                self.rc.decode_bit_1(size0);

                char_mask = [u8::MAX; 256];
                char_mask[self.min_context.as_ref().union2.state2.symbol as usize] = 0;
                self.prev_success = 0;
            }
            loop {
                let mut freq_sum = 0;
                self.rc.normalize_remote()?;
                let mut mc = self.min_context;
                let num_masked = mc.as_ref().num_stats as u32;

                loop {
                    self.order_fall += 1;
                    if mc.as_ref().suffix.is_null() {
                        return Ok(SYM_END);
                    }
                    mc = self.get_context(mc.as_ref().suffix);

                    if mc.as_ref().num_stats as u32 != num_masked {
                        break;
                    }
                }

                let s = self.get_multi_state_stats(mc);
                let mut num = (mc.as_ref().num_stats as u32) + 1;
                let mut num2 = num / 2;

                num &= 1;
                let mut hi_cnt = s.as_ref().freq as u32
                    & char_mask[s.as_ref().symbol as usize] as u32
                    & 0u32.wrapping_sub(num);
                let mut s = s.offset(num as isize);
                self.min_context = mc;

                loop {
                    let sym0 = s.offset(0).as_ref().symbol as u32;
                    let sym1 = s.offset(1).as_ref().symbol as u32;
                    s = s.offset(2);
                    hi_cnt += s.offset(-2).as_ref().freq as u32 & char_mask[sym0 as usize] as u32;
                    hi_cnt += s.offset(-1).as_ref().freq as u32 & char_mask[sym1 as usize] as u32;

                    num2 -= 1;
                    if num2 == 0 {
                        break;
                    }
                }

                let see_source = self.make_esc_freq(num_masked, &mut freq_sum);
                freq_sum += hi_cnt;
                let freq_sum2 = self.rc.correct_sum_range(freq_sum);

                let mut count = self.rc.get_threshold(freq_sum2);

                if count < hi_cnt {
                    s = self.get_multi_state_stats(self.min_context);
                    hi_cnt = count;

                    loop {
                        count = count.wrapping_sub(
                            s.as_ref().freq as u32 & char_mask[s.as_ref().symbol as usize] as u32,
                        );
                        s = s.offset(1);

                        if (count as i32) < 0 {
                            break;
                        }
                    }
                    s = s.offset(-1);
                    self.rc.decode_final(
                        hi_cnt
                            .wrapping_sub(count)
                            .wrapping_sub(s.as_ref().freq as u32),
                        s.as_ref().freq as u32,
                    )?;

                    let see = self.get_see(see_source);
                    see.update();
                    self.found_state = s;
                    let sym = s.as_ref().symbol;
                    self.update2();
                    return Ok(sym as i32);
                }

                if count >= freq_sum2 {
                    return Ok(SYM_ERROR);
                }

                self.rc.decode(hi_cnt, freq_sum2.wrapping_sub(hi_cnt));

                // We increase (see.summ) for sum of frequencies of all non_masked symbols.
                // new (see.summ) value can overflow over 16-bits in some rare cases.
                let see = self.get_see(see_source);
                see.summ = (see.summ as u32).wrapping_add(freq_sum) as u16;

                s = self.get_multi_state_stats(self.min_context);
                let s2 = s
                    .offset(self.min_context.as_ref().num_stats as i32 as isize)
                    .offset(1);
                loop {
                    char_mask[s.as_ref().symbol as usize] = 0;
                    s = s.offset(1);

                    if s >= s2 {
                        break;
                    }
                }
            }
        }
    }
}
