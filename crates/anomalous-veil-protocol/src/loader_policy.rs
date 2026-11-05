//! Private userspace policy for two explicitly declared absent namespace paths
//! and bounded observation of a later live pathname without a namespace answer.
//! The record is personality state, not mapping or filesystem authority. Native
//! copied reads authenticate the suspended caller separately from this policy.

use crate::invocation;

pub const record_size: usize = 128;
pub const pathname_read_budget: usize = 256;
pub const absent_path: &[u8; 18] = b"/etc/ld.so.preload";
pub const absent_cache_path: &[u8; 16] = b"/etc/ld.so.cache";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum record_error {
    size,
    header,
    role,
    initial_break,
    namespace,
    reserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_policy {
    initial_break: u64,
}

impl startup_policy {
    /// Role zero is the inert host record; only startup admits task roles1/2.
    pub fn decode(bytes: &[u8], expected_role: u64) -> Result<Self, record_error> {
        if bytes.len() != record_size {
            return Err(record_error::size);
        }
        let word = |at| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        if expected_role > 2 || word(0) != expected_role {
            return Err(record_error::role);
        }
        if &bytes[8..16] != b"ANLPOL03" || word(16) != 3 || word(24) != record_size as u64 {
            return Err(record_error::header);
        }
        let initial_break = word(32);
        if initial_break == 0 || initial_break & 4095 != 0 || initial_break >= 1 << 47 {
            return Err(record_error::initial_break);
        }
        if word(40) != 1
            || word(48) != absent_path.len() as u64
            || &bytes[56..74] != absent_path
            || word(80) != absent_cache_path.len() as u64
            || &bytes[88..104] != absent_cache_path
        {
            return Err(record_error::namespace);
        }
        if bytes[74..80]
            .iter()
            .chain(&bytes[104..])
            .any(|byte| *byte != 0)
        {
            return Err(record_error::reserved);
        }
        Ok(Self { initial_break })
    }

    pub const fn initial_break(self) -> u64 {
        self.initial_break
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum read_error {
    inaccessible,
    unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum access_outcome {
    returned(i64),
    unsupported,
}

/// Read at most one byte per callback, stopping immediately at NUL. The work
/// budget bounds this invocation, not pathname length throughout the system.
/// Unknown paths and exhausted work remain unsupported rather than invented
/// namespace answers. There is deliberately no successful access result.
pub fn evaluate_access(
    _policy: startup_policy,
    request: invocation,
    read: impl FnMut(u64) -> Result<u8, read_error>,
) -> access_outcome {
    if request.number() != 21 {
        return access_outcome::unsupported;
    }
    let arguments = request.arguments();
    // Linux access takes an int mode; high register bits do not participate.
    if arguments[1] as u32 & !7 != 0 {
        return access_outcome::returned(-22);
    }
    evaluate_absent_path(arguments[0], absent_path, read)
}

/// This bounded personality handles only the observed read-only cache open.
/// Linux openat takes int dirfd/flags, so only their low32 bits participate.
/// Without O_CREAT/__O_TMPFILE, mode is ignored (Linux v6.17 fs/open.c,
/// build_open_how); unsupported flags are not interpreted as absent files.
/// https://raw.githubusercontent.com/torvalds/linux/v6.17/fs/open.c
pub fn evaluate_openat(
    _policy: startup_policy,
    request: invocation,
    read: impl FnMut(u64) -> Result<u8, read_error>,
) -> access_outcome {
    if !openat_shape(request) {
        return access_outcome::unsupported;
    }
    evaluate_absent_path(request.arguments()[1], absent_cache_path, read)
}

fn openat_shape(request: invocation) -> bool {
    let arguments = request.arguments();
    request.number() == 257 && arguments[0] as u32 as i32 == -100 && arguments[2] as u32 == 0x80000
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum path_capture_error {
    request_shape,
    address_overflow,
    inaccessible,
    resource,
    budget,
}

/// Observe an openat pathname without choosing namespace behavior or a Linux
/// result. The returned count includes the first NUL; empty and non-UTF8 names
/// are valid observations. A failed or incomplete read never becomes a capture.
/// The callback independently authenticates the suspended caller's memory.
/// This invocation's 256-read work budget is not a global pathname length limit.
pub fn capture_openat_path(
    request: invocation,
    mut read: impl FnMut(u64) -> Result<u8, read_error>,
) -> Result<usize, path_capture_error> {
    if !openat_shape(request) {
        return Err(path_capture_error::request_shape);
    }
    let pointer = request.arguments()[1];
    for index in 0..pathname_read_budget {
        let address = pointer
            .checked_add(index as u64)
            .ok_or(path_capture_error::address_overflow)?;
        let byte = read(address).map_err(|error| match error {
            read_error::inaccessible => path_capture_error::inaccessible,
            read_error::unsupported => path_capture_error::resource,
        })?;
        if byte == 0 {
            return Ok(index + 1);
        }
    }
    Err(path_capture_error::budget)
}

fn evaluate_absent_path(
    pointer: u64,
    declared_path: &[u8],
    mut read: impl FnMut(u64) -> Result<u8, read_error>,
) -> access_outcome {
    let mut matches = true;
    for index in 0..pathname_read_budget {
        let Some(address) = pointer.checked_add(index as u64) else {
            return access_outcome::returned(-14);
        };
        let byte = match read(address) {
            Ok(byte) => byte,
            Err(read_error::inaccessible) => return access_outcome::returned(-14),
            Err(read_error::unsupported) => return access_outcome::unsupported,
        };
        if byte == 0 {
            return if matches && index == declared_path.len() {
                access_outcome::returned(-2)
            } else {
                access_outcome::unsupported
            };
        }
        matches &= declared_path.get(index) == Some(&byte);
    }
    access_outcome::unsupported
}

/// Interpret one Linux directory suffix for an already-admitted metadata
/// request. The complete userspace namespace must still prove absence: known
/// files, implicit directories, scope roots and unsupported syntax stay false.
/// Removing exactly one slash leaves repeated suffixes invalid. The captured
/// pathname is retained unchanged, and regular-file opens do not use this path.
#[cfg(feature = "read-only-files")]
pub fn metadata_path_is_absent<source: crate::file_service::immutable_file_source + ?Sized>(
    namespace: &crate::file_service::namespace<'_, source>,
    path: &[u8],
) -> bool {
    let path = path.strip_suffix(b"/").unwrap_or(path);
    matches!(
        namespace.lookup(path),
        Err(crate::file_service::file_error::not_found)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(role: u64, initial_break: u64) -> [u8; record_size] {
        let mut bytes = [0; record_size];
        for (at, word) in [
            (0, role),
            (16, 3),
            (24, 128),
            (32, initial_break),
            (40, 1),
            (48, 18),
            (80, 16),
        ] {
            bytes[at..at + 8].copy_from_slice(&word.to_le_bytes());
        }
        bytes[8..16].copy_from_slice(b"ANLPOL03");
        bytes[56..74].copy_from_slice(absent_path);
        bytes[88..104].copy_from_slice(absent_cache_path);
        bytes
    }

    fn policy() -> startup_policy {
        startup_policy::decode(&record(2, 0x40009000), 2).unwrap()
    }

    fn request(address: u64, mode: u64) -> invocation {
        invocation::new(2, 21, [address, mode, 13, 17, 19, 23]).unwrap()
    }

    #[test]
    fn decodes_exact_roles_and_full_width_break() {
        for role in 0..=2 {
            let record = record(role, 0x7fff12345000);
            let decoded = startup_policy::decode(&record, role).unwrap();
            assert_eq!(decoded.initial_break(), 0x7fff12345000);
            for wrong in [0, 1, 2, 3, u64::MAX] {
                if wrong != role {
                    assert_eq!(
                        startup_policy::decode(&record, wrong),
                        Err(record_error::role)
                    );
                }
            }
        }
    }

    #[test]
    fn every_record_byte_is_validated_or_retained_as_the_break_fact() {
        let original = record(2, 0x40009000);
        for index in 0..record_size {
            for bit in 0..8 {
                let mut bytes = original;
                bytes[index] ^= 1 << bit;
                if let Ok(decoded) = startup_policy::decode(&bytes, 2) {
                    assert!((32..40).contains(&index));
                    assert_ne!(decoded.initial_break(), policy().initial_break());
                    assert_eq!(decoded.initial_break() & 4095, 0);
                    assert!(decoded.initial_break() < 1 << 47);
                }
            }
        }
        for length in 0..record_size {
            assert_eq!(
                startup_policy::decode(&original[..length], 2),
                Err(record_error::size)
            );
        }
        let mut longer = [0; record_size + 1];
        longer[..record_size].copy_from_slice(&original);
        assert_eq!(startup_policy::decode(&longer, 2), Err(record_error::size));
        for bad in [0, 1, 0x40009001, 1 << 47, u64::MAX] {
            assert_eq!(
                startup_policy::decode(&record(2, bad), 2),
                Err(record_error::initial_break)
            );
        }
    }

    #[test]
    fn absent_path_is_not_success_and_nul_at_page_end_needs_no_following_read() {
        let nul = 0x1fff;
        let start = nul - absent_path.len() as u64;
        for mode in 0..=7 {
            let mut count = 0;
            let result = evaluate_access(policy(), request(start, mode), |address| {
                assert_eq!(address, start + count as u64);
                assert!(address <= nul);
                count += 1;
                Ok(if address == nul {
                    0
                } else {
                    absent_path[(address - start) as usize]
                })
            });
            assert_eq!(result, access_outcome::returned(-2));
            assert_eq!(count, 19);
        }
    }

    #[test]
    fn invalid_modes_do_not_read_and_linux_int_truncation_is_explicit() {
        for mode in [8, 15, 0x80000000, u32::MAX as u64, u64::MAX] {
            assert_eq!(
                evaluate_access(policy(), request(0x1000, mode), |_| panic!(
                    "unexpected read"
                )),
                access_outcome::returned(-22)
            );
        }
        for mode in [4, (1 << 32) | 4] {
            assert_eq!(
                evaluate_access(policy(), request(0x1000, mode), |_| Err(
                    read_error::inaccessible
                )),
                access_outcome::returned(-14)
            );
        }
    }

    #[test]
    fn inaccessible_and_overflow_stop_without_wrapping_or_claiming_absence() {
        let mut count = 0;
        let result = evaluate_access(policy(), request(u64::MAX, 4), |address| {
            assert_eq!(address, u64::MAX);
            count += 1;
            Ok(b'/')
        });
        assert_eq!(result, access_outcome::returned(-14));
        assert_eq!(count, 1);
        for fail_at in 0..19 {
            let mut count = 0;
            let result = evaluate_access(policy(), request(0x1000, 4), |address| {
                assert_eq!(address, 0x1000 + count as u64);
                count += 1;
                if count - 1 == fail_at {
                    Err(read_error::inaccessible)
                } else {
                    Ok(absent_path[count - 1])
                }
            });
            assert_eq!(result, access_outcome::returned(-14));
            assert_eq!(count, fail_at + 1);
        }
    }

    #[test]
    fn unknown_empty_extended_and_unterminated_paths_remain_unsupported() {
        for bytes in [
            b"\0".as_slice(),
            b"/etc/ld.so.preloaD\0",
            b"/etc/ld.so.preload/extra\0",
            b"/elsewhere\0",
        ] {
            let mut count = 0;
            assert_eq!(
                evaluate_access(policy(), request(0x1000, 4), |_| {
                    let byte = bytes[count];
                    count += 1;
                    Ok(byte)
                }),
                access_outcome::unsupported
            );
            assert_eq!(count, bytes.len());
        }
        let mut count = 0;
        assert_eq!(
            evaluate_access(policy(), request(0x1000, 4), |_| {
                count += 1;
                Ok(b'a')
            }),
            access_outcome::unsupported
        );
        assert_eq!(count, pathname_read_budget);
        assert_eq!(
            evaluate_access(policy(), request(0x1000, 4), |_| Err(
                read_error::unsupported
            )),
            access_outcome::unsupported
        );
        let other = invocation::new(2, 22, [0; 6]).unwrap();
        assert_eq!(
            evaluate_access(policy(), other, |_| panic!("unexpected read")),
            access_outcome::unsupported
        );
    }

    #[test]
    fn callback_resource_refusal_is_unsupported_and_stops_further_reads() {
        let mut count = 0;
        let result = evaluate_access(policy(), request(0x1000, 4), |address| {
            assert_eq!(address, 0x1000 + count as u64);
            count += 1;
            assert!(count <= 8);
            if count == 8 {
                Err(read_error::unsupported)
            } else {
                Ok(absent_path[count - 1])
            }
        });
        assert_eq!(result, access_outcome::unsupported);
        assert_eq!(count, 8);
    }

    fn open_request(address: u64, dirfd: u64, flags: u64, mode: u64) -> invocation {
        invocation::new(3, 257, [dirfd, address, flags, mode, 19, 23]).unwrap()
    }

    #[test]
    fn cache_open_uses_linux_int_arguments_and_ignores_noncreating_mode() {
        for dirfd in [0xffffff9c, (-100i64) as u64, 0x12345678ffffff9c] {
            for flags in [0x80000, 0xffffffff00080000] {
                for mode in [0, 0o777, u64::MAX] {
                    let mut count = 0;
                    let result = evaluate_openat(
                        policy(),
                        open_request(0x1000, dirfd, flags, mode),
                        |address| {
                            assert_eq!(address, 0x1000 + count as u64);
                            let byte = absent_cache_path.get(count).copied().unwrap_or(0);
                            count += 1;
                            Ok(byte)
                        },
                    );
                    assert_eq!(result, access_outcome::returned(-2));
                    assert_eq!(count, 17);
                }
            }
        }
    }

    #[test]
    fn cache_open_rejects_other_calls_dirfds_and_flags_without_reading() {
        for dirfd in [0, 1, 99, u64::MAX, 0xffffff9b] {
            assert_eq!(
                evaluate_openat(policy(), open_request(0x1000, dirfd, 0x80000, 0), |_| {
                    panic!("unexpected read")
                }),
                access_outcome::unsupported
            );
        }
        for flags in [
            0,
            1,
            2,
            0x80001,
            0x80002,
            0x80040,
            0x80080,
            0x490000,
            u32::MAX as u64,
        ] {
            assert_eq!(
                evaluate_openat(policy(), open_request(0x1000, 0xffffff9c, flags, 0), |_| {
                    panic!("unexpected read")
                }),
                access_outcome::unsupported
            );
        }
        assert_eq!(
            evaluate_openat(policy(), request(0x1000, 4), |_| panic!("unexpected read")),
            access_outcome::unsupported
        );
    }

    #[test]
    fn cache_nul_at_page_end_requires_no_following_byte() {
        let nul = 0x1fff;
        let start = nul - absent_cache_path.len() as u64;
        let mut count = 0;
        let result = evaluate_openat(
            policy(),
            open_request(start, 0xffffff9c, 0x80000, 0),
            |address| {
                assert_eq!(address, start + count as u64);
                assert!(address <= nul);
                let byte = absent_cache_path.get(count).copied().unwrap_or(0);
                count += 1;
                Ok(byte)
            },
        );
        assert_eq!(result, access_outcome::returned(-2));
        assert_eq!(count, 17);
    }

    #[test]
    fn cache_faults_overflow_and_resource_budget_do_not_invent_absence() {
        for fail_at in 0..17 {
            for error in [read_error::inaccessible, read_error::unsupported] {
                let mut count = 0;
                let result = evaluate_openat(
                    policy(),
                    open_request(0x1000, 0xffffff9c, 0x80000, 0),
                    |address| {
                        assert_eq!(address, 0x1000 + count as u64);
                        count += 1;
                        if count - 1 == fail_at {
                            Err(error)
                        } else {
                            Ok(absent_cache_path[count - 1])
                        }
                    },
                );
                assert_eq!(
                    result,
                    match error {
                        read_error::inaccessible => access_outcome::returned(-14),
                        read_error::unsupported => access_outcome::unsupported,
                    }
                );
                assert_eq!(count, fail_at + 1);
            }
        }
        let mut count = 0;
        assert_eq!(
            evaluate_openat(
                policy(),
                open_request(u64::MAX, 0xffffff9c, 0x80000, 0),
                |address| {
                    assert_eq!(address, u64::MAX);
                    count += 1;
                    Ok(b'/')
                }
            ),
            access_outcome::returned(-14)
        );
        assert_eq!(count, 1);
        count = 0;
        assert_eq!(
            evaluate_openat(
                policy(),
                open_request(0x1000, 0xffffff9c, 0x80000, 0),
                |_| {
                    count += 1;
                    Ok(b'a')
                }
            ),
            access_outcome::unsupported
        );
        assert_eq!(count, pathname_read_budget);
    }

    #[test]
    fn cache_unknown_empty_extended_and_cross_paths_remain_unsupported() {
        for bytes in [
            b"\0".as_slice(),
            b"/etc/ld.so.cachE\0",
            b"/etc/ld.so.cache/extra\0",
            b"/elsewhere\0",
            b"/etc/ld.so.preload\0",
        ] {
            let mut count = 0;
            assert_eq!(
                evaluate_openat(
                    policy(),
                    open_request(0x1000, 0xffffff9c, 0x80000, 0),
                    |_| {
                        let byte = bytes[count];
                        count += 1;
                        Ok(byte)
                    }
                ),
                access_outcome::unsupported
            );
            assert_eq!(count, bytes.len());
        }
        let bytes = b"/etc/ld.so.cache\0";
        let mut count = 0;
        assert_eq!(
            evaluate_access(policy(), request(0x1000, 4), |_| {
                let byte = bytes[count];
                count += 1;
                Ok(byte)
            }),
            access_outcome::unsupported
        );
        assert_eq!(count, bytes.len());
    }

    #[test]
    fn capture_observes_empty_binary_and_unknown_names_without_policy_results() {
        for bytes in [
            b"\0".as_slice(),
            b"\xff\x80/\xfe\0",
            b"/not/a/declared/name\0",
            b"/etc/ld.so.preload\0",
            b"/etc/ld.so.cache\0",
        ] {
            let mut count = 0;
            let result =
                capture_openat_path(open_request(0x1000, 0xffffff9c, 0x80000, 0), |address| {
                    assert_eq!(address, 0x1000 + count as u64);
                    let byte = bytes[count];
                    count += 1;
                    Ok(byte)
                });
            assert_eq!(result, Ok(bytes.len()));
            assert_eq!(count, bytes.len());
        }
    }

    #[test]
    fn capture_validates_linux_int_shape_before_reading_and_ignores_mode() {
        for dirfd in [0xffffff9c, (-100i64) as u64, 0x12345678ffffff9c] {
            for flags in [0x80000, 0xffffffff00080000] {
                for unused in [0, 1, u64::MAX] {
                    let request =
                        invocation::new(4, 257, [dirfd, 0x1000, flags, unused, unused, unused])
                            .unwrap();
                    let mut count = 0;
                    assert_eq!(
                        capture_openat_path(request, |address| {
                            assert_eq!(address, 0x1000);
                            count += 1;
                            Ok(0)
                        }),
                        Ok(1)
                    );
                    assert_eq!(count, 1);
                }
            }
        }
        for invalid in [
            request(0x1000, 4),
            open_request(0x1000, 0, 0x80000, 0),
            open_request(0x1000, u64::MAX, 0x80000, 0),
            open_request(0x1000, 0xffffff9c, 0, 0),
            open_request(0x1000, 0xffffff9c, 0x80001, 0),
            open_request(0x1000, 0xffffff9c, 0x80040, 0),
            open_request(0x1000, 0xffffff9c, 0x490000, 0),
        ] {
            assert_eq!(
                capture_openat_path(invalid, |_| panic!("unexpected read")),
                Err(path_capture_error::request_shape)
            );
        }
    }

    #[test]
    fn capture_checks_every_shape_bit_and_only_truncates_linux_int_arguments() {
        let valid = [0xffffff9c, 0x1000, 0x80000, 0, 0, 0];
        for bit in 0..64 {
            let changed_number = invocation::new(4, 257 ^ (1 << bit), valid).unwrap();
            assert_eq!(
                capture_openat_path(changed_number, |_| panic!("unexpected read")),
                Err(path_capture_error::request_shape)
            );
            for argument in [0, 2] {
                let mut changed = valid;
                changed[argument] ^= 1 << bit;
                let changed_request = invocation::new(4, 257, changed).unwrap();
                let mut count = 0;
                let result = capture_openat_path(changed_request, |address| {
                    assert_eq!(address, 0x1000);
                    count += 1;
                    Ok(0)
                });
                if bit < 32 {
                    assert_eq!(result, Err(path_capture_error::request_shape));
                    assert_eq!(count, 0);
                } else {
                    assert_eq!(result, Ok(1));
                    assert_eq!(count, 1);
                }
            }
        }
    }

    #[test]
    fn capture_stops_at_every_nul_position_without_reading_following_memory() {
        for length in 1..=pathname_read_budget {
            // The first range ends exactly at a page boundary; the second
            // crosses one; the third ends at the final possible address.
            for start in [
                0x2000 - length as u64,
                0x1fff,
                u64::MAX - (length - 1) as u64,
            ] {
                let mut count = 0;
                let result =
                    capture_openat_path(open_request(start, 0xffffff9c, 0x80000, 0), |address| {
                        assert_eq!(address, start.checked_add(count as u64).unwrap());
                        assert!(count < length);
                        count += 1;
                        Ok(if count == length { 0 } else { 0xff })
                    });
                assert_eq!(result, Ok(length));
                assert_eq!(count, length);
            }
        }
    }

    #[test]
    fn capture_fault_and_resource_refusal_at_every_byte_are_distinct_failures() {
        for fail_at in 0..pathname_read_budget {
            for (error, expected) in [
                (read_error::inaccessible, path_capture_error::inaccessible),
                (read_error::unsupported, path_capture_error::resource),
            ] {
                let mut count = 0;
                let result =
                    capture_openat_path(open_request(0x1000, 0xffffff9c, 0x80000, 0), |address| {
                        assert_eq!(address, 0x1000 + count as u64);
                        count += 1;
                        if count - 1 == fail_at {
                            Err(error)
                        } else {
                            Ok(b'a')
                        }
                    });
                assert_eq!(result, Err(expected));
                assert_eq!(count, fail_at + 1);
            }
        }
    }

    #[test]
    fn capture_distinguishes_address_overflow_from_exact_work_exhaustion() {
        for available in 1..pathname_read_budget {
            let start = u64::MAX - (available - 1) as u64;
            let mut count = 0;
            assert_eq!(
                capture_openat_path(open_request(start, 0xffffff9c, 0x80000, 0), |address| {
                    assert_eq!(address, start.checked_add(count as u64).unwrap());
                    assert!(count < available);
                    count += 1;
                    Ok(b'a')
                }),
                Err(path_capture_error::address_overflow)
            );
            assert_eq!(count, available);
        }
        for start in [0x1000, u64::MAX - (pathname_read_budget - 1) as u64] {
            let mut count = 0;
            assert_eq!(
                capture_openat_path(open_request(start, 0xffffff9c, 0x80000, 0), |address| {
                    assert_eq!(address, start.checked_add(count as u64).unwrap());
                    count += 1;
                    assert!(count <= pathname_read_budget);
                    Ok(b'a')
                }),
                Err(path_capture_error::budget)
            );
            assert_eq!(count, pathname_read_budget);
        }
    }
}

#[cfg(all(test, feature = "read-only-files"))]
mod metadata_path_tests {
    use super::metadata_path_is_absent;
    use crate::file_service::{file_error, file_snapshot, namespace, namespace_entry};

    #[test]
    fn one_directory_suffix_allows_proven_absence_without_changing_open_lookup() {
        let file = file_snapshot::new(1, 1, b"libc").unwrap();
        let entries = [namespace_entry::new(b"/lib/x86_64-linux-gnu/libc.so.6", file).unwrap()];
        let namespace = namespace::new(b"/lib/x86_64-linux-gnu", &entries).unwrap();
        let path = b"/lib/x86_64-linux-gnu/glibc-hwcaps/x86-64-v2/";
        assert!(metadata_path_is_absent(&namespace, path));
        assert!(metadata_path_is_absent(&namespace, &path[..path.len() - 1]));
        assert_eq!(namespace.lookup(path), Err(file_error::unsupported_path));
    }

    #[test]
    fn known_file_scope_root_and_implicit_directories_never_become_absent() {
        let file = file_snapshot::new(1, 1, b"libc").unwrap();
        let entries = [
            namespace_entry::new(b"/lib/x86_64-linux-gnu/libc.so.6", file).unwrap(),
            namespace_entry::new(b"/lib/x86_64-linux-gnu/present/nested/libc.so.6", file).unwrap(),
        ];
        let namespace = namespace::new(b"/lib/x86_64-linux-gnu", &entries).unwrap();
        for path in [
            &b"/lib/x86_64-linux-gnu/"[..],
            b"/lib/x86_64-linux-gnu/libc.so.6/",
            b"/lib/x86_64-linux-gnu/present/",
            b"/lib/x86_64-linux-gnu/present/nested/",
        ] {
            assert!(!metadata_path_is_absent(&namespace, path));
            assert!(!metadata_path_is_absent(
                &namespace,
                &path[..path.len() - 1]
            ));
        }
    }

    #[test]
    fn malformed_out_of_scope_and_file_descendant_paths_remain_unsupported() {
        let file = file_snapshot::new(1, 1, b"libc").unwrap();
        let entries = [namespace_entry::new(b"/lib/x86_64-linux-gnu/libc.so.6", file).unwrap()];
        let namespace = namespace::new(b"/lib/x86_64-linux-gnu", &entries).unwrap();
        for path in [
            &b"/lib/x86_64-linux-gnu/missing//"[..],
            b"/lib/x86_64-linux-gnu//missing/",
            b"/lib/x86_64-linux-gnu/./missing/",
            b"/lib/x86_64-linux-gnu/../missing/",
            b"/lib/x86_64-linux-gnu/libc.so.6/child/",
            b"/lib/x86_64-linux-gnux/missing/",
            b"/other/missing/",
            b"/lib/x86_64-linux-gnu/missing\0/",
            b"lib/x86_64-linux-gnu/missing/",
            b"/",
            b"",
        ] {
            assert!(!metadata_path_is_absent(&namespace, path));
        }
    }
}
