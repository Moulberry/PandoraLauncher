// OpenEXR's bit-count based half-float quantizer (`quantize` in
// internal_dwa_helpers.h): `algo_quantize` searches nearby half
// representations and keeps the one with the fewest set bits that still stays
// within the tolerated error. All the `handle_quantize_*` cases and their
// helpers are internal to this search and never used elsewhere.

use half::f16;

pub(super) fn algo_quantize(src: u32, herr_tol: u32, err_tol: f32, src_float: f32) -> u32 {
    // Port of OpenEXR's bit-count based half-float quantizer.
    // It searches nearby representations and keeps the one with the fewest
    // set bits while staying within the tolerated error.
    let sign = src & 0x8000;
    let abssrc = src & 0x7fff;
    let src_float = src_float.abs();

    let src_exp_biased = src & 0x7c00;
    let tol_exp_biased = herr_tol & 0x7c00;

    if src_exp_biased == 0x7c00 {
        return src;
    }

    if src_float < err_tol {
        return 0;
    }

    let exp_diff = src_exp_biased.wrapping_sub(tol_exp_biased) >> 10;
    let mut tol_sig = shift_right((herr_tol & 0x03ff) | (1 << 10), exp_diff);

    if tol_exp_biased == 0 {
        if exp_diff == 0 || exp_diff == 1 {
            tol_sig = herr_tol & 0x03ff;
            if tol_sig == 0 {
                return src;
            }
            return sign | handle_quantize_generic(abssrc, tol_sig, err_tol, src_float);
        }

        tol_sig = herr_tol & 0x03ff;
        if tol_sig == 0 {
            return src;
        }

        tol_sig = shift_right(tol_sig, exp_diff);
        if tol_sig == 0 {
            tol_sig = 1;
        }

        return sign | handle_quantize_denorm_tol(abssrc, tol_sig, err_tol, src_float);
    }

    if tol_sig == 0 {
        return src;
    }

    if exp_diff > 1 || src_exp_biased == 0 {
        return sign | handle_quantize_default(abssrc, tol_sig, err_tol, src_float);
    }

    if exp_diff == 0 {
        return sign | handle_quantize_equal_exp(abssrc, tol_sig, err_tol, src_float);
    }
    sign | handle_quantize_close_exp(abssrc, tol_sig, err_tol, src_float)
}

fn shift_right(value: u32, shift: u32) -> u32 {
    if shift >= 32 {
        0
    } else {
        value >> shift
    }
}

fn half_to_f32(bits: u32) -> f32 {
    f16::from_bits(bits as u16).to_f32()
}

fn test_quant_alternate_large(
    alt: u32,
    smallest: &mut u32,
    smallbits: &mut u32,
    smalldelta: &mut f32,
    err_tol: f32,
    src_float: f32,
) {
    let bits = alt.count_ones();
    if bits < *smallbits {
        let delta = half_to_f32(alt) - src_float;
        if delta < err_tol {
            *smallbits = bits;
            *smalldelta = delta;
            *smallest = alt;
        }
    } else if bits == *smallbits {
        let delta = half_to_f32(alt) - src_float;
        if delta < *smalldelta {
            *smallest = alt;
            *smalldelta = delta;
            *smallbits = bits;
        }
    }
}

fn test_quant_alternate_small(
    alt: u32,
    smallest: &mut u32,
    smallbits: &mut u32,
    smalldelta: &mut f32,
    err_tol: f32,
    src_float: f32,
) {
    let bits = alt.count_ones();
    if bits < *smallbits {
        let delta = src_float - half_to_f32(alt);
        if delta < err_tol {
            *smallbits = bits;
            *smalldelta = delta;
            *smallest = alt;
        }
    } else if bits == *smallbits {
        let delta = src_float - half_to_f32(alt);
        if delta < *smalldelta {
            *smallest = alt;
            *smalldelta = delta;
            *smallbits = bits;
        }
    }
}

fn quant_mask(tol_sig: u32) -> (u32, u32, u32, u32) {
    let tsigshift = 32 - tol_sig.leading_zeros();
    let npow2 = 1u32 << tsigshift;
    let lowermask = npow2 - 1;
    let mask = !lowermask;
    let mask2 = mask ^ npow2;
    (npow2, lowermask, mask, mask2)
}

fn handle_quantize_denorm_tol(abssrc: u32, tol_sig: u32, err_tol: f32, src_float: f32) -> u32 {
    let (npow2, _, mask, mask2) = quant_mask(tol_sig);
    let mut smallest = abssrc;
    let mut smallbits = abssrc.count_ones();
    let mut smalldelta = err_tol;

    test_quant_alternate_small(
        abssrc & mask2,
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );
    test_quant_alternate_small(
        abssrc & mask,
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );
    test_quant_alternate_large(
        (abssrc + npow2) & mask,
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );
    test_quant_alternate_large(
        (abssrc + (npow2 << 1)) & mask,
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );

    smallest
}

fn handle_quantize_generic(abssrc: u32, tol_sig: u32, err_tol: f32, src_float: f32) -> u32 {
    let (npow2, lowermask, mask, mask2) = quant_mask(tol_sig);
    let src_masked_val = abssrc & lowermask;
    let extrabit = u32::from(tol_sig > src_masked_val);
    let mask3 = mask2 ^ (((npow2 << 1) * extrabit) | ((npow2 >> 1) * (1 - extrabit)));

    let mut smallest = abssrc;
    let mut smallbits = abssrc.count_ones();
    let mut smalldelta = err_tol;

    if extrabit != 0 {
        test_quant_alternate_small(
            abssrc & mask3,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            abssrc & mask2,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            abssrc & mask,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
    } else if (abssrc & npow2) != 0 {
        test_quant_alternate_small(
            abssrc & mask2,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            abssrc & mask3,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            abssrc & mask,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
    } else {
        test_quant_alternate_small(
            abssrc & mask2,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            abssrc & mask,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            abssrc & mask3,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
    }
    test_quant_alternate_large(
        (abssrc + npow2) & mask,
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );

    smallest
}

fn handle_quantize_equal_exp(abssrc: u32, tol_sig: u32, err_tol: f32, src_float: f32) -> u32 {
    let npow2 = 0x0800;
    let lowermask = npow2 - 1;
    let mask = !lowermask;
    let mask2 = mask ^ npow2;
    let src_masked_val = abssrc & lowermask;
    let extrabit = u32::from(tol_sig > src_masked_val);
    let mask3 = mask2 ^ (((npow2 << 1) * extrabit) | ((npow2 >> 1) * (1 - extrabit)));

    let mut smallest = abssrc;
    let mut smallbits = abssrc.count_ones();
    let mut smalldelta = err_tol;

    if src_masked_val == abssrc {
        test_quant_alternate_small(
            abssrc & mask3,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
    } else {
        let mut alt0 = abssrc & mask2;
        let alt1 = abssrc & mask;
        if alt0 == alt1 {
            alt0 = abssrc & mask3;
        }
        test_quant_alternate_small(
            alt0,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
        test_quant_alternate_small(
            alt1,
            &mut smallest,
            &mut smallbits,
            &mut smalldelta,
            err_tol,
            src_float,
        );
    }
    test_quant_alternate_large(
        (abssrc + npow2) & mask,
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );

    smallest
}

fn handle_quantize_close_exp(abssrc: u32, tol_sig: u32, err_tol: f32, src_float: f32) -> u32 {
    let npow2 = 0x0400;
    let lowermask = npow2 - 1;
    let mask = !lowermask;
    let mask2 = mask ^ npow2;
    let src_masked_val = abssrc & lowermask;
    let extrabit = u32::from(tol_sig > src_masked_val);
    let mask3 = mask2 ^ (((npow2 << 1) * extrabit) | ((npow2 >> 1) * (1 - extrabit)));

    let mut alternates = [0u32; 3];
    if (abssrc & npow2) == 0 {
        if extrabit != 0 {
            alternates[0] = abssrc & mask3;
            alternates[1] = abssrc & mask;
        } else {
            alternates[0] = abssrc & mask;
            alternates[1] = abssrc & mask3;
        }
    } else if extrabit != 0 {
        alternates[0] = abssrc & mask3;
        alternates[1] = abssrc & mask2;
        let alt1delta = src_float - half_to_f32(alternates[1]);
        if alt1delta >= err_tol {
            alternates[1] = abssrc & mask;
        }
    } else {
        alternates[0] = abssrc & mask2;
        alternates[1] = abssrc & mask3;
        let alt0delta = src_float - half_to_f32(alternates[0]);
        if alt0delta >= err_tol {
            alternates[0] = abssrc & mask;
        }
    }
    alternates[2] = (abssrc + npow2) & mask;

    let mut smallest = abssrc;
    let mut smallbits = abssrc.count_ones();
    let mut smalldelta = err_tol;

    test_quant_alternate_small(
        alternates[0],
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );
    test_quant_alternate_small(
        alternates[1],
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );
    test_quant_alternate_large(
        alternates[2],
        &mut smallest,
        &mut smallbits,
        &mut smalldelta,
        err_tol,
        src_float,
    );

    smallest
}

fn handle_quantize_larger_sig(
    abssrc: u32,
    npow2: u32,
    mask: u32,
    err_tol: f32,
    src_float: f32,
) -> u32 {
    let mask2 = mask ^ (npow2 | (npow2 >> 1));
    let alt0 = abssrc & mask2;
    let alt1 = (abssrc + npow2) & mask;
    choose_two_sided_alternate(abssrc, alt0, alt1, err_tol, src_float)
}

fn handle_quantize_smaller_sig(
    abssrc: u32,
    npow2: u32,
    mask: u32,
    err_tol: f32,
    src_float: f32,
) -> u32 {
    let alt0 = abssrc & mask;
    let alt1 = (abssrc + npow2) & mask;
    choose_two_sided_alternate(abssrc, alt0, alt1, err_tol, src_float)
}

fn choose_two_sided_alternate(
    abssrc: u32,
    alt0: u32,
    alt1: u32,
    err_tol: f32,
    src_float: f32,
) -> u32 {
    let bits0 = alt0.count_ones();
    let bits1 = alt1.count_ones();

    if bits1 < bits0 {
        let delta = half_to_f32(alt1) - src_float;
        if delta < err_tol {
            return alt1;
        }
        let delta = src_float - half_to_f32(alt0);
        if delta < err_tol {
            return alt0;
        }
    } else if bits1 == bits0 {
        let delta = src_float - half_to_f32(alt0);
        let delta1 = half_to_f32(alt1) - src_float;
        if delta < err_tol {
            return if delta1 < delta {
                alt1
            } else {
                alt0
            };
        }
        if delta1 < err_tol {
            return alt1;
        }
    } else {
        let delta = src_float - half_to_f32(alt0);
        if delta < err_tol {
            return alt0;
        }

        if bits1 < abssrc.count_ones() {
            let delta = half_to_f32(alt1) - src_float;
            if delta < err_tol {
                return alt1;
            }
        }
    }

    abssrc
}

fn handle_quantize_equal_sig(
    abssrc: u32,
    npow2: u32,
    mask: u32,
    err_tol: f32,
    src_float: f32,
) -> u32 {
    let mut alt0 = abssrc & mask;
    let alt1 = (abssrc + npow2) & mask;
    let mut delta0 = src_float - half_to_f32(alt0);

    if delta0 >= err_tol {
        let mask2 = mask ^ (npow2 | (npow2 >> 1));
        alt0 = abssrc & mask2;
        delta0 = src_float - half_to_f32(alt0);

        if delta0 >= err_tol {
            let delta1 = half_to_f32(alt1) - src_float;
            if delta1 < err_tol && alt1.count_ones() < abssrc.count_ones() {
                return alt1;
            }
            return abssrc;
        }
    }

    let bits0 = alt0.count_ones();
    let bits1 = alt1.count_ones();

    if bits1 < bits0 {
        let delta1 = half_to_f32(alt1) - src_float;
        if delta1 < err_tol {
            return alt1;
        }
    } else if bits1 == bits0 {
        let delta1 = half_to_f32(alt1) - src_float;
        if delta1 < delta0 {
            return alt1;
        }
    }

    alt0
}

fn handle_quantize_default(abssrc: u32, tol_sig: u32, err_tol: f32, src_float: f32) -> u32 {
    let (npow2, lowermask, mask, _) = quant_mask(tol_sig);
    let src_masked_val = abssrc & lowermask;

    if src_masked_val > tol_sig {
        handle_quantize_larger_sig(abssrc, npow2, mask, err_tol, src_float)
    } else if src_masked_val < tol_sig {
        handle_quantize_smaller_sig(abssrc, npow2, mask, err_tol, src_float)
    } else {
        handle_quantize_equal_sig(abssrc, npow2, mask, err_tol, src_float)
    }
}
