use super::*;

impl<W: Write> PPMd7<RangeEncoder<W>> {
    pub(crate) fn encode_symbol(&mut self, symbol: i32) -> Result<(), std::io::Error> {
        unsafe {
            let mut char_mask: [u8; 256];

            if self.min_context.as_ref().num_stats != 1 {
                let mut s = self.get_multi_state_stats(self.min_context);

                self.rc.range /= self.min_context.as_ref().union2.summ_freq as u32;

                if s.as_ref().symbol as i32 == symbol {
                    self.rc.encode_final(0, s.as_ref().freq as u32)?;
                    self.found_state = s;
                    self.update1_0();
                    return Ok(());
                }
                self.prev_success = 0;

                let mut sum = s.as_ref().freq as u32;

                for _ in 1..(self.min_context.as_ref().num_stats as u32) {
                    s = s.offset(1);
                    if s.as_ref().symbol as i32 == symbol {
                        self.rc.encode_final(sum, s.as_ref().freq as u32)?;
                        self.found_state = s;
                        self.update1();
                        return Ok(());
                    }
                    sum += s.as_ref().freq as u32;
                }

                self.rc.encode(
                    sum,
                    (self.min_context.as_ref().union2.summ_freq as u32) - sum,
                );

                self.hi_bits_flag = Self::hi_bits_flag3(self.found_state.as_ref().symbol as u32);
                char_mask = [u8::MAX; 256];

                let s2 = self.get_multi_state_stats(self.min_context);
                Self::mask_symbols(&mut char_mask, s, s2);
            } else {
                let s = self.get_single_state(self.min_context);
                let range = self.rc.range;
                let prob = self.get_bin_summ();

                let mut pr = *prob as u32;
                let bound = (range >> 14) * pr;
                pr = ppmd_update_prob_1(pr);

                if s.as_ref().symbol as i32 == symbol {
                    *prob = (pr + (1 << PPMD_INT_BITS)) as u16;

                    self.rc.encode_bit_0(bound)?;
                    self.update_bin(s);

                    return Ok(());
                }
                *prob = pr as u16;
                self.init_esc = self.exp_escape[(pr >> 10) as usize] as u32;
                self.rc.encode_bit_1(bound)?;

                char_mask = [u8::MAX; 256];
                char_mask[s.as_ref().symbol as usize] = 0;
                self.prev_success = 0;
            }
            loop {
                self.rc.normalize_remote()?;

                let mut mc = self.min_context;
                let num_masked = mc.as_ref().num_stats as u32;

                let mut i;

                loop {
                    self.order_fall += 1;
                    if mc.as_ref().suffix.is_null() {
                        return Ok(());
                    }
                    mc = self.get_context(mc.as_ref().suffix);
                    i = mc.as_ref().num_stats as u32;

                    if i != num_masked {
                        break;
                    }
                }

                self.min_context = mc;

                let mut esc_freq = 0;
                let mut s = self.get_multi_state_stats(mc);
                let see_source = self.make_esc_freq(num_masked, &mut esc_freq);
                let mut sum = 0;

                while i != 0 {
                    let cur = s.as_ref().symbol;
                    if cur as i32 == symbol {
                        let low = sum;
                        let freq = s.as_ref().freq as u32;

                        let see = self.get_see(see_source);
                        see.update();
                        self.found_state = s;
                        sum += esc_freq;

                        let mut num2 = i / 2;
                        i &= 1;
                        sum += freq & 0u32.wrapping_sub(i);
                        if num2 != 0 {
                            s = s.offset(i as isize);

                            while num2 != 0 {
                                let sym0_0 = s.offset(0).as_ref().symbol as u32;
                                let sym1_0 = s.offset(1).as_ref().symbol as u32;
                                s = s.offset(2);
                                sum += (s.offset(-2).as_ref().freq & char_mask[sym0_0 as usize])
                                    as u32;
                                sum += (s.offset(-1).as_ref().freq & char_mask[sym1_0 as usize])
                                    as u32;
                                num2 -= 1;
                            }
                        }
                        self.rc.range /= sum;
                        self.rc.encode_final(low, freq)?;
                        self.update2();
                        return Ok(());
                    }
                    sum += s.as_ref().freq as u32 & char_mask[cur as usize] as u32;
                    s = s.offset(1);
                    i -= 1;
                }

                {
                    let total = sum + esc_freq;
                    let see = self.get_see(see_source);
                    see.summ = ((see.summ as u32) + total) as u16;

                    self.rc.range /= total;
                    self.rc.encode(sum, esc_freq);
                }

                let s2 = self.get_multi_state_stats(self.min_context);
                s = s.offset(-1);
                Self::mask_symbols(&mut char_mask, s, s2);
            }
        }
    }
}
