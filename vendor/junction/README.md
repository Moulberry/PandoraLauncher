[![Build Status][actions-badge]][actions-url]
[![Documentation](https://docs.rs/junction/badge.svg)](https://docs.rs/junction)
[![Crates.io](https://img.shields.io/crates/v/junction.svg)](https://crates.io/crates/junction)

# junction

Library for working with [NTFS junctions][junction].

As opposed to symlinks, junction does not require [administrator privileges during creation][perm].
(*However starting with Windows 10 Insiders build 14972, symlinks
can be created without needing to elevate the console as
administrator*)[^1].

Quoted from [Computer Hope](https://www.computerhope.com/jargon/j/junction.htm):

> A junction, also called an NTFS junction point, is a feature of the
> NTFS file system. It is pointer to a directory on the local volume,
> similar to a symlink. It can be accessed through the Windows GUI in
> addition to the Windows command line. Junction points were first
> introduced with Windows 2000 and NTFS 3.0, and are supported in all
> subsequent versions of Windows.

### Minimal Supported Rust versions

Specified in the `rust-version` field in `Cargo.toml`.

## All relevant references

* https://www.codeproject.com/Articles/194/Windows-2000-Junction-Points#The_Solution
* https://www.codeproject.com/Articles/15633/Manipulating-NTFS-Junction-Points-in-NET
* https://web.archive.org/web/20230411010804/http://www.flexhex.com/docs/articles/hard-links.phtml
* https://googleprojectzero.blogspot.com/2016/02/the-definitive-guide-on-win32-to-nt.html
* https://github.com/googleprojectzero/symboliclink-testing-tools/blob/master/DumpReparsePoint/DumpReparsePoint.cpp
* https://github.com/googleprojectzero/symboliclink-testing-tools/blob/master/CommonUtils/ReparsePoint.cpp
* https://github.com/containerd/continuity/blob/master/syscallx/syscall_windows.go

## License

All the code in this repository is released under the MIT License,
for more information, please read COPYRIGHT file.

[actions-badge]: https://github.com/lzutao/junction/workflows/Rust/badge.svg?branchName=master
[actions-url]: https://github.com/lzutao/junction/actions
[junction]: https://learn.microsoft.com/en-us/windows/win32/fileio/hard-links-and-junctions#junctions
[perm]: https://en.wikipedia.org/wiki/NTFS_links#Restrictions_and_drawbacks
[^1]: https://blogs.windows.com/windowsdeveloper/2016/12/02/symlinks-windows-10/#Ed9Olhkz6hJp4KWV.97
