use crate::{
    std::ops::{Deref, DerefMut},
    Error,
};

use core::mem;

use sval_ref::ValueRef as _;

use self::raw::{
    into_value_parts, stream_parts, BinaryHeader, BorrowedBinary, BorrowedText, Encode, EnumHeader,
    MapHeader, MapKeyHeader, MapValueHeader, RecordHeader, RecordTupleHeader,
    RecordTupleValueHeader, RecordValueHeader, SeqHeader, SeqValueHeader, TagHintPart, TagPart,
    TaggedHeader, TextHeader, TupleHeader, TupleValueHeader, ValueBufParts, ValueParts,
};

#[cfg(feature = "alloc")]
use self::raw::make_owned;

#[cfg(not(feature = "alloc"))]
use self::raw::ArrayVec;

mod raw;

/**
Buffer arbitrary values into a tree-like structure.

This type requires the `alloc` or `std` features, otherwise most methods
will fail.
*/
#[derive(Debug)]
pub struct ValueBuf<'sval> {
    parts: ValueBufParts<'sval>,
    // A frame for each open container, for header patching on `end`
    stack: BufMut<Frame, STACK_CAP>,
    // The text or binary value currently being collected
    buffering: Buffering<'sval>,
    // An accumulated descriptive error during buffering
    // `sval`'s errors don't carry any information, so we track it here
    err: Option<Error>,
}

/**
An immutable buffered value.

This type is more compact than `ValueBuf`.
*/
#[derive(Debug, Clone)]
pub struct Value<'sval> {
    parts: ValueParts<'sval>,
}

impl<'sval> Default for ValueBuf<'sval> {
    fn default() -> Self {
        ValueBuf::new()
    }
}

impl<'sval> ValueBuf<'sval> {
    /**
    Create a new empty value buffer.
    */
    pub fn new() -> Self {
        ValueBuf {
            parts: ValueBufParts::new(),
            stack: Default::default(),
            buffering: Buffering::Value,
            err: None,
        }
    }

    /**
    Buffer a value.

    This method will fail if the `alloc` feature is not enabled.
    */
    pub fn collect(v: &'sval (impl sval::Value + ?Sized)) -> Result<Self, Error> {
        let mut buf = ValueBuf::new();

        match v.stream(&mut buf) {
            Ok(()) => Ok(buf),
            Err(_) => Err(buf
                .into_err()
                .unwrap_or_else(|| Error::invalid_value("the value itself failed to stream"))),
        }
    }

    /**
    Whether or not the contents of the value buffer are complete.
    */
    pub fn is_complete(&self) -> bool {
        self.stack.len() == 0 && self.parts.len() > 0 && matches!(self.buffering, Buffering::Value)
    }

    /**
    Clear this buffer so it can be re-used for future values.
    */
    pub fn clear(&mut self) {
        let ValueBuf {
            parts,
            stack,
            buffering,
            err,
        } = self;

        parts.clear();
        stack.clear();
        *buffering = Buffering::Value;
        *err = None;
    }

    /**
    Get an independent immutable value from this buffer.
    */
    pub fn to_value(&self) -> Value<'sval> {
        Value {
            parts: into_value_parts(self.parts.clone()),
        }
    }

    /**
    Convert this buffer into an immutable value.
    */
    pub fn into_value(self) -> Value<'sval> {
        Value {
            parts: into_value_parts(self.parts),
        }
    }

    /**
    Fully buffer any borrowed data, returning a buffer that doesn't borrow anything.

    This method will fail if the `alloc` feature is not enabled.
    */
    pub fn into_owned(self) -> Result<ValueBuf<'static>, Error> {
        #[cfg(feature = "alloc")]
        {
            let ValueBuf {
                parts, stack, err, ..
            } = self;

            Ok(ValueBuf {
                parts: make_owned(parts)?,
                stack,
                buffering: Buffering::Value,
                err,
            })
        }
        #[cfg(not(feature = "alloc"))]
        {
            Err(Error::no_alloc("owned value"))
        }
    }

    fn try_catch(
        &mut self,
        f: impl FnOnce(&mut ValueBuf<'sval>) -> Result<(), Error>,
    ) -> sval::Result {
        match f(self) {
            Ok(()) => Ok(()),
            Err(e) => self.fail(e),
        }
    }

    fn fail(&mut self, err: Error) -> sval::Result {
        self.err = Some(err);
        sval::error()
    }

    /**
    Take an error produced while attempting to buffer a value.

    This method may return `None` even if streaming failed if a value failed
    without ever calling into the buffer.
    */
    pub fn into_err(self) -> Option<Error> {
        self.err
    }
}

impl ValueBuf<'static> {
    /**
    Fully buffer a value, including any internal borrowed data.

    This method will fail if the `alloc` feature is not enabled.
    */
    pub fn collect_owned(v: impl sval::Value) -> Result<Self, Error> {
        let mut buf = ValueBuf::new();

        match sval::stream_computed(&mut buf, v) {
            Ok(()) => Ok(buf),
            Err(_) => Err(buf
                .into_err()
                .unwrap_or_else(|| Error::invalid_value("the value itself failed to stream"))),
        }
    }
}

impl<'sval> Value<'sval> {
    /**
    Buffer a value.

    This method will fail if the `alloc` feature is not enabled.
    */
    pub fn collect(v: &'sval (impl sval::Value + ?Sized)) -> Result<Self, Error> {
        ValueBuf::collect(v).map(|buf| buf.into_value())
    }

    /**
    Fully buffer this value, including any internal borrowed data.

    This method will fail if the `alloc` feature is not enabled.
    */
    pub fn into_owned(self) -> Result<Value<'static>, Error> {
        #[cfg(feature = "alloc")]
        {
            let Value { parts } = self;

            Ok(Value {
                parts: make_owned(parts)?,
            })
        }
        #[cfg(not(feature = "alloc"))]
        {
            Err(Error::no_alloc("owned value"))
        }
    }
}

impl Value<'static> {
    /**
    Fully buffer a value, including any internal borrowed data.

    This method will fail if the `alloc` feature is not enabled.
    */
    pub fn collect_owned(v: impl sval::Value) -> Result<Self, Error> {
        ValueBuf::collect_owned(v).map(|buf| buf.into_value())
    }
}

impl<'a> sval::Value for ValueBuf<'a> {
    fn stream<'sval, S: sval::Stream<'sval> + ?Sized>(&'sval self, stream: &mut S) -> sval::Result {
        self.stream_ref(stream)
    }
}

impl<'sval> sval_ref::ValueRef<'sval> for ValueBuf<'sval> {
    fn stream_ref<S: sval::Stream<'sval> + ?Sized>(&self, stream: &mut S) -> sval::Result {
        stream_parts(&self.parts, stream)
    }
}

impl<'a> sval::Value for Value<'a> {
    fn stream<'sval, S: sval::Stream<'sval> + ?Sized>(&'sval self, stream: &mut S) -> sval::Result {
        self.stream_ref(stream)
    }
}

impl<'sval> sval_ref::ValueRef<'sval> for Value<'sval> {
    fn stream_ref<S: sval::Stream<'sval> + ?Sized>(&self, stream: &mut S) -> sval::Result {
        stream_parts(&self.parts, stream)
    }
}

/**
Buffer a value.
*/
pub fn stream_to_value<'sval>(
    v: &'sval (impl sval::Value + ?Sized),
) -> Result<ValueBuf<'sval>, Error> {
    ValueBuf::collect(v)
}

/**
Buffer an owned value.
*/
pub fn stream_to_value_owned(v: impl sval::Value) -> Result<ValueBuf<'static>, Error> {
    ValueBuf::collect_owned(v)
}

// The maximum size to inline small borrowed text fragments
// This reduces the size of buffered values and makes conversion
// into owned cheaper, but changes borrowed fragments into computed ones
#[cfg(feature = "alloc")]
const MAX_INLINE_FRAGMENT_LEN: usize = 24;

#[derive(Debug, Clone, Copy)]
enum Buffering<'sval> {
    Value,
    Text(BufferingState<'sval, str>),
    Binary(BufferingState<'sval, [u8]>),
}

#[derive(Debug)]
enum BufferingState<'sval, T: ?Sized> {
    Empty,
    Borrowed(&'sval T),
    Inline { len_at: usize },
}

impl<'sval, T: ?Sized> Clone for BufferingState<'sval, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'sval, T: ?Sized> Copy for BufferingState<'sval, T> {}

impl<'sval> sval::Stream<'sval> for ValueBuf<'sval> {
    fn null(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(()))
    }

    fn bool(&mut self, value: bool) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn text_begin(&mut self, _: Option<usize>) -> sval::Result {
        self.try_catch(|buf| buf.buffering_begin(Buffering::Text(BufferingState::Empty)))
    }

    fn text_fragment(&mut self, fragment: &'sval str) -> sval::Result {
        self.try_catch(|buf| buf.buffering_push_text(fragment))
    }

    fn text_fragment_computed(&mut self, fragment: &str) -> sval::Result {
        self.try_catch(|buf| buf.encode_buffering_text_inline(fragment))
    }

    fn text_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_text_end())
    }

    fn binary_begin(&mut self, _: Option<usize>) -> sval::Result {
        self.try_catch(|buf| buf.buffering_begin(Buffering::Binary(BufferingState::Empty)))
    }

    fn binary_fragment(&mut self, fragment: &'sval [u8]) -> sval::Result {
        self.try_catch(|buf| buf.buffering_push_binary(fragment))
    }

    fn binary_fragment_computed(&mut self, fragment: &[u8]) -> sval::Result {
        self.try_catch(|buf| buf.encode_buffering_binary_inline(fragment))
    }

    fn binary_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_binary_end())
    }

    fn u8(&mut self, value: u8) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn u16(&mut self, value: u16) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn u32(&mut self, value: u32) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn u64(&mut self, value: u64) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn u128(&mut self, value: u128) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn i8(&mut self, value: i8) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn i16(&mut self, value: i16) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn i32(&mut self, value: i32) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn i64(&mut self, value: i64) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn i128(&mut self, value: i128) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn f32(&mut self, value: f32) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn f64(&mut self, value: f64) -> sval::Result {
        self.try_catch(|buf| buf.encode_value(value))
    }

    fn map_begin(&mut self, _num_entries_hint: Option<usize>) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(MapHeader {
                len: 0,
                num_entries: 0,
            })
        })
    }

    fn map_key_begin(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_entry_begin(MapKeyHeader { len: 0 }))
    }

    fn map_key_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn map_value_begin(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_begin(MapValueHeader { len: 0 }))
    }

    fn map_value_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn map_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn seq_begin(&mut self, _num_entries_hint: Option<usize>) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(SeqHeader {
                len: 0,
                num_entries: 0,
            })
        })
    }

    fn seq_value_begin(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_entry_begin(SeqValueHeader { len: 0 }))
    }

    fn seq_value_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn seq_end(&mut self) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn enum_begin(
        &mut self,
        tag: Option<&sval::Tag>,
        label: Option<&sval::Label>,
        index: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(EnumHeader {
                len: 0,
                tag: tag.cloned(),
                index: index.cloned(),
                label: label
                    .map(|label| {
                        label
                            .try_to_owned()
                            .map_err(|_| Error::no_alloc("owned label"))
                    })
                    .transpose()?,
            })
        })
    }

    fn enum_end(
        &mut self,
        _: Option<&sval::Tag>,
        _: Option<&sval::Label>,
        _: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn tagged_begin(
        &mut self,
        tag: Option<&sval::Tag>,
        label: Option<&sval::Label>,
        index: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(TaggedHeader {
                len: 0,
                tag: tag.cloned(),
                index: index.cloned(),
                label: label
                    .map(|label| {
                        label
                            .try_to_owned()
                            .map_err(|_| Error::no_alloc("owned label"))
                    })
                    .transpose()?,
            })
        })
    }

    fn tagged_end(
        &mut self,
        _: Option<&sval::Tag>,
        _: Option<&sval::Label>,
        _: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn tag(
        &mut self,
        tag: Option<&sval::Tag>,
        label: Option<&sval::Label>,
        index: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_value(TagPart {
                tag: tag.cloned(),
                index: index.cloned(),
                label: label
                    .map(|label| {
                        label
                            .try_to_owned()
                            .map_err(|_| Error::no_alloc("owned label"))
                    })
                    .transpose()?,
            })
        })
    }

    fn tag_hint(&mut self, tag: &sval::Tag) -> sval::Result {
        self.try_catch(|buf| buf.encode_value_discard(TagHintPart { tag: tag.clone() }))
    }

    fn record_begin(
        &mut self,
        tag: Option<&sval::Tag>,
        label: Option<&sval::Label>,
        index: Option<&sval::Index>,
        _num_entries: Option<usize>,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(RecordHeader {
                len: 0,
                tag: tag.cloned(),
                index: index.cloned(),
                label: label
                    .map(|label| {
                        label
                            .try_to_owned()
                            .map_err(|_| Error::no_alloc("owned label"))
                    })
                    .transpose()?,
                num_entries: 0,
            })
        })
    }

    fn record_value_begin(&mut self, tag: Option<&sval::Tag>, label: &sval::Label) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_entry_begin(RecordValueHeader {
                len: 0,
                tag: tag.cloned(),
                label: label
                    .try_to_owned()
                    .map_err(|_| Error::no_alloc("owned label"))?,
            })
        })
    }

    fn record_value_end(&mut self, _: Option<&sval::Tag>, _: &sval::Label) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn record_end(
        &mut self,
        _: Option<&sval::Tag>,
        _: Option<&sval::Label>,
        _: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn tuple_begin(
        &mut self,
        tag: Option<&sval::Tag>,
        label: Option<&sval::Label>,
        index: Option<&sval::Index>,
        _num_entries: Option<usize>,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(TupleHeader {
                len: 0,
                tag: tag.cloned(),
                index: index.cloned(),
                label: label
                    .map(|label| {
                        label
                            .try_to_owned()
                            .map_err(|_| Error::no_alloc("owned label"))
                    })
                    .transpose()?,
                num_entries: 0,
            })
        })
    }

    fn tuple_value_begin(&mut self, tag: Option<&sval::Tag>, index: &sval::Index) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_entry_begin(TupleValueHeader {
                len: 0,
                tag: tag.cloned(),
                index: index.clone(),
            })
        })
    }

    fn tuple_value_end(&mut self, _: Option<&sval::Tag>, _: &sval::Index) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn tuple_end(
        &mut self,
        _: Option<&sval::Tag>,
        _: Option<&sval::Label>,
        _: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn record_tuple_begin(
        &mut self,
        tag: Option<&sval::Tag>,
        label: Option<&sval::Label>,
        index: Option<&sval::Index>,
        _num_entries: Option<usize>,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_container_begin(RecordTupleHeader {
                len: 0,
                tag: tag.cloned(),
                index: index.cloned(),
                label: label
                    .map(|label| {
                        label
                            .try_to_owned()
                            .map_err(|_| Error::no_alloc("owned label"))
                    })
                    .transpose()?,
                num_entries: 0,
            })
        })
    }

    fn record_tuple_value_begin(
        &mut self,
        tag: Option<&sval::Tag>,
        label: &sval::Label,
        index: &sval::Index,
    ) -> sval::Result {
        self.try_catch(|buf| {
            buf.encode_entry_begin(RecordTupleValueHeader {
                len: 0,
                tag: tag.cloned(),
                label: label
                    .try_to_owned()
                    .map_err(|_| Error::no_alloc("owned label"))?,
                index: index.clone(),
            })
        })
    }

    fn record_tuple_value_end(
        &mut self,
        _: Option<&sval::Tag>,
        _: &sval::Label,
        _: &sval::Index,
    ) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }

    fn record_tuple_end(
        &mut self,
        _: Option<&sval::Tag>,
        _: Option<&sval::Label>,
        _: Option<&sval::Index>,
    ) -> sval::Result {
        self.try_catch(|buf| buf.encode_container_end())
    }
}

impl<'sval> ValueBuf<'sval> {
    /**
    Encode a part into the buffer, failing if it's not in a value context.
    */
    #[inline]
    fn encode_value<T: Encode<'sval>>(&mut self, header: T) -> Result<(), Error> {
        if !matches!(self.buffering, Buffering::Value) {
            return Err(Error::invalid_value(
                "attempt to buffer value instead of text or binary",
            ));
        }

        self.encode_value_always(header)
    }

    /**
    Encode a part into the buffer, discarding it if it's not in a value context.
    */
    #[inline]
    fn encode_value_discard<T: Encode<'sval>>(&mut self, header: T) -> Result<(), Error> {
        if !matches!(self.buffering, Buffering::Value) {
            // Discard rather than error when outside a value
            return Ok(());
        }

        self.encode_value_always(header)
    }

    /**
    Encode a part into the buffer regardless of whether its in a value context.
    */
    #[inline]
    fn encode_value_always<T: Encode<'sval>>(&mut self, header: T) -> Result<(), Error> {
        header.encode(&mut self.parts)
    }

    #[inline]
    fn encode_container_begin<T: Encode<'sval>>(&mut self, header: T) -> Result<(), Error> {
        let len_at = self.parts.next_len_at();

        // In no-std builds the stack's capacity is fixed; reserve the
        // frame's slot before encoding anything, so a full stack can't
        // leave a container header behind with no frame to patch it. With
        // `alloc`, `push` grows on demand and can't fail
        #[cfg(not(feature = "alloc"))]
        self.stack.reserve(1)?;

        self.encode_value(header)?;

        // `encode_part` performs validation that protects `stack`, so we
        // call it first; the slot reserved above means this can't fail
        self.stack.push(Frame::new::<T>(len_at))
    }

    #[inline]
    fn encode_entry_begin<T: Encode<'sval>>(&mut self, header: T) -> Result<(), Error> {
        self.encode_container_begin(header)?;

        // Count this entry in its parent's frame; the parent is the frame
        // just below the entry's own. The count is written into
        // entry-tracking headers when the parent ends
        let len = self.stack.len();
        if len >= 2 {
            self.stack[len - 2].count_entry();
        }

        Ok(())
    }

    #[inline(always)]
    fn encode_container_end(&mut self) -> Result<(), Error> {
        if !matches!(self.buffering, Buffering::Value) {
            return Err(Error::invalid_value(
                "attempt to buffer value instead text or binary",
            ));
        }

        let frame = self
            .stack
            .pop()
            .ok_or_else(|| Error::invalid_value("unbalanced calls to `begin` and `end`"))?;

        // The container's body starts after its header and runs to the
        // current end of the buffer
        let len = self.parts.len() - (frame.len_at() + frame.header_size());

        self.parts
            .patch_container_end(frame.len_at(), len, frame.num_entries());

        Ok(())
    }

    #[inline]
    fn buffering_begin(&mut self, buffering: Buffering<'sval>) -> Result<(), Error> {
        match self.buffering {
            Buffering::Value => {
                self.buffering = buffering;

                Ok(())
            }
            _ => Err(Error::invalid_value("already buffering")),
        }
    }

    #[inline]
    fn encode_buffering_text_borrowed(&mut self, v: &'sval str) -> Result<(), Error> {
        self.encode_value(BorrowedText { fragment: v })
    }

    #[inline]
    fn encode_buffering_binary_borrowed(&mut self, v: &'sval [u8]) -> Result<(), Error> {
        self.encode_value(BorrowedBinary { fragment: v })
    }

    #[inline]
    fn encode_buffering_text_inline(&mut self, fragment: &str) -> Result<(), Error> {
        match self.buffering {
            // We're already buffering text; append to the inline entry
            Buffering::Text(BufferingState::Inline { len_at }) => {
                // SAFETY: `len_at` is the length field of the text part
                // ending the buffer while state is `Inline`, and `fragment`
                // is UTF8
                unsafe { self.parts.push_raw_bytes(len_at, fragment.as_bytes()) }
            }
            // We've only seen borrowed text so far; we need to convert it into inline, then append
            Buffering::Text(BufferingState::Borrowed(prev)) => {
                let len_at = self.parts.next_len_at();
                self.encode_value_always(TextHeader { len: 0 })?;

                // SAFETY: `len_at` is the length field of the text part
                // just encoded at the end of the buffer, and `prev` and
                // `fragment` are UTF8
                unsafe {
                    self.parts.push_raw_bytes(len_at, prev.as_bytes())?;

                    self.buffering = Buffering::Text(BufferingState::Inline { len_at });

                    self.parts.push_raw_bytes(len_at, fragment.as_bytes())
                }
            }
            // We haven't seen any text yet; start buffering an inline entry
            Buffering::Text(BufferingState::Empty) => {
                let len_at = self.parts.next_len_at();
                self.encode_value_always(TextHeader { len: 0 })?;

                // SAFETY: `len_at` is the length field of the text part
                // just encoded at the end of the buffer, and `fragment` is UTF8
                unsafe { self.parts.push_raw_bytes(len_at, fragment.as_bytes()) }?;

                self.buffering = Buffering::Text(BufferingState::Inline { len_at });

                Ok(())
            }
            _ => Err(Error::outside_container("text")),
        }
    }

    #[inline]
    fn encode_buffering_binary_inline(&mut self, fragment: &[u8]) -> Result<(), Error> {
        match self.buffering {
            // We're already buffering binary; append to the inline entry
            Buffering::Binary(BufferingState::Inline { len_at }) => {
                // SAFETY: `len_at` is the length field of the binary part
                // ending the buffer while state is `Inline`
                unsafe { self.parts.push_raw_bytes(len_at, fragment) }
            }
            // We've only seen borrowed binary so far; we need to convert it into inline, then append
            Buffering::Binary(BufferingState::Borrowed(prev)) => {
                let len_at = self.parts.next_len_at();
                self.encode_value_always(BinaryHeader { len: 0 })?;

                // SAFETY: `len_at` is the length field of the binary part
                // just encoded at the end of the buffer
                unsafe {
                    self.parts.push_raw_bytes(len_at, prev)?;

                    self.buffering = Buffering::Binary(BufferingState::Inline { len_at });

                    self.parts.push_raw_bytes(len_at, fragment)
                }
            }
            // We haven't seen any binary yet; start buffering an inline entry
            Buffering::Binary(BufferingState::Empty) => {
                let len_at = self.parts.next_len_at();
                self.encode_value_always(BinaryHeader { len: 0 })?;

                // SAFETY: `len_at` is the length field of the binary part
                // just encoded at the end of the buffer
                unsafe { self.parts.push_raw_bytes(len_at, fragment) }?;

                self.buffering = Buffering::Binary(BufferingState::Inline { len_at });

                Ok(())
            }
            _ => Err(Error::outside_container("binary")),
        }
    }

    #[inline]
    fn encode_text_end(&mut self) -> Result<(), Error> {
        match mem::replace(&mut self.buffering, Buffering::Value) {
            // Empty text
            Buffering::Text(BufferingState::Empty) => self.encode_value(TextHeader { len: 0 }),
            // A single borrowed text fragment (this is the most common case)
            Buffering::Text(BufferingState::Borrowed(v)) => {
                // Copy small fragments into the buffer instead of borrowing
                // them: while nothing else is borrowed it keeps `into_owned`
                // free. Once something is borrowed anyway that benefit is
                // gone, so keep borrowing and skip the copies
                #[cfg(feature = "alloc")]
                if v.len() <= MAX_INLINE_FRAGMENT_LEN && !self.parts.is_borrowed() {
                    return self.encode_text_inline_copy(v);
                }

                self.encode_buffering_text_borrowed(v)
            }
            // A buffered text fragment
            Buffering::Text(BufferingState::Inline { .. }) => Ok(()),
            _ => Err(Error::outside_container("text")),
        }
    }

    #[inline]
    fn encode_binary_end(&mut self) -> Result<(), Error> {
        match mem::replace(&mut self.buffering, Buffering::Value) {
            // Empty binary
            Buffering::Binary(BufferingState::Empty) => self.encode_value(BinaryHeader { len: 0 }),
            // A single borrowed binary fragment (this is the most common case)
            Buffering::Binary(BufferingState::Borrowed(v)) => {
                // Copy small fragments into the buffer instead of borrowing
                // them; see `encode_text_end`
                #[cfg(feature = "alloc")]
                if v.len() <= MAX_INLINE_FRAGMENT_LEN && !self.parts.is_borrowed() {
                    return self.encode_binary_inline_copy(v);
                }

                self.encode_buffering_binary_borrowed(v)
            }
            // A buffered binary fragment
            Buffering::Binary(BufferingState::Inline { .. }) => Ok(()),
            _ => Err(Error::outside_container("binary")),
        }
    }

    #[cfg(feature = "alloc")]
    #[inline(never)]
    fn encode_text_inline_copy(&mut self, v: &str) -> Result<(), Error> {
        let len_at = self.parts.next_len_at();
        self.encode_value_always(TextHeader { len: 0 })?;

        // SAFETY: `len_at` is the length field of the text part just
        // encoded at the end of the buffer, and `v` is UTF8
        unsafe { self.parts.push_raw_bytes(len_at, v.as_bytes()) }
    }

    #[cfg(feature = "alloc")]
    #[inline(never)]
    fn encode_binary_inline_copy(&mut self, v: &[u8]) -> Result<(), Error> {
        let len_at = self.parts.next_len_at();
        self.encode_value_always(BinaryHeader { len: 0 })?;

        // SAFETY: `len_at` is the length field of the binary part just
        // encoded at the end of the buffer
        unsafe { self.parts.push_raw_bytes(len_at, v) }
    }

    #[inline]
    fn buffering_push_text(&mut self, fragment: &'sval str) -> Result<(), Error> {
        match self.buffering {
            // This is the first fragment; we'll keep it borrowed if it's the only one
            Buffering::Text(BufferingState::Empty) => {
                self.buffering = Buffering::Text(BufferingState::Borrowed(fragment));

                Ok(())
            }
            // We're already buffering; push the fragment
            Buffering::Text(_) => self.encode_buffering_text_inline(fragment),
            _ => Err(Error::outside_container("text")),
        }
    }

    #[inline]
    fn buffering_push_binary(&mut self, fragment: &'sval [u8]) -> Result<(), Error> {
        match self.buffering {
            // This is the first fragment; we'll keep it borrowed if it's the only one
            Buffering::Binary(BufferingState::Empty) => {
                self.buffering = Buffering::Binary(BufferingState::Borrowed(fragment));

                Ok(())
            }
            // We're already buffering; push the fragment
            Buffering::Binary(_) => self.encode_buffering_binary_inline(fragment),
            _ => Err(Error::outside_container("binary")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    // Offset of the container header's `len` field
    len_at: u32,
    // The number of direct entries seen so far
    entries: u32,
    // The size of the header's payload struct
    size: u8,
    // Whether the header tracks its number of entries
    entry_tracking: bool,
}

impl Frame {
    #[inline]
    fn new<'sval, T: Encode<'sval>>(len_at: usize) -> Self {
        debug_assert!(mem::size_of::<T>() <= u8::MAX as usize);
        debug_assert!(len_at <= u32::MAX as usize);

        Frame {
            len_at: len_at as u32,
            entries: 0,
            size: mem::size_of::<T>() as u8,
            entry_tracking: T::is_entry_tracking(),
        }
    }

    #[inline]
    fn len_at(self) -> usize {
        self.len_at as usize
    }

    #[inline]
    fn header_size(self) -> usize {
        self.size as usize
    }

    #[inline]
    fn num_entries(self) -> Option<u32> {
        if self.entry_tracking {
            Some(self.entries)
        } else {
            None
        }
    }

    #[inline]
    fn count_entry(&mut self) {
        self.entries += 1;
    }
}

// The maximum container nesting depth in no-std builds.
#[cfg(feature = "alloc")]
const STACK_CAP: usize = 1;

#[cfg(not(feature = "alloc"))]
const STACK_CAP: usize = 16;

#[derive(Debug)]
struct BufMut<T, const N: usize> {
    #[cfg(feature = "alloc")]
    inner: crate::std::vec::Vec<T>,
    #[cfg(not(feature = "alloc"))]
    inner: ArrayVec<T, N>,
}

impl<T, const N: usize> Default for BufMut<T, N> {
    fn default() -> Self {
        BufMut {
            inner: Default::default(),
        }
    }
}

impl<T, const N: usize> Deref for BufMut<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, const N: usize> DerefMut for BufMut<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T, const N: usize> BufMut<T, N> {
    #[cfg(not(feature = "alloc"))]
    #[inline]
    fn reserve(&mut self, extra: usize) -> Result<(), Error> {
        if self.inner.len() + extra > N {
            Err(Error::no_alloc("container frame"))
        } else {
            Ok(())
        }
    }

    fn push(&mut self, value: T) -> Result<(), Error> {
        #[cfg(feature = "alloc")]
        {
            self.inner.push(value);

            Ok(())
        }
        #[cfg(not(feature = "alloc"))]
        {
            self.inner.push(value)
        }
    }

    fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }

    fn clear(&mut self) {
        self.inner.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{value::raw::test_util::*, BinaryBuf, TextBuf};

    use sval::Stream as _;

    #[test]
    fn is_send_sync() {
        fn assert<T: Send + Sync>() {}

        assert::<ValueBuf>();
        assert::<Value>();
    }

    #[test]
    fn empty_is_complete() {
        assert!(!ValueBuf::new().is_complete());
    }

    #[test]
    fn primitive_is_complete() {
        assert!(ValueBuf::collect(&42).unwrap().is_complete());
    }

    #[test]
    fn text_is_complete() {
        let mut buf = ValueBuf::new();

        buf.text_begin(None).unwrap();

        assert!(!buf.is_complete());

        buf.text_end().unwrap();

        assert!(buf.is_complete());
    }

    #[test]
    fn binary_is_complete() {
        let mut buf = ValueBuf::new();

        buf.binary_begin(None).unwrap();

        assert!(!buf.is_complete());

        buf.binary_end().unwrap();

        assert!(buf.is_complete());
    }

    #[test]
    fn map_is_complete() {
        let mut buf = ValueBuf::new();

        buf.map_begin(None).unwrap();

        assert!(!buf.is_complete());

        buf.map_end().unwrap();

        assert!(buf.is_complete());
    }

    #[test]
    fn seq_is_complete() {
        let mut buf = ValueBuf::new();

        buf.seq_begin(None).unwrap();

        assert!(!buf.is_complete());

        buf.seq_end().unwrap();

        assert!(buf.is_complete());
    }

    #[test]
    fn empty() {
        use sval_test::{assert_tokens, Token::*};

        assert_tokens(&ValueBuf::new(), &[Null]);
    }

    #[test]
    fn buffer_primitive() {
        for (value, expected) in [
            (
                ValueBuf::collect(&true).unwrap(),
                vec![ValueKind::Bool(true)],
            ),
            (ValueBuf::collect(&1i8).unwrap(), vec![ValueKind::I8(1)]),
            (ValueBuf::collect(&2i16).unwrap(), vec![ValueKind::I16(2)]),
            (ValueBuf::collect(&3i32).unwrap(), vec![ValueKind::I32(3)]),
            (ValueBuf::collect(&4i64).unwrap(), vec![ValueKind::I64(4)]),
            (ValueBuf::collect(&5i128).unwrap(), vec![ValueKind::I128(5)]),
            (ValueBuf::collect(&1u8).unwrap(), vec![ValueKind::U8(1)]),
            (ValueBuf::collect(&2u16).unwrap(), vec![ValueKind::U16(2)]),
            (ValueBuf::collect(&3u32).unwrap(), vec![ValueKind::U32(3)]),
            (ValueBuf::collect(&4u64).unwrap(), vec![ValueKind::U64(4)]),
            (ValueBuf::collect(&5u128).unwrap(), vec![ValueKind::U128(5)]),
            (
                ValueBuf::collect(&3.14f32).unwrap(),
                vec![ValueKind::F32(3.14)],
            ),
            (
                ValueBuf::collect(&3.1415f64).unwrap(),
                vec![ValueKind::F64(3.1415)],
            ),
            (
                ValueBuf::collect("abc").unwrap(),
                vec![ValueKind::Text(TextBuf::from("abc"))],
            ),
            (
                ValueBuf::collect(sval::BinarySlice::new(b"abc")).unwrap(),
                vec![ValueKind::Binary(BinaryBuf::from(b"abc"))],
            ),
            (
                ValueBuf::collect(sval::MapSlice::<&str, i32>::new(&[])).unwrap(),
                vec![ValueKind::Map {
                    num_parts: 0,
                    num_entries: 0,
                }],
            ),
            (
                ValueBuf::collect(&[] as &[i32]).unwrap(),
                vec![ValueKind::Seq {
                    num_parts: 0,
                    num_entries: 0,
                }],
            ),
        ] {
            assert_eq!(expected, value.parts.decode(), "{:?}", value);
        }
    }

    #[test]
    fn buffer_empty_enum() {
        let mut buf = ValueBuf::new();

        buf.enum_begin(None, Some(&sval::Label::new("Enum")), None)
            .unwrap();
        buf.enum_end(None, Some(&sval::Label::new("Enum")), None)
            .unwrap();

        assert_eq!(
            vec![ValueKind::Enum {
                num_parts: 0,
                tag: None,
                label: Some(sval::Label::new("Enum")),
                index: None
            }],
            buf.parts.decode()
        );
    }

    #[test]
    fn buffer_empty_record() {
        let mut buf = ValueBuf::new();

        buf.record_begin(None, Some(&sval::Label::new("Record")), None, Some(0))
            .unwrap();
        buf.record_end(None, Some(&sval::Label::new("Record")), None)
            .unwrap();

        assert_eq!(
            vec![ValueKind::Record {
                num_parts: 0,
                tag: None,
                label: Some(sval::Label::new("Record")),
                index: None,
                num_entries: Some(0)
            }],
            buf.parts.decode()
        );
    }

    #[test]
    fn buffer_empty_tuple() {
        let mut buf = ValueBuf::new();

        buf.tuple_begin(None, Some(&sval::Label::new("Tuple")), None, Some(0))
            .unwrap();
        buf.tuple_end(None, Some(&sval::Label::new("Tuple")), None)
            .unwrap();

        assert_eq!(
            vec![ValueKind::Tuple {
                num_parts: 0,
                tag: None,
                label: Some(sval::Label::new("Tuple")),
                index: None,
                num_entries: Some(0)
            }],
            buf.parts.decode()
        );
    }

    #[test]
    fn buffer_reuse() {
        let mut buf = ValueBuf::new();

        buf.i32(42).unwrap();

        assert_eq!(
            Value::collect(&42i32).unwrap().parts.decode(),
            buf.to_value().parts.decode()
        );

        buf.clear();

        buf.bool(true).unwrap();

        assert_eq!(
            Value::collect(&true).unwrap().parts.decode(),
            buf.to_value().parts.decode()
        );
    }

    #[test]
    fn buffer_computed_text() {
        // Computed fragments are buffered inline, so they work even without
        // an allocator
        let mut buf = ValueBuf::new();

        buf.text_begin(None).unwrap();
        buf.text_fragment_computed("ab").unwrap();
        buf.text_fragment_computed("cd").unwrap();
        buf.text_end().unwrap();

        assert!(buf.is_complete());

        match buf.parts.decode()[0] {
            ValueKind::Text(ref text) => assert_eq!("abcd", text.as_str()),
            _ => unreachable!(),
        }
    }

    #[test]
    fn buffer_value_inside_text() {
        // A value streamed in the middle of a text string would corrupt the
        // encoding, so it must fail
        let mut buf = ValueBuf::new();

        buf.text_begin(None).unwrap();
        buf.text_fragment_computed("ab").unwrap();

        // Write some invalid data in the middle
        assert!(buf.bool(true).is_err());
    }

    #[test]
    fn buffer_invalid() {
        struct Kaboom;

        impl sval::Value for Kaboom {
            fn stream<'sval, S: sval::Stream<'sval> + ?Sized>(
                &'sval self,
                _: &mut S,
            ) -> sval::Result {
                sval::error()
            }
        }

        // Ensure we don't panic
        let _ = Value::collect(&Kaboom);
        let _ = Value::collect_owned(&Kaboom);
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod alloc_tests {
    use super::*;

    use crate::value::raw::test_util::*;

    use libstd::string::String;

    use sval::Stream as _;
    use sval_derive_macros::*;

    #[test]
    fn collect_owned() {
        let short_lived = String::from("abc");

        let buf = ValueBuf::collect_owned(&short_lived).unwrap();
        drop(short_lived);

        match buf.parts.decode()[0] {
            ValueKind::Text(ref text) => {
                assert!(text.as_borrowed_str().is_none());
                assert_eq!("abc", text.as_str());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn into_owned() {
        // Long enough that it's buffered as borrowed rather than inlined
        let short_lived = String::from("a value too long to buffer inline");

        let buf = ValueBuf::collect(&short_lived).unwrap();
        assert!(buf.parts.is_borrowed());

        let owned = buf.into_owned().unwrap();
        drop(short_lived);

        // The buffer is rebuilt into a single allocation of exactly the
        // required size
        assert_eq!(owned.parts.len(), owned.parts.capacity());

        match owned.parts.decode()[0] {
            ValueKind::Text(ref text) => {
                assert!(text.as_borrowed_str().is_none());
                assert_eq!("a value too long to buffer inline", text.as_str());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn small_borrowed_text_inlined() {
        // Small borrowed fragments are copied into the buffer up front, so
        // nothing borrows and `into_owned` has nothing to convert
        let short_lived = String::from("abc");

        let buf = ValueBuf::collect(&short_lived).unwrap();
        assert!(!buf.parts.is_borrowed());

        let owned = buf.into_owned().unwrap();
        drop(short_lived);

        match owned.parts.decode()[0] {
            ValueKind::Text(ref text) => {
                assert!(text.as_borrowed_str().is_none());
                assert_eq!("abc", text.as_str());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn small_borrowed_text_stays_borrowed_after_borrow() {
        // Once something is already borrowed, small fragments keep borrowing
        // too: `into_owned` will walk the buffer regardless, so there's
        // nothing to gain from copying them
        let long = String::from("a value too long to buffer inline");
        let small = String::from("abc");

        let value = (&long, &small);
        let buf = ValueBuf::collect(&value).unwrap();

        assert!(buf.parts.is_borrowed());

        // The tuple decodes flat: the small string is the second tuple
        // value's payload
        match buf.parts.decode()[4] {
            ValueKind::Text(ref text) => {
                assert!(text.as_borrowed_str().is_some());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn into_owned_binary_clone_and_drop() {
        // A borrowed binary value converted to owned in-place, then cloned so
        // two buffers own independent copies. Dropping both must not
        // double-free or leak (checked by miri).
        let short_lived = alloc::vec![1u8, 2, 3];

        let buf = ValueBuf::collect(sval::BinarySlice::new(&short_lived)).unwrap();

        let owned = buf.into_owned().unwrap();
        let cloned = owned.to_value();
        drop(short_lived);

        for decoded in [owned.parts.decode(), cloned.parts.decode()] {
            match decoded[0] {
                ValueKind::Binary(ref binary) => {
                    assert!(binary.as_borrowed_slice().is_none());
                    assert_eq!(&[1, 2, 3], binary.as_slice());
                }
                _ => unreachable!(),
            }
        }

        drop(cloned);
        drop(owned);
    }

    #[test]
    fn into_owned_nested() {
        // Borrowed text nested inside containers. Converting it to owned
        // changes the size of the text parts, so every enclosing container's
        // length has to be patched.
        #[derive(Value)]
        struct Nested<'a> {
            id: i32,
            title: &'a str,
            attributes: &'a [&'a str],
        }

        let title = String::from("A very important document");
        let attributes = [String::from("#1"), String::from("a longer attribute")];

        let data = Nested {
            id: 42,
            title: &title,
            attributes: &[&attributes[0], &attributes[1]],
        };

        // Fully-computed collection is already owned; converting borrowed
        // data must produce the same value
        let expected = ValueBuf::collect_owned(&data).unwrap();
        let owned = ValueBuf::collect(&data).unwrap().into_owned().unwrap();

        assert_eq!(expected.parts.decode(), owned.parts.decode());

        drop(title);
        drop(attributes);

        // The owned buffer must still stream correctly
        let roundtrip = ValueBuf::collect(&owned).unwrap();
        assert_eq!(owned.parts.decode(), roundtrip.parts.decode());
    }

    #[test]
    fn text_mixed_fragments() {
        // A borrowed fragment followed by more fragments is moved into a
        // single inline part
        let mut buf = ValueBuf::new();

        buf.text_begin(None).unwrap();
        buf.text_fragment("borrowed").unwrap();
        buf.text_fragment_computed(" computed ").unwrap();
        buf.text_fragment("borrowed").unwrap();
        buf.text_end().unwrap();

        match buf.parts.decode()[0] {
            ValueKind::Text(ref text) => {
                assert!(text.as_borrowed_str().is_none());
                assert_eq!("borrowed computed borrowed", text.as_str());
            }
            _ => unreachable!(),
        }

        // The buffer owns nothing that needs dropping or converting
        assert!(!buf.parts.is_owned());
        assert!(!buf.parts.is_borrowed());
    }

    #[test]
    fn incomplete_buffering_is_safe_to_walk() {
        // A value abandoned in the middle of a text string. The inline
        // part's length is patched on every append, so the buffer stays
        // well-formed and every walk over it (clone, drop with owned labels,
        // conversion, streaming) is sound even though the value is
        // incomplete
        let computed = String::from("computed");

        let mut buf = ValueBuf::new();
        buf.record_begin(
            None,
            Some(&sval::Label::new_computed(&computed)),
            None,
            None,
        )
        .unwrap();
        buf.record_value_begin(None, &sval::Label::new_computed(&computed))
            .unwrap();
        buf.text_begin(None).unwrap();
        buf.text_fragment_computed("nönsense \u{9F} bytes ÿ")
            .unwrap();

        assert!(!buf.is_complete());

        let value = buf.to_value();

        let mut tokens = sval_test::TokenBuf::new();
        value.stream_ref(&mut tokens).unwrap();

        let owned = value.into_owned().unwrap();

        drop(owned);
        drop(buf);
    }

    #[test]
    fn incomplete_buffering_reuse() {
        let mut buf = ValueBuf::new();

        buf.text_begin(None).unwrap();
        buf.text_fragment_computed("abandoned").unwrap();

        buf.clear();
        buf.i32(42).unwrap();

        assert!(buf.is_complete());
        assert_eq!(
            ValueBuf::collect(&42i32).unwrap().parts.decode(),
            buf.parts.decode()
        );
    }

    #[test]
    fn owned_clone_and_drop() {
        let short_lived = String::from("a computed string");
        let buf = ValueBuf::collect_owned(&short_lived).unwrap();
        drop(short_lived);

        let cloned = buf.to_value();

        assert_eq!(buf.parts.decode(), cloned.parts.decode());

        drop(cloned);
        drop(buf);
    }

    #[test]
    fn owned_label() {
        let short_lived = String::from("field");

        let mut buf = ValueBuf::new();
        buf.record_begin(None, Some(&sval::Label::new("R")), None, Some(1))
            .unwrap();
        buf.record_value_begin(None, &sval::Label::new_computed(&short_lived))
            .unwrap();
        buf.i32(1).unwrap();
        buf.record_value_end(None, &sval::Label::new_computed(&short_lived))
            .unwrap();
        buf.record_end(None, Some(&sval::Label::new("R")), None)
            .unwrap();

        let owned = buf.into_owned().unwrap();
        drop(short_lived);

        let roundtrip = ValueBuf::collect(&owned).unwrap();
        assert_eq!(owned.parts.decode(), roundtrip.parts.decode());
    }

    #[test]
    fn computed_text_multi_fragment() {
        let mut buf = ValueBuf::new();
        buf.text_begin(None).unwrap();
        buf.text_fragment_computed("ab").unwrap();
        buf.text_fragment_computed("cd").unwrap();
        buf.text_end().unwrap();

        match buf.parts.decode()[0] {
            ValueKind::Text(ref text) => {
                assert_eq!("abcd", text.as_str());
                assert!(text.as_borrowed_str().is_none());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn buffer_option() {
        let expected = vec![ValueKind::Tag {
            tag: Some(sval::tags::RUST_OPTION_NONE),
            label: Some(sval::Label::new("None")),
            index: Some(sval::Index::new(0)),
        }];

        assert_eq!(
            expected,
            ValueBuf::collect(&None::<i32>).unwrap().parts.decode()
        );

        let expected = vec![
            ValueKind::Tagged {
                num_parts: 1,
                tag: Some(sval::tags::RUST_OPTION_SOME),
                label: Some(sval::Label::new("Some")),
                index: Some(sval::Index::new(1)),
            },
            ValueKind::I32(42),
        ];

        assert_eq!(
            expected,
            ValueBuf::collect(&Some(42i32)).unwrap().parts.decode()
        );
    }

    #[test]
    fn buffer_map() {
        let mut value = ValueBuf::new();

        value.map_begin(Some(2)).unwrap();

        value.map_key_begin().unwrap();
        value.i32(0).unwrap();
        value.map_key_end().unwrap();

        value.map_value_begin().unwrap();
        value.bool(false).unwrap();
        value.map_value_end().unwrap();

        value.map_key_begin().unwrap();
        value.i32(1).unwrap();
        value.map_key_end().unwrap();

        value.map_value_begin().unwrap();
        value.bool(true).unwrap();
        value.map_value_end().unwrap();

        value.map_end().unwrap();

        let expected = vec![
            ValueKind::Map {
                num_parts: 8,
                num_entries: 2,
            },
            ValueKind::MapKey { num_parts: 1 },
            ValueKind::I32(0),
            ValueKind::MapValue { num_parts: 1 },
            ValueKind::Bool(false),
            ValueKind::MapKey { num_parts: 1 },
            ValueKind::I32(1),
            ValueKind::MapValue { num_parts: 1 },
            ValueKind::Bool(true),
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_seq() {
        let mut value = ValueBuf::new();

        value.seq_begin(Some(2)).unwrap();

        value.seq_value_begin().unwrap();
        value.bool(false).unwrap();
        value.seq_value_end().unwrap();

        value.seq_value_begin().unwrap();
        value.bool(true).unwrap();
        value.seq_value_end().unwrap();

        value.seq_end().unwrap();

        let expected = vec![
            ValueKind::Seq {
                num_parts: 4,
                num_entries: 2,
            },
            ValueKind::SeqValue { num_parts: 1 },
            ValueKind::Bool(false),
            ValueKind::SeqValue { num_parts: 1 },
            ValueKind::Bool(true),
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_record() {
        let mut value = ValueBuf::new();

        value
            .record_begin(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
                Some(2),
            )
            .unwrap();

        value
            .record_value_begin(None, &sval::Label::new("a"))
            .unwrap();
        value.bool(false).unwrap();
        value
            .record_value_end(None, &sval::Label::new("a"))
            .unwrap();

        value
            .record_value_begin(None, &sval::Label::new("b"))
            .unwrap();
        value.bool(true).unwrap();
        value
            .record_value_end(None, &sval::Label::new("b"))
            .unwrap();

        value
            .record_end(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
            )
            .unwrap();

        let expected = vec![
            ValueKind::Record {
                num_parts: 4,
                tag: Some(sval::Tag::new("test")),
                label: Some(sval::Label::new("A")),
                index: Some(sval::Index::new(1)),
                num_entries: Some(2),
            },
            ValueKind::RecordValue {
                num_parts: 1,
                tag: None,
                label: sval::Label::new("a"),
            },
            ValueKind::Bool(false),
            ValueKind::RecordValue {
                num_parts: 1,
                tag: None,
                label: sval::Label::new("b"),
            },
            ValueKind::Bool(true),
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_tuple() {
        let mut value = ValueBuf::new();

        value
            .tuple_begin(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
                Some(2),
            )
            .unwrap();

        value.tuple_value_begin(None, &sval::Index::new(0)).unwrap();
        value.bool(false).unwrap();
        value.tuple_value_end(None, &sval::Index::new(0)).unwrap();

        value.tuple_value_begin(None, &sval::Index::new(1)).unwrap();
        value.bool(true).unwrap();
        value.tuple_value_end(None, &sval::Index::new(1)).unwrap();

        value
            .tuple_end(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
            )
            .unwrap();

        let expected = vec![
            ValueKind::Tuple {
                num_parts: 4,
                tag: Some(sval::Tag::new("test")),
                label: Some(sval::Label::new("A")),
                index: Some(sval::Index::new(1)),
                num_entries: Some(2),
            },
            ValueKind::TupleValue {
                num_parts: 1,
                tag: None,
                index: sval::Index::new(0),
            },
            ValueKind::Bool(false),
            ValueKind::TupleValue {
                num_parts: 1,
                tag: None,
                index: sval::Index::new(1),
            },
            ValueKind::Bool(true),
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_record_tuple() {
        let mut value = ValueBuf::new();

        value
            .record_tuple_begin(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
                Some(2),
            )
            .unwrap();

        value
            .record_tuple_value_begin(None, &sval::Label::new("a"), &sval::Index::new(0))
            .unwrap();
        value.bool(false).unwrap();
        value
            .record_tuple_value_end(None, &sval::Label::new("a"), &sval::Index::new(0))
            .unwrap();

        value
            .record_tuple_value_begin(None, &sval::Label::new("b"), &sval::Index::new(1))
            .unwrap();
        value.bool(true).unwrap();
        value
            .record_tuple_value_end(None, &sval::Label::new("b"), &sval::Index::new(1))
            .unwrap();

        value
            .record_tuple_end(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
            )
            .unwrap();

        let expected = vec![
            ValueKind::RecordTuple {
                num_parts: 4,
                tag: Some(sval::Tag::new("test")),
                label: Some(sval::Label::new("A")),
                index: Some(sval::Index::new(1)),
                num_entries: Some(2),
            },
            ValueKind::RecordTupleValue {
                num_parts: 1,
                tag: None,
                label: sval::Label::new("a"),
                index: sval::Index::new(0),
            },
            ValueKind::Bool(false),
            ValueKind::RecordTupleValue {
                num_parts: 1,
                tag: None,
                label: sval::Label::new("b"),
                index: sval::Index::new(1),
            },
            ValueKind::Bool(true),
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_enum() {
        let mut value = ValueBuf::new();

        value
            .enum_begin(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
            )
            .unwrap();

        value
            .tag(
                None,
                Some(&sval::Label::new("B")),
                Some(&sval::Index::new(0)),
            )
            .unwrap();

        value
            .enum_end(
                Some(&sval::Tag::new("test")),
                Some(&sval::Label::new("A")),
                Some(&sval::Index::new(1)),
            )
            .unwrap();

        let expected = vec![
            ValueKind::Enum {
                num_parts: 1,
                tag: Some(sval::Tag::new("test")),
                label: Some(sval::Label::new("A")),
                index: Some(sval::Index::new(1)),
            },
            ValueKind::Tag {
                tag: None,
                label: Some(sval::Label::new("B")),
                index: Some(sval::Index::new(0)),
            },
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_tag_hints() {
        let mut value = ValueBuf::new();

        value.tag_hint(&sval::Tag::new("test")).unwrap();

        value.seq_begin(Some(2)).unwrap();

        value.seq_value_begin().unwrap();
        value.tag_hint(&sval::Tag::new("test")).unwrap();
        value.bool(false).unwrap();
        value.seq_value_end().unwrap();

        value.seq_value_begin().unwrap();
        value.bool(true).unwrap();
        value.seq_value_end().unwrap();

        value.seq_end().unwrap();

        value.tag_hint(&sval::Tag::new("test")).unwrap();

        let expected = vec![
            ValueKind::TagHint {
                tag: sval::Tag::new("test"),
            },
            ValueKind::Seq {
                num_parts: 5,
                num_entries: 2,
            },
            ValueKind::SeqValue { num_parts: 2 },
            ValueKind::TagHint {
                tag: sval::Tag::new("test"),
            },
            ValueKind::Bool(false),
            ValueKind::SeqValue { num_parts: 1 },
            ValueKind::Bool(true),
            ValueKind::TagHint {
                tag: sval::Tag::new("test"),
            },
        ];

        assert_eq!(expected, value.parts.decode());
    }

    #[test]
    fn buffer_roundtrip() {
        for value_1 in [
            ValueBuf::collect(&42i32).unwrap(),
            ValueBuf::collect(&vec![
                vec![],
                vec![vec![1, 2, 3], vec![4]],
                vec![vec![5, 6], vec![7, 8, 9]],
            ])
            .unwrap(),
            ValueBuf::collect(&{
                #[derive(Value)]
                struct Record {
                    a: i32,
                    b: bool,
                }

                Record { a: 42, b: true }
            })
            .unwrap(),
            ValueBuf::collect(&{
                #[derive(Value)]
                struct Tuple(i32, bool);

                Tuple(42, true)
            })
            .unwrap(),
            ValueBuf::collect(&{
                #[derive(Value)]
                enum Enum {
                    A,
                }

                Enum::A
            })
            .unwrap(),
        ] {
            let value_2 = ValueBuf::collect(&value_1).unwrap();

            assert_eq!(
                value_1.parts.decode(),
                value_2.parts.decode(),
                "{:?}",
                value_1
            );
        }
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    use libstd::{panic, string::String, vec::Vec};
    use rand::seq::IndexedRandom;
    use sval::Stream as _;

    #[derive(Debug, Clone, Copy)]
    enum Fault {
        None,
        Err,
        Panic,
    }

    impl Fault {
        const ALL: [Fault; 3] = [Fault::None, Fault::Err, Fault::Panic];

        fn apply(self, f: impl FnOnce() -> sval::Result) -> sval::Result {
            match self {
                Fault::None => f(),
                Fault::Err => sval::error(),
                Fault::Panic => panic!("explicit panic"),
            }
        }
    }

    type Op = (
        &'static str,
        fn(&mut ValueBuf<'static>, fault: Fault) -> sval::Result,
    );

    // Fragment contents deliberately include bytes that look like owned or
    // invalid part tags if a walk misreads them as part boundaries
    const TEXT_FRAGMENT: &str = "nönsense \u{9F} bytes ÿ";
    const BINARY_FRAGMENT: &[u8] = &[0xFF, 0x9F, 0xC0, 1, 2, 3];

    fn test_tag() -> sval::Tag {
        sval::Tag::new("test")
    }

    fn test_index() -> sval::Index {
        sval::Index::new(1)
    }

    fn computed(storage: &String) -> sval::Label<'_> {
        sval::Label::new_computed(storage)
    }

    const BUFFERING_OPS: usize = 8;

    const OPS: &[Op] = &[
        ("text_begin", |buf, fault| {
            fault.apply(|| buf.text_begin(Some(4)))
        }),
        ("text_fragment", |buf, fault| {
            fault.apply(|| buf.text_fragment(TEXT_FRAGMENT))
        }),
        ("text_fragment_computed", |buf, fault| {
            fault.apply(|| {
                let storage = String::from(TEXT_FRAGMENT);
                let r = buf.text_fragment_computed(&storage);
                r
            })
        }),
        ("text_end", |buf, fault| fault.apply(|| buf.text_end())),
        ("binary_begin", |buf, fault| {
            fault.apply(|| buf.binary_begin(Some(4)))
        }),
        ("binary_fragment", |buf, fault| {
            fault.apply(|| buf.binary_fragment(BINARY_FRAGMENT))
        }),
        ("binary_fragment_computed", |buf, fault| {
            fault.apply(|| {
                let storage = Vec::from(BINARY_FRAGMENT);
                let r = buf.binary_fragment_computed(&storage);
                r
            })
        }),
        ("binary_end", |buf, fault| fault.apply(|| buf.binary_end())),
        ("null", |buf, fault| fault.apply(|| buf.null())),
        ("bool", |buf, fault| fault.apply(|| buf.bool(true))),
        ("u8", |buf, fault| fault.apply(|| buf.u8(1))),
        ("u16", |buf, fault| fault.apply(|| buf.u16(2))),
        ("u32", |buf, fault| fault.apply(|| buf.u32(3))),
        ("u64", |buf, fault| fault.apply(|| buf.u64(4))),
        ("u128", |buf, fault| fault.apply(|| buf.u128(5))),
        ("i8", |buf, fault| fault.apply(|| buf.i8(-1))),
        ("i16", |buf, fault| fault.apply(|| buf.i16(-2))),
        ("i32", |buf, fault| fault.apply(|| buf.i32(-3))),
        ("i64", |buf, fault| fault.apply(|| buf.i64(-4))),
        ("i128", |buf, fault| fault.apply(|| buf.i128(-5))),
        ("f32", |buf, fault| fault.apply(|| buf.f32(3.14))),
        ("f64", |buf, fault| fault.apply(|| buf.f64(3.1415))),
        ("map_begin", |buf, fault| {
            fault.apply(|| buf.map_begin(Some(1)))
        }),
        ("map_key_begin", |buf, fault| {
            fault.apply(|| buf.map_key_begin())
        }),
        ("map_key_end", |buf, fault| {
            fault.apply(|| buf.map_key_end())
        }),
        ("map_value_begin", |buf, fault| {
            fault.apply(|| buf.map_value_begin())
        }),
        ("map_value_end", |buf, fault| {
            fault.apply(|| buf.map_value_end())
        }),
        ("map_end", |buf, fault| fault.apply(|| buf.map_end())),
        ("seq_begin", |buf, fault| {
            fault.apply(|| buf.seq_begin(Some(1)))
        }),
        ("seq_value_begin", |buf, fault| {
            fault.apply(|| buf.seq_value_begin())
        }),
        ("seq_value_end", |buf, fault| {
            fault.apply(|| buf.seq_value_end())
        }),
        ("seq_end", |buf, fault| fault.apply(|| buf.seq_end())),
        ("enum_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.enum_begin(
                    Some(&test_tag()),
                    Some(&computed(&storage)),
                    Some(&test_index()),
                );
                r
            })
        }),
        ("enum_end", |buf, fault| {
            fault.apply(|| buf.enum_end(None, Some(&sval::Label::new("label")), None))
        }),
        ("tagged_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.tagged_begin(
                    Some(&test_tag()),
                    Some(&computed(&storage)),
                    Some(&test_index()),
                );
                r
            })
        }),
        ("tagged_end", |buf, fault| {
            fault.apply(|| buf.tagged_end(None, Some(&sval::Label::new("label")), None))
        }),
        ("tag", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.tag(
                    Some(&test_tag()),
                    Some(&computed(&storage)),
                    Some(&test_index()),
                );
                r
            })
        }),
        ("tag_hint", |buf, fault| {
            fault.apply(|| buf.tag_hint(&test_tag()))
        }),
        ("record_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.record_begin(
                    Some(&test_tag()),
                    Some(&computed(&storage)),
                    Some(&test_index()),
                    Some(1),
                );
                r
            })
        }),
        ("record_value_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.record_value_begin(Some(&test_tag()), &computed(&storage));
                r
            })
        }),
        ("record_value_end", |buf, fault| {
            fault.apply(|| buf.record_value_end(None, &sval::Label::new("label")))
        }),
        ("record_end", |buf, fault| {
            fault.apply(|| buf.record_end(None, Some(&sval::Label::new("label")), None))
        }),
        ("tuple_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.tuple_begin(
                    Some(&test_tag()),
                    Some(&computed(&storage)),
                    Some(&test_index()),
                    Some(1),
                );
                r
            })
        }),
        ("tuple_value_begin", |buf, fault| {
            fault.apply(|| buf.tuple_value_begin(Some(&test_tag()), &test_index()))
        }),
        ("tuple_value_end", |buf, fault| {
            fault.apply(|| buf.tuple_value_end(None, &test_index()))
        }),
        ("tuple_end", |buf, fault| {
            fault.apply(|| buf.tuple_end(None, Some(&sval::Label::new("label")), None))
        }),
        ("record_tuple_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.record_tuple_begin(
                    Some(&test_tag()),
                    Some(&computed(&storage)),
                    Some(&test_index()),
                    Some(1),
                );
                r
            })
        }),
        ("record_tuple_value_begin", |buf, fault| {
            fault.apply(|| {
                let storage = String::from("owned-label");
                let r = buf.record_tuple_value_begin(
                    Some(&test_tag()),
                    &computed(&storage),
                    &test_index(),
                );
                r
            })
        }),
        ("record_tuple_value_end", |buf, fault| {
            fault.apply(|| {
                buf.record_tuple_value_end(None, &sval::Label::new("label"), &test_index())
            })
        }),
        ("record_tuple_end", |buf, fault| {
            fault.apply(|| buf.record_tuple_end(None, Some(&sval::Label::new("label")), None))
        }),
    ];

    fn exercise(mut buf: ValueBuf<'static>, desc: impl Fn() -> String) {
        // Clone the buffer, deep-cloning any owned payloads
        let value = buf.to_value();

        let _ = value.parts.decode();

        {
            let _ = ValueBuf::collect(&value);
        }

        if let Ok(owned) = value.into_owned() {
            let _ = ValueBuf::collect(&owned);
        }

        buf.clear();
        buf.i32(42).unwrap();
        assert!(buf.is_complete(), "buffer unusable after: {}", desc());
    }

    fn exhaustive(ops: &[Op], max_len: usize) {
        let mut sequences = 0usize;

        for len in 0..=max_len {
            let mut seq = alloc::vec![0usize; len];

            'sequences: loop {
                let mut buf = ValueBuf::new();

                for &op in seq.iter() {
                    let _ = (ops[op].1)(&mut buf, Fault::None);
                }

                exercise(buf, || {
                    seq.iter()
                        .map(|&op| ops[op].0)
                        .collect::<Vec<_>>()
                        .join(" -> ")
                });

                sequences += 1;

                // Advance to the next sequence, in mixed-radix order
                for slot in seq.iter_mut() {
                    *slot += 1;

                    if *slot < ops.len() {
                        continue 'sequences;
                    }

                    *slot = 0;
                }

                break;
            }
        }

        // Every sequence of every length must actually have run
        let expected = (0..=max_len as u32)
            .map(|len| ops.len().pow(len))
            .sum::<usize>();
        assert_eq!(expected, sequences);
    }

    #[test]
    fn exhaustive_stream_calls() {
        // Every combination of stream calls up to a fixed length
        let max_len = if cfg!(miri) { 2 } else { 3 };

        exhaustive(OPS, max_len);
    }

    #[test]
    fn exhaustive_buffering_calls() {
        // Longer combinations of just the text and binary calls, which are
        // the only ones that hold buffering state between calls
        let max_len = if cfg!(miri) { 3 } else { 5 };

        exhaustive(&OPS[..BUFFERING_OPS], max_len);
    }

    #[test]
    fn abandoned_prefixes() {
        for prefix in 0..=OPS.len() {
            let mut buf = ValueBuf::new();

            for (name, apply) in &OPS[..prefix] {
                // The full sequence is valid, so with an allocator every call
                // must succeed. Without one it can hit the fixed capacity and
                // error, which must still leave the buffer safe to walk.
                if cfg!(feature = "alloc") {
                    apply(&mut buf, Fault::None).unwrap_or_else(|_| panic!("{} failed", name));
                } else {
                    let _ = apply(&mut buf, Fault::None);
                }
            }

            exercise(buf, || {
                OPS[..prefix]
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            });
        }
    }

    #[test]
    fn scrambled() {
        // Quick smoke test to weed out UB from panicking/erroring stream calls
        for _ in 0..500 {
            let mut buf = ValueBuf::new();

            for _ in 0..rand::random_range(0..10) {
                let fault = Fault::ALL.choose(&mut rand::rng()).unwrap();
                let (_, op) = OPS.choose(&mut rand::rng()).unwrap();

                let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| op(&mut buf, *fault)));
            }

            // Attempt to use the buffer
            let _ = ValueBuf::collect(&buf);
        }
    }
}
