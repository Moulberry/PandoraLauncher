// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

//! Rust Language Bindings for the libseccomp Library
//!
//! The libseccomp library provides an easy to use, platform independent, interface to
//! the Linux Kernel's syscall filtering mechanism. The libseccomp API is designed to
//! abstract away the underlying BPF based syscall filter language and present a more
//! conventional function-call based filtering interface that should be familiar to, and
//! easily adopted by, application developers.
//!
//! The libseccomp crate is a high-level safe API for the libseccomp library.
//!
//! # Examples
//!
//! ```rust
//! use libseccomp::*;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut filter = ScmpFilterContext::new(ScmpAction::Allow)?;
//!     let syscall = ScmpSyscall::from_name("getuid")?;
//!
//!     filter.add_arch(ScmpArch::X8664)?;
//!     filter.add_rule(ScmpAction::Errno(1), syscall)?;
//!     filter.set_ctl_log(true)?;
//!     filter.set_syscall_priority(syscall, 100)?;
//!     filter.load()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! The above example can be replaced with builder pattern.
//!
//! ```rust
//! use libseccomp::*;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let syscall = ScmpSyscall::from_name("getuid")?;
//!
//!     ScmpFilterContext::new(ScmpAction::Allow)?
//!         .add_arch(ScmpArch::X8664)?
//!         .add_rule(ScmpAction::Errno(1), syscall)?
//!         .set_ctl_log(true)?
//!         .set_syscall_priority(syscall, 100)?
//!         .load()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - `const-syscall`: Allow creating of `ScmpSyscall` in a `const`-context.

#![warn(rust_2018_idioms)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::inefficient_to_string)]
#![warn(clippy::string_to_string)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::unwrap_in_result)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// Errors
pub mod error;

mod action;
mod api;
mod arch;
mod arg_compare;
mod compare_op;
mod filter_attr;
mod filter_context;
mod functions;
mod notify;
mod syscall;
mod version;

use error::{Result, SeccompError};

pub use action::ScmpAction;
pub use api::{check_api, get_api, set_api};
pub use arch::ScmpArch;
pub use arg_compare::ScmpArgCompare;
pub use compare_op::ScmpCompareOp;
pub use filter_attr::ScmpFilterAttr;
pub use filter_context::ScmpFilterContext;
pub use functions::*;
pub use notify::*;
pub use syscall::{RawSyscall, ScmpSyscall};
pub use version::{check_version, ScmpVersion};

fn cvt(ret: i32) -> Result<()> {
    if ret == 0 {
        Ok(())
    } else {
        Err(SeccompError::from_errno(ret))
    }
}
