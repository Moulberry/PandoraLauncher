use alloc::{vec, vec::Vec};

use crate::{Read, error_invalid_data, error_other};

#[derive(Default)]
pub(crate) struct LzDecoder {
    buf: Vec<u8>,
    buf_size: usize,
    start: usize,
    pos: usize,
    full: usize,
    limit: usize,
    pending_len: usize,
    pending_dist: usize,
}

impl LzDecoder {
    pub(crate) fn new(dict_size: usize, preset_dict: Option<&[u8]>) -> Self {
        let mut buf = vec![0; dict_size];
        let mut pos = 0;
        let mut full = 0;
        let mut start = 0;
        if let Some(preset) = preset_dict {
            pos = preset.len().min(dict_size);
            full = pos;
            start = pos;
            let ps = preset.len() - pos;
            buf[0..pos].copy_from_slice(&preset[ps..]);
        }
        Self {
            buf,
            buf_size: dict_size,
            pos,
            full,
            start,
            ..Default::default()
        }
    }

    pub(crate) fn reset(&mut self) {
        self.start = 0;
        self.pos = 0;
        self.full = 0;
        self.limit = 0;
        self.buf[self.buf_size - 1] = 0;
    }

    pub(crate) fn set_limit(&mut self, out_max: usize) {
        self.limit = (out_max + self.pos).min(self.buf_size);
    }

    pub(crate) fn has_space(&self) -> bool {
        self.pos < self.limit
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.pending_len > 0
    }

    pub(crate) fn get_pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn get_byte(&self, dist: usize) -> u8 {
        let offset = if dist >= self.pos {
            self.buf_size
                .saturating_add(self.pos)
                .saturating_sub(dist)
                .saturating_sub(1)
        } else {
            self.pos.saturating_sub(dist).saturating_sub(1)
        };

        self.buf.get(offset).copied().unwrap_or(0)
    }

    pub(crate) fn put_byte(&mut self, b: u8) {
        self.buf[self.pos] = b;
        self.pos += 1;
        if self.full < self.pos {
            self.full = self.pos;
        }
    }

    pub(crate) fn repeat(&mut self, dist: usize, len: usize) -> crate::Result<()> {
        if dist >= self.full {
            return Err(error_other("dist overflow"));
        }
        let mut left = usize::min(self.limit - self.pos, len);
        self.pending_len = len - left;
        self.pending_dist = dist;

        let back = if self.pos < dist + 1 {
            // The distance wraps around to the end of the cyclic dictionary
            // buffer. We cannot get here if the dictionary isn't full.
            debug_assert_eq!(self.full, self.buf_size);
            let mut back = self.buf_size + self.pos - dist - 1;

            let copy_size = usize::min(self.buf_size - back, left);
            self.buf.copy_within(back..back + copy_size, self.pos);
            self.pos += copy_size;
            back = 0;
            left -= copy_size;

            if left == 0 {
                return Ok(());
            }

            back
        } else {
            self.pos - dist - 1
        };

        debug_assert!(back < self.pos);
        debug_assert!(left > 0);

        if dist >= left {
            // No overlap possible. We can copy directly.
            let (src_part, dst_part) = self.buf.split_at_mut(self.pos);
            dst_part[..left].copy_from_slice(&src_part[back..back + left]);
            self.pos += left;
        } else {
            loop {
                let copy_size = left.min(self.pos - back);
                self.buf.copy_within(back..back + copy_size, self.pos);
                self.pos += copy_size;
                left -= copy_size;
                if left == 0 {
                    break;
                }
            }
        }

        if self.full < self.pos {
            self.full = self.pos;
        }
        Ok(())
    }

    pub(crate) fn repeat_pending(&mut self) -> crate::Result<()> {
        if self.pending_len > 0 {
            self.repeat(self.pending_dist, self.pending_len)?;
        }
        Ok(())
    }

    pub(crate) fn copy_uncompressed<R: Read>(
        &mut self,
        mut in_data: R,
        len: usize,
    ) -> crate::Result<()> {
        let copy_size = (self.buf_size - self.pos).min(len);
        let buf = &mut self.buf[self.pos..(self.pos + copy_size)];
        in_data.read_exact(buf)?;
        self.pos += copy_size;
        if self.full < self.pos {
            self.full = self.pos;
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self, out: &mut [u8], out_off: usize) -> crate::Result<usize> {
        let copy_size = self.pos.saturating_sub(self.start);

        if self.pos == self.buf_size {
            self.pos = 0;
        }

        let src = self
            .buf
            .get(self.start..(self.start + copy_size))
            .ok_or(error_invalid_data("invalid source range"))?;

        let dst = out
            .get_mut(out_off..(out_off + copy_size))
            .ok_or(error_invalid_data("invalid destination range"))?;

        dst.copy_from_slice(src);

        self.start = self.pos;

        Ok(copy_size)
    }
}
