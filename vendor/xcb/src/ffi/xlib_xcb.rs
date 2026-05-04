/*
 * Copyright (C) 2013 James Miller <james@aatch.net>
 * Copyright (c) 2016
 *         Remi Thebault <remi.thebault@gmail.com>
 *         Thomas Bracht Laumann Jespersen <laumann.thomas@gmail.com>
 *
 * Permission is hereby granted, free of charge, to any
 * person obtaining a copy of this software and associated
 * documentation files (the "Software"), to deal in the
 * Software without restriction, including without
 * limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of
 * the Software, and to permit persons to whom the Software
 * is furnished to do so, subject to the following
 * conditions:
 *
 * The above copyright notice and this permission notice
 * shall be included in all copies or substantial portions
 * of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 * ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
 * TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
 * PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
 * SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
 * OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
 * IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]

use crate::ffi::xcb_connection_t;
use libc::c_uint;

#[cfg(all(feature = "xlib_xcb", not(feature = "xlib_xcb_dl")))]
use x11::xlib;

#[cfg(feature = "xlib_xcb_dl")]
use x11_dl::xlib;

/// Type for [XlibXcbLib::XSetEventQueueOwner] owner parameter
///
/// This item is behind the `xlib_xcb` cargo feature.
pub type XEventQueueOwner = c_uint;

/// Xlib owns the event queue
///
/// This item is behind the `xlib_xcb` cargo feature.
pub const XlibOwnsEventQueue: XEventQueueOwner = 0;

/// XCB owns the event queue
///
/// This item is behind the `xlib_xcb` cargo feature.
pub const XCBOwnsEventQueue: XEventQueueOwner = 1;

#[cfg(feature = "xlib_xcb_dl")]
use super::dl::define_api_dynamic as define_api;

#[cfg(not(feature = "xlib_xcb_dl"))]
use super::dl::define_api_link as define_api;

define_api! {
    /// Dynamically loaded X11-xcb library.
    pub XlibXcbLib XLIBXCBLIB_CACHE
    libs: ["libX11-xcb.so.1", "libX11-xcb.so"]
    link: "X11-xcb"

    functions:

    /// Get an XCB connection from the `xlib::Display`.
    ///
    /// This function is behind the `xlib_xcb`/`xlib_xcb_dl` cargo features.
    pub fn XGetXCBConnection(dpy: *mut xlib::Display) -> *mut xcb_connection_t;
    /// Set the owner of the X client event queue.
    ///
    /// This function is behind the `xlib_xcb`/`xlib_xcb_dl` cargo features.
    pub fn XSetEventQueueOwner(dpy: *mut xlib::Display, owner: XEventQueueOwner);
}
