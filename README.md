libmount
========

[Documentation](https://docs.rs/libmount) |
[Github](https://github.com/tailhook/libmount) |
[Crate](https://crates.io/crates/libmount)

This is a higher-level wrapper around ``mount()`` system call for linux.

Goals:

1. Type-safe wrapper, including mount options
2. Support of new features such as overlayfs
3. Good support of unprivileges user namespaces
4. Very detailed error messages, which are helpful for end users

Features:

* [x] Bind Mounts
* [x] OverlayFS
* [x] Tmpfs
* [ ] Pseudo file systems: `proc`, `sys`
* [ ] `umount` and `umount2`
* [x] Parser of `/proc/PID/mountinfo`
* [x] Read-only mounts (remount)
* [ ] Ext2/3/4
* [ ] Btrfs
* [ ] Support of mount flags throught trait
* [ ] Fuse


License
=======

Licensed under either of

* Apache License, Version 2.0, (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
