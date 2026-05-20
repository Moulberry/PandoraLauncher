use super::ext::*;
use libc::{c_char, c_int, c_uint, c_void, iovec};

/// Current protocol version
pub const X_PROTOCOL: u32 = 11;

/// Current minor version
pub const X_PROTOCOL_REVISION: u32 = 0;

/// X_TCP_PORT + display number = server port for TCP transport
pub const X_TCP_PORT: u32 = 6000;

/// xcb connection errors because of socket, pipe and other stream errors.
pub const XCB_CONN_ERROR: i32 = 1;

/// xcb connection shutdown because of extension not supported
pub const XCB_CONN_CLOSED_EXT_NOTSUPPORTED: i32 = 2;

/// malloc(), calloc() and realloc() error upon failure, for eg ENOMEM
pub const XCB_CONN_CLOSED_MEM_INSUFFICIENT: i32 = 3;

/// Connection closed, exceeding request length that server accepts.
pub const XCB_CONN_CLOSED_REQ_LEN_EXCEED: i32 = 4;

/// Connection closed, error during parsing display string.
pub const XCB_CONN_CLOSED_PARSE_ERR: i32 = 5;

/// Connection closed because the server does not have a screen matching the display.
pub const XCB_CONN_CLOSED_INVALID_SCREEN: i32 = 6;

/// Connection closed because some FD passing operation failed
pub const XCB_CONN_CLOSED_FDPASSING_FAILED: i32 = 7;

/// XCB Connection structure.
///
/// A structure that contain all data that  XCB needs to communicate with an X server.
pub enum xcb_connection_t {}

pub(crate) enum xcb_special_event_t {}

/// Generic event.
///
/// A generic event structure.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct xcb_generic_event_t {
    /// Type of the response
    pub response_type: u8,
    /// Padding
    pub pad0: u8,
    /// Sequence number
    pub sequence: u16,
    /// Padding
    pub pad: [u32; 7],
    /// full sequence
    pub full_sequence: u32,
}

// GE_GENERIC stuff is actually generated in X-proto, but we need it here in FFI form,
// so it is just copy pasted from xproto.h

/// `response_type` number corresponding to a [xcb_ge_generic_event_t].
pub const XCB_GE_GENERIC: u8 = 35;

/// FFI type for the Generic Event Extension.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct xcb_ge_generic_event_t {
    pub response_type: u8,
    pub extension: u8,
    pub sequence: u16,
    pub length: u32,
    pub event_type: u16,
    pub pad0: [u8; 22],
    pub full_sequence: u32,
}

/// Generic error
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct xcb_generic_error_t {
    pub response_type: u8,
    pub error_code: u8,
    pub sequence: u16,
    pub resource_id: u32,
    pub minor_code: u16,
    pub major_code: u8,
    pub pad0: u8,
    pub pad: [u32; 5],
    pub full_sequence: u32,
}

/// FFI type for a void request cookie.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub(crate) struct xcb_void_cookie_t {
    pub(crate) seq: u32,
}

/// Container for authorization information.
/// A container for authorization information to be sent to the X server
#[repr(C)]
pub(crate) struct xcb_auth_info_t {
    /// length of the string name (as returned by strlen)
    pub namelen: c_int,
    /// String containing the authentication protocol name,
    /// such as "MIT-MAGIC-COOKIE-1" or "XDM-AUTHORIZATION-1".
    pub name: *mut c_char,
    /// length of the data member
    pub datalen: c_int,
    /// data interpreted in a protocol specific manner
    pub data: *mut c_char,
}

#[cfg(feature = "dl")]
pub(crate) use crate::ffi::dl::define_api_dynamic as define_api;

#[cfg(not(feature = "dl"))]
pub(crate) use crate::ffi::dl::define_api_link as define_api;

define_api! {
    pub(crate) XcbLib XCBLIB_CACHE
    libs: ["libxcb.so.1", "libxcb.so"]
    link: "xcb"

    functions:
    pub(crate) fn xcb_flush(c: *mut xcb_connection_t) -> c_int;

    pub(crate) fn xcb_get_maximum_request_length(c: *mut xcb_connection_t) -> u32;

    pub(crate) fn xcb_prefetch_maximum_request_length(c: *mut xcb_connection_t) -> c_void;

    pub(crate) fn xcb_wait_for_event(c: *mut xcb_connection_t) -> *mut xcb_generic_event_t;

    pub(crate) fn xcb_poll_for_event(c: *mut xcb_connection_t) -> *mut xcb_generic_event_t;

    pub(crate) fn xcb_poll_for_queued_event(c: *mut xcb_connection_t) -> *mut xcb_generic_event_t;

    pub(crate) fn xcb_poll_for_special_event(
        c: *mut xcb_connection_t,
        se: *mut xcb_special_event_t,
    ) -> *mut xcb_generic_event_t;

    pub(crate) fn xcb_wait_for_special_event(
        c: *mut xcb_connection_t,
        se: *mut xcb_special_event_t,
    ) -> *mut xcb_generic_event_t;

    pub(crate) fn xcb_register_for_special_xge(
        c: *mut xcb_connection_t,
        ext: *mut xcb_extension_t,
        eid: u32,
        stamp: *mut u32,
    ) -> *mut xcb_special_event_t;

    pub(crate) fn xcb_unregister_for_special_event(
        c: *mut xcb_connection_t,
        se: *mut xcb_special_event_t,
    );

    pub(crate) fn xcb_request_check(
        c: *mut xcb_connection_t,
        cookie: xcb_void_cookie_t,
    ) -> *mut xcb_generic_error_t;

    pub(crate) fn xcb_discard_reply(c: *mut xcb_connection_t, sequence: c_uint);

    pub(crate) fn xcb_discard_reply64(c: *mut xcb_connection_t, sequence: u64);

    // We trick the result from `*const xcb_query_extension_reply_t` to `*const u8` to be compatible with
    // `x::QueryExtensionReply::from_raw`
    pub(crate) fn xcb_get_extension_data(
        c: *mut xcb_connection_t,
        ext: *mut xcb_extension_t,
    ) -> *const u8;

    pub(crate) fn xcb_prefetch_extension_data(c: *mut xcb_connection_t, ext: *mut xcb_extension_t);

    // We trick the result from `*const xcb_setup_t` to `*const u8` to be compatible with
    // `x::Setup::from_data`
    pub(crate) fn xcb_get_setup(c: *mut xcb_connection_t) -> *const u8;

    pub(crate) fn xcb_get_file_descriptor(c: *mut xcb_connection_t) -> c_int;

    pub(crate) fn xcb_connection_has_error(c: *mut xcb_connection_t) -> c_int;

    pub(crate) fn xcb_connect_to_fd(
        fd: c_int,
        auth_info: *mut xcb_auth_info_t,
    ) -> *mut xcb_connection_t;

    pub(crate) fn xcb_disconnect(c: *mut xcb_connection_t);

    pub(crate) fn xcb_parse_display(
        name: *const c_char,
        host: *mut *mut c_char,
        display: *mut c_int,
        screen: *mut c_int,
    ) -> c_int;

    pub(crate) fn xcb_connect(
        displayname: *const c_char,
        screenp: *mut c_int,
    ) -> *mut xcb_connection_t;

    pub(crate) fn xcb_connect_to_display_with_auth_info(
        display: *const c_char,
        auth: *mut xcb_auth_info_t,
        screen: *mut c_int,
    ) -> *mut xcb_connection_t;

    pub(crate) fn xcb_generate_id(c: *mut xcb_connection_t) -> u32;

    pub(crate) fn xcb_total_read(c: *mut xcb_connection_t) -> u64;

    pub(crate) fn xcb_total_written(c: *mut xcb_connection_t) -> u64;

    // ext
    pub(crate) fn xcb_send_request(
        c: *mut xcb_connection_t,
        flags: c_int,
        vector: *mut iovec,
        request: *const xcb_protocol_request_t,
    ) -> c_uint;

    /**
     * @brief Send a request to the server.
     * @param c The connection to the X server.
     * @param flags A combination of flags from the xcb_send_request_flags_t enumeration.
     * @param vector Data to send; must have two iovecs before start for internal use.
     * @param request Information about the request to be sent.
     * @param num_fds Number of additional file descriptors to send to the server
     * @param fds Additional file descriptors that should be send to the server.
     * @return The request's sequence number on success, 0 otherwise.
     *
     * This function sends a new request to the X server. The data of the request is
     * given as an array of @c iovecs in the @p vector argument. The length of that
     * array and the necessary management information are given in the @p request
     * argument.
     *
     * If @p num_fds is non-zero, @p fds points to an array of file descriptors that
     * will be sent to the X server along with this request. After this function
     * returns, all file descriptors sent are owned by xcb and will be closed
     * eventually.
     *
     * When this function returns, the request might or might not be sent already.
     * Use xcb_flush() to make sure that it really was sent.
     *
     * Please note that this function is not the preferred way for sending requests.
     *
     * Please note that xcb might use index -1 and -2 of the @p vector array internally,
     * so they must be valid!
     */
    pub(crate) fn xcb_send_request_with_fds(
        c: *mut xcb_connection_t,
        flags: c_int,
        vector: *mut iovec,
        request: *const xcb_protocol_request_t,
        num_fds: c_uint,
        fds: *mut c_int,
    ) -> c_uint;

    pub(crate) fn xcb_send_request64(
        c: *mut xcb_connection_t,
        flags: c_int,
        vector: *mut iovec,
        request: *const xcb_protocol_request_t,
    ) -> u64;

    /**
     * @brief Send a request to the server, with 64-bit sequence number returned.
     * @param c The connection to the X server.
     * @param flags A combination of flags from the xcb_send_request_flags_t enumeration.
     * @param vector Data to send; must have two iovecs before start for internal use.
     * @param request Information about the request to be sent.
     * @param num_fds Number of additional file descriptors to send to the server
     * @param fds Additional file descriptors that should be send to the server.
     * @return The request's sequence number on success, 0 otherwise.
     *
     * This function sends a new request to the X server. The data of the request is
     * given as an array of @c iovecs in the @p vector argument. The length of that
     * array and the necessary management information are given in the @p request
     * argument.
     *
     * If @p num_fds is non-zero, @p fds points to an array of file descriptors that
     * will be sent to the X server along with this request. After this function
     * returns, all file descriptors sent are owned by xcb and will be closed
     * eventually.
     *
     * When this function returns, the request might or might not be sent already.
     * Use xcb_flush() to make sure that it really was sent.
     *
     * Please note that this function is not the preferred way for sending requests.
     * It's better to use the generated wrapper functions.
     *
     * Please note that xcb might use index -1 and -2 of the @p vector array internally,
     * so they must be valid!
     */
    pub(crate) fn xcb_send_request_with_fds64(
        c: *mut xcb_connection_t,
        flags: c_int,
        vector: *mut iovec,
        request: *const xcb_protocol_request_t,
        num_fds: c_uint,
        fds: *mut c_int,
    ) -> u64;

    /**
     * @brief Send a file descriptor to the server in the next call to xcb_send_request.
     * @param c The connection to the X server.
     * @param fd The file descriptor to send.
     *
     * After this function returns, the file descriptor given is owned by xcb and
     * will be closed eventually.
     *
     * @deprecated This function cannot be used in a thread-safe way. Two threads
     * that run xcb_send_fd(); xcb_send_request(); could mix up their file
     * descriptors. Instead, xcb_send_request_with_fds() should be used.
     */
    pub(crate) fn xcb_send_fd(c: *mut xcb_connection_t, fd: c_int);

    /** no-run
     * @brief Take over the write side of the socket
     * @param c The connection to the X server.
     * @param return_socket Callback function that will be called when xcb wants
     *                        to use the socket again.
     * @param closure Argument to the callback function.
     * @param flags A combination of flags from the xcb_send_request_flags_t enumeration.
     * @param sent Location to the sequence number of the last sequence request.
     *              Must not be NULL.
     * @return 1 on success, else 0.
     *
     * xcb_take_socket allows external code to ask XCB for permission to
     * take over the write side of the socket and send raw data with
     * xcb_writev. xcb_take_socket provides the sequence number of the last
     * request XCB sent. The caller of xcb_take_socket must supply a
     * callback which XCB can call when it wants the write side of the
     * socket back to make a request. This callback synchronizes with the
     * external socket owner and flushes any output queues if appropriate.
     * If you are sending requests which won't cause a reply, please note the
     * comment for xcb_writev which explains some sequence number wrap issues.
     *
     * All replies that are generated while the socket is owned externally have
     * @p flags applied to them. For example, use XCB_REQUEST_CHECK if you don't
     * want errors to go to xcb's normal error handling, but instead having to be
     * picked up via xcb_wait_for_reply(), xcb_poll_for_reply() or
     * xcb_request_check().
     */
    pub(crate) fn xcb_take_socket(
        c: *mut xcb_connection_t,
        return_socket: extern "C" fn(closure: *mut c_void),
        closure: *mut c_void,
        flags: c_int,
        sent: *mut u64,
    ) -> c_int;

    /**
     * @brief Send raw data to the X server.
     * @param c The connection to the X server.
     * @param vector Array of data to be sent.
     * @param count Number of entries in @p vector.
     * @param requests Number of requests that are being sent.
     * @return 1 on success, else 0.
     *
     * You must own the write-side of the socket (you've called
     * xcb_take_socket, and haven't returned from return_socket yet) to call
     * xcb_writev. Also, the iovec must have at least 1 byte of data in it.
     * You have to make sure that xcb can detect sequence number wraps correctly.
     * This means that the first request you send after xcb_take_socket must cause a
     * reply (e.g. just insert a GetInputFocus request). After every (1 << 16) - 1
     * requests without a reply, you have to insert a request which will cause a
     * reply. You can again use GetInputFocus for this. You do not have to wait for
     * any of the GetInputFocus replies, but can instead handle them via
     * xcb_discard_reply().
     */
    pub(crate) fn xcb_writev(
        c: *mut xcb_connection_t,
        vector: *mut iovec,
        count: c_int,
        requests: u64,
    ) -> c_int;

    pub(crate) fn xcb_wait_for_reply(
        c: *mut xcb_connection_t,
        request: c_uint,
        e: *mut *mut xcb_generic_error_t,
    ) -> *mut c_void;

    pub(crate) fn xcb_wait_for_reply64(
        c: *mut xcb_connection_t,
        request: u64,
        e: *mut *mut xcb_generic_error_t,
    ) -> *mut c_void;

    pub(crate) fn xcb_poll_for_reply(
        c: *mut xcb_connection_t,
        request: c_uint,
        reply: *mut *mut c_void,
        e: *mut *mut xcb_generic_error_t,
    ) -> c_int;

    pub(crate) fn xcb_poll_for_reply64(
        c: *mut xcb_connection_t,
        request: u64,
        reply: *mut *mut c_void,
        e: *mut *mut xcb_generic_error_t,
    ) -> c_int;

    /**
     * @brief Don't use this, only needed by the generated code.
     * @param c The connection to the X server.
     * @param reply A reply that was received from the server
     * @param replylen The size of the reply.
     * @return Pointer to the location where received file descriptors are stored.
     */
    pub(crate) fn xcb_get_reply_fds(
        c: *mut xcb_connection_t,
        reply: *mut c_void,
        replylen: usize,
    ) -> *mut c_int;
}
