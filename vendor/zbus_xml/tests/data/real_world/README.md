# Real-world introspection documents

Introspection XML captured verbatim from live D-Bus services (running on Ubuntu 24.04), by
launching each service on a real bus and calling `org.freedesktop.DBus.Introspectable.Introspect`
on it:

```sh
busctl call <dest> <path> org.freedesktop.DBus.Introspectable Introspect --json=short \
    | jq -r '.data[0]'
```

The documents are intentionally untouched — whatever the service sent is what is in the file — so
that the parser is exercised with the exact bytes it will encounter in the wild, across the XML
flavors produced by sd-bus, libdbus and GDBus.

| File | Service (version) | Object path |
|------|-------------------|-------------|
| `dbus_daemon.xml` | `dbus-daemon` (1.14.10) | `/org/freedesktop/DBus` |
| `systemd1_manager.xml` | `systemd --user` (255.4) | `/org/freedesktop/systemd1` |
| `systemd1_scope_unit.xml` | `systemd --user` (255.4) | `/org/freedesktop/systemd1/unit/init_2escope` |
| `systemd1_unit_list.xml` | `systemd --user` (255.4) | `/org/freedesktop/systemd1/unit` |
| `hostname1.xml` | `systemd-hostnamed` (255.4) | `/org/freedesktop/hostname1` |
| `timedate1.xml` | `systemd-timedated` (255.4) | `/org/freedesktop/timedate1` |
| `locale1.xml` | `systemd-localed` (255.4) | `/org/freedesktop/locale1` |
| `login1.xml` | `systemd-logind` (255.4) | `/org/freedesktop/login1` |
| `network1.xml` | `systemd-networkd` (255.4) | `/org/freedesktop/network1` |
| `polkit1_authority.xml` | `polkitd` (124) | `/org/freedesktop/PolicyKit1/Authority` |
| `packagekit.xml` | `packagekitd` (1.2.8) | `/org/freedesktop/PackageKit` |
| `dconf_writer.xml` | `dconf-service` (0.40.0, GDBus 2.80.0) | `/ca/desrt/dconf/Writer/user` |

Note: `systemd1_manager.xml` and `login1.xml` contain `h` (file descriptor) signatures, which
zvariant only supports on Unix — parsing these documents fails on other platforms.
