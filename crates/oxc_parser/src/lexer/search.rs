//! Structs and macros for searching source for combinations of byte values.
//!
//! * `ByteMatchTable` and `SafeByteMatchTable` are lookup table types for byte values.
//! * `byte_match_table!` and `safe_byte_match_table!` macros create those tables at compile time.
//! * `byte_search!` macro searches source text for first byte matching a byte table.

/// Batch size for searching
pub const SEARCH_BATCH_SIZE: usize = 32;

/// Byte matcher lookup table.
///
/// Create table at compile time as a `static` or `const` with `byte_match_table!` macro.
/// Test bytes against table with `ByteMatchTable::matches`.
/// Or use `byte_search!` macro to search for first matching byte in source.
///
/// If the match pattern satisfies constraints of `SafeByteMatchTable`, use that instead.
///
/// # Examples
/// ```
/// use crate::lexer::search::{ByteMatchTable, byte_match_table};
///
/// static NOT_WHITESPACE: ByteMatchTable = byte_match_table!(|b| b != b' ' && b != b'\t');
/// assert_eq!(NOT_WHITESPACE.matches(b'X'), true);
/// assert_eq!(NOT_WHITESPACE.matches(b' '), false);
///
/// impl<'a> Lexer<'a> {
///   fn eat_whitespace(&mut self) {
///     // NB: Using `byte_search!` macro with a `ByteMatchTable` is unsafe
///     unsafe {
///       byte_search! {
///         lexer: self,
///         table: NOT_WHITESPACE,
///         handle_match: |matched_byte, start| {},
///         handle_eof: |start| {},
///       };
///     };
///   }
/// }
/// ```
// TODO: Delete this type + `byte_match_table!` macro if not used
#[repr(C, align(64))]
pub struct ByteMatchTable([bool; 256]);

#[allow(dead_code)]
impl ByteMatchTable {
    // Create new `ByteMatchTable`.
    pub const fn new(bytes: [bool; 256]) -> Self {
        let mut table = Self([false; 256]);
        let mut i = 0;
        loop {
            table.0[i] = bytes[i];
            i += 1;
            if i == 256 {
                break;
            }
        }
        table
    }

    /// Declare that using this table for searching.
    /// An unsafe function here, whereas for `SafeByteMatchTable` it's safe.
    /// `byte_search!` macro calls `.use_table()` on whatever table it's provided, which makes
    /// using the macro unsafe for `ByteMatchTable`, but safe for `SafeByteMatchTable`.
    #[allow(clippy::unused_self)]
    #[inline]
    pub const unsafe fn use_table(&self) {}

    /// Test a value against this `ByteMatchTable`.
    #[inline]
    pub const fn matches(&self, b: u8) -> bool {
        self.0[b as usize]
    }
}

/// Macro to create a `ByteMatchTable` at compile time.
///
/// `byte_match_table!(|b| b < 3)` expands to:
///
/// ```
/// {
///   use crate::lexer::search::ByteMatchTable;
///   #[allow(clippy::eq_op)]
///   const TABLE: ByteMatchTable = ByteMatchTable::new([
///     (0u8 < 3),
///     (1u8 < 3),
///     (2u8 < 3),
///     (3u8 < 3),
///     /* ... */
///     (254u8 < 3),
///     (255u8 < 3),
///   ]);
///   TABLE
/// }
/// ```
#[allow(unused_macros)]
macro_rules! byte_match_table {
    (|$byte:ident| $res:expr) => {{
        use crate::lexer::search::ByteMatchTable;
        // Clippy creates warnings because e.g. `byte_match_table!(|b| b == 0)`
        // is expanded to `ByteMatchTable([(0 == 0), ... ])`
        #[allow(clippy::eq_op)]
        const TABLE: ByteMatchTable = seq_macro::seq!($byte in 0u8..=255 {
            ByteMatchTable::new([ #($res,)* ])
        });
        TABLE
    }};
}
#[allow(unused_imports)]
pub(crate) use byte_match_table;

/// Safe byte matcher lookup table.
///
/// Create table at compile time as a `static` or `const` with `safe_byte_match_table!` macro.
/// Test bytes against table with `SafeByteMatchTable::matches`.
/// Or use `byte_search!` macro to search for first matching byte in source.
///
/// Only difference between this and `ByteMatchTable` is that for `SafeByteMatchTable`,
/// it must be guaranteed that `byte_search!` macro using this table will always end up with
/// `lexer.source` positioned on a UTF-8 character boundary.
///
/// Usage of `byte_search!` macro with a `SafeByteMatchTable` table is safe,
/// and does not require an `unsafe {}` block (unlike `ByteMatchTable`).
///
/// To make this guarantee, one of the following must be true:
///
/// 1. Table contains `true` for all byte values 192 - 247
///    i.e. first byte of any multi-byte Unicode character matches.
///    (NB: 248 - 255 cannot occur in UTF-8 strings)
///    e.g. `safe_byte_match_table!(|b| b >= 192)`
///         `safe_byte_match_table!(|b| !b.is_ascii())`
///
/// 2. Table contains `false` for all byte values 128 - 191
///    i.e. the continuation bytes of any multi-byte Unicode chars will be consumed in full.
///    e.g. `safe_byte_match_table!(|b| b < 128 || b >= 192)`
///         `safe_byte_match_table!(|b| b.is_ascii())`
///         `safe_byte_match_table!(|b| b == ' ' || b == '\t')`
///
/// This is statically checked by `SafeByteMatchTable::new`, and will fail to compile if match
/// pattern does not satisfy one of the above.
///
/// # Examples
/// ```
/// use crate::lexer::search::{SafeByteMatchTable, safe_byte_match_table};
///
/// static NOT_ASCII: SafeByteMatchTable = safe_byte_match_table!(|b| !b.is_ascii());
/// assert_eq!(NOT_ASCII.matches(b'X'), false);
/// assert_eq!(NOT_ASCII.matches(192), true);
///
/// impl<'a> Lexer<'a> {
///   fn eat_ascii(&mut self) {
///     // NB: Using `byte_search!` macro with a `SafeByteMatchTable` is safe
///     byte_search! {
///       lexer: self,
///       table: NOT_ASCII,
///       handle_match: |matched_byte, start| {},
///       handle_eof: |start| {},
///     };
///   }
/// }
/// ```
#[repr(C, align(64))]
pub struct SafeByteMatchTable([bool; 256]);

impl SafeByteMatchTable {
    // Create new `SafeByteMatchTable`.
    pub const fn new(bytes: [bool; 256]) -> Self {
        let mut table = Self([false; 256]);

        // Check if contains either:
        // 1. `true` for all byte values 192..248
        // 2. `false` for all byte values 128..192
        let mut unicode_start_all_match = true;
        let mut unicode_cont_all_no_match = true;

        let mut i = 0;
        loop {
            let matches = bytes[i];
            table.0[i] = matches;

            if matches {
                if i >= 128 && i < 192 {
                    unicode_cont_all_no_match = false;
                }
            } else if i >= 192 && i < 248 {
                unicode_start_all_match = false;
            }

            i += 1;
            if i == 256 {
                break;
            }
        }

        assert!(
            unicode_start_all_match || unicode_cont_all_no_match,
            "Cannot create a `SafeByteMatchTable` with an unsafe pattern"
        );

        table
    }

    /// Declare that using this table for searching.
    /// A safe function here, whereas for `ByteMatchTable` it's unsafe.
    /// `byte_search!` macro calls `.use_table()` on whatever table it's provided, which makes
    /// using the macro unsafe for `ByteMatchTable`, but safe for `SafeByteMatchTable`.
    #[allow(clippy::unused_self)]
    #[inline]
    pub const fn use_table(&self) {}

    /// Test a value against this `SafeByteMatchTable`.
    #[inline]
    pub const fn matches(&self, b: u8) -> bool {
        self.0[b as usize]
    }
}

/// Macro to create a `SafeByteMatchTable` at compile time.
///
/// `safe_byte_match_table!(|b| !b.is_ascii())` expands to:
///
/// ```
/// {
///   use crate::lexer::search::SafeByteMatchTable;
///   #[allow(clippy::eq_op)]
///   const TABLE: SafeByteMatchTable = SafeByteMatchTable::new([
///     (!0u8.is_ascii()),
///     (!1u8.is_ascii()),
///     /* ... */
///     (!255u8.is_ascii()),
///   ]);
///   TABLE
/// }
/// ```
macro_rules! safe_byte_match_table {
    (|$byte:ident| $res:expr) => {{
        use crate::lexer::search::SafeByteMatchTable;
        // Clippy creates warnings because e.g. `safe_byte_match_table!(|b| b == 0)`
        // is expanded to `SafeByteMatchTable([0 == 0, ... ])`
        #[allow(clippy::eq_op)]
        const TABLE: SafeByteMatchTable = seq_macro::seq!($byte in 0u8..=255 {
            SafeByteMatchTable::new([#($res,)*])
        });
        TABLE
    }};
}
pub(crate) use safe_byte_match_table;

/// Macro to search for first byte matching a `ByteMatchTable` or `SafeByteMatchTable`.
///
/// Search processes source in batches of `SEARCH_BATCH_SIZE` bytes for speed.
/// When not enough bytes remaining in source for a batch, search source byte by byte.
///
/// This is a macro rather than a function for 2 reasons:
/// 1. Searching is a bit faster when all the code is in a single function.
/// 2. The `handle_match` section has to be repeated twice.
///    This macro does that, so code using the macro can be DRY-er.
///
/// Used as follows:
///
/// ```
/// static NOT_STUFF_TABLE: SafeByteMatchTable = safe_byte_match_table!(|b| !is_stuff(b));
///
/// impl<'a> Lexer<'a> {
///   fn eat_stuff(&mut self) -> bool {
///     byte_search! {
///       lexer: self,
///       table: NOT_STUFF_TABLE,
///       handle_match: |matched_byte, start| {
///         // Matching byte has been found.
///         // `matched_byte` is `u8` value of first byte which matched the table.
///         // `start` is `SourcePosition` where search began.
///         // `lexer.source` is now positioned on first matching byte.
///         // Handle the next matching byte (deal with any special cases).
///         // Value this block evaluates to will be returned from enclosing function.
///         matched_byte == b'X'
///       },
///       handle_eof: |start| {
///         // No bytes from start position to end of source matched the table.
///         // `start` is `SourcePosition` where search began.
///         // `lexer.source` is now positioned at EOF.
///         // Handle EOF in some way.
///         // Value this block evaluates to will be returned from enclosing function.
///         false
///       },
///     };
///
///     // This is unreachable.
///     // Macro always exits current function with a `return` statement.
///   }
/// }
/// ```
///
/// or provide the `SourcePosition` to start searching from:
///
/// ```
/// impl<'a> Lexer<'a> {
///   fn eat_stuff(&mut self) -> bool {
///     let start = unsafe { self.source.position().add(1) };
///     byte_search! {
///       lexer: self,
///       table: NOT_STUFF_TABLE,
///       start: start,
///       handle_match: |matched_byte| {
///         // Matching byte has been found.
///         // `matched_byte` is `u8` value of first byte which matched the table.
///         // `lexer.source` is now positioned on first matching byte.
///         // Handle the next matching byte (deal with any special cases).
///         // Value this block evaluates to will be returned from enclosing function.
///         true
///       },
///       handle_eof: || {
///         // No bytes from start position to end of source matched the table.
///         // `lexer.source` is now positioned at EOF.
///         // Handle EOF in some way.
///         // Value this block evaluates to will be returned from enclosing function.
///         false
///       },
///     };
///
///     // This is unreachable.
///     // Macro always exits current function with a `return` statement.
///   }
/// }
/// ```
///
/// Can also add a block to decide whether to continue searching for some matches:
///
/// ```text
/// impl<'a> Lexer<'a> {
///   fn eat_stuff(&mut self) -> bool {
///     byte_search! {
///       lexer: self,
///       table: NOT_STUFF_TABLE,
///       continue_if: |matched_byte, pos| {
///         // Matching byte found. Decide whether it's really a match.
///         // NB: `lexer.source` has NOT been updated at this point.
///         if matched_byte == 0xE2 {
///           // Only match a specific Unicode char (in this case 0xE2, 0x80, 0xA8)
///           unsafe { pos.add(1).read() != 0x80 || pos.add(2).read() != 0xA8) }
///         } else {
///           // All others do match. `handle_match` is executed.
///           false
///         }
///       },
///       handle_match: |matched_byte| {
///         // Matching byte has been found and `continue_if` returned `false` for it.
///         // `matched_byte` is `u8` value of first byte which matched the table.
///         // `lexer.source` is now positioned on first matching byte.
///         // Handle the next matching byte (deal with any special cases).
///         // Value this block evaluates to will be returned from enclosing function.
///         true
///       },
///       handle_eof: || {
///         // No bytes from start position to end of source matched the table.
///         // `lexer.source` is now positioned at EOF.
///         // Handle EOF in some way.
///         // Value this block evaluates to will be returned from enclosing function.
///         false
///       },
///     };
///
///     // This is unreachable.
///     // Macro always exits current function with a `return` statement.
///   }
/// }
/// ```
///
/// NB: The macro always causes enclosing function to return.
/// It creates `return` statements with the value that `handle_match` / `handle_eof` blocks evaluate to.
/// After the `byte_search!` macro is unreachable.
///
/// # SAFETY
///
/// This macro will consume bytes from `lexer.source` according to the `ByteMatchTable`
/// or `SafeByteMatchTable` provided.
///
/// Using `byte_search!` with a `SafeByteMatchTable` is guaranteed to end up with `lexer.source`
/// positioned on a UTF-8 character boundary when entering `handle_match`.
/// Therefore it's safe to use `byte_search!` with a `SafeByteMatchTable`.
///
/// `ByteMatchTable` makes no such guarantee, and using `byte_search!` with a `ByteMatchTable` is unsafe.
/// It is caller's responsibility to ensure that `lexer.source` is moved onto a UTF-8 character boundary.
/// This is similar to the contract's of `Source`'s unsafe methods.
macro_rules! byte_search {
    // Standard version.
    // `start` is calculated from current position of `lexer.source`.
    (
        lexer: $lexer:ident,
        table: $table:ident,
        handle_match: |$match_byte:ident, $match_start:ident| $match_handler:expr,
        handle_eof: |$eof_start:ident| $eof_handler:expr,
    ) => {{
        let start = $lexer.source.position();
        byte_search! {
            lexer: $lexer,
            table: $table,
            start: start,
            continue_if: |__byte, pos| false,
            handle_match: |$match_byte, $match_start| $match_handler,
            handle_eof: |$eof_start| $eof_handler,
        }
    }};

    // Standard version with `continue_if`.
    // `start` is calculated from current position of `lexer.source`.
    (
        lexer: $lexer:ident,
        table: $table:ident,
        continue_if: |$continue_byte:ident, $pos:ident| $should_continue:expr,
        handle_match: |$match_byte:ident, $match_start:ident| $match_handler:expr,
        handle_eof: |$eof_start:ident| $eof_handler:expr,
    ) => {{
        let start = $lexer.source.position();
        byte_search! {
            lexer: $lexer,
            table: $table,
            start: start,
            continue_if: |$continue_byte, $pos| $should_continue,
            handle_match: |$match_byte, $match_start| $match_handler,
            handle_eof: |$eof_start| $eof_handler,
        }
    }};

    // Provide your own `start` position
    (
        lexer: $lexer:ident,
        table: $table:ident,
        start: $start:ident,
        handle_match: |$match_byte:ident| $match_handler:expr,
        handle_eof: || $eof_handler:expr,
    ) => {
        byte_search! {
            lexer: $lexer,
            table: $table,
            start: $start,
            continue_if: |__byte, pos| false,
            handle_match: |$match_byte, __start| $match_handler,
            handle_eof: |__start| $eof_handler,
        }
    };

    // Provide your own `start` position, and `continue_if`
    (
        lexer: $lexer:ident,
        table: $table:ident,
        start: $start:ident,
        continue_if: |$continue_byte:ident, $pos:ident| $should_continue:expr,
        handle_match: |$match_byte:ident| $match_handler:expr,
        handle_eof: || $eof_handler:expr,
    ) => {{
        byte_search! {
            lexer: $lexer,
            table: $table,
            start: $start,
            continue_if: |$continue_byte, $pos| $should_continue,
            handle_match: |$match_byte, __start| $match_handler,
            handle_eof: |__start| $eof_handler,
        }
    }};

    // Actual implementation
    (
        lexer: $lexer:ident,
        table: $table:ident,
        start: $start:ident,
        continue_if: |$continue_byte:ident, $pos:ident| $should_continue:expr,
        handle_match: |$match_byte:ident, $match_start:ident| $match_handler:expr,
        handle_eof: |$eof_start:ident| $eof_handler:expr,
    ) => {{
        // SAFETY:
        // If `$table` is a `SafeByteMatchTable`, it's guaranteed that `lexer.source`
        // will be positioned on a UTF-8 character boundary before `handle_match` is called.
        // If `$table` is a `ByteMatchTable`, no such guarantee is given, but call to
        // `$table.use_table()` here makes using this macro unsafe, and it's the user's
        // responsibility to uphold this invariant.
        // Therefore we can assume this is taken care of one way or another, and wrap the calls
        // to unsafe functions in this function with `unsafe {}`.
        $table.use_table();

        let mut $pos = $start;
        #[allow(unused_unsafe)] // Silence warnings if macro called in unsafe code
        'outer: loop {
            #[allow(clippy::redundant_else)]
            if $pos.addr() <= $lexer.source.end_for_batch_search_addr() {
                // Search a batch of `SEARCH_BATCH_SIZE` bytes.
                //
                // `'inner: loop {}` is not a real loop - it always exits on first turn.
                // Only using `loop {}` so that can use `break 'inner` to get out of it.
                // This allows complex logic of `$should_continue` and `$match_handler` to be
                // outside the `for` loop, keeping it as minimal as possible, to encourage
                // compiler to unroll it.
                //
                // SAFETY:
                // `$pos.addr() <= lexer.source.end_for_batch_search_addr()` check above ensures
                // there are at least `SEARCH_BATCH_SIZE` bytes remaining in `lexer.source`.
                // So calls to `$pos.read()` and `$pos.add(1)` in this loop cannot go out of bounds.
                let $match_byte = 'inner: loop {
                    for _i in 0..crate::lexer::search::SEARCH_BATCH_SIZE {
                        // SAFETY: `$pos` cannot go out of bounds in this loop (see above)
                        let byte = unsafe { $pos.read() };
                        if $table.matches(byte) {
                            break 'inner byte;
                        }

                        // No match - continue searching batch.
                        // SAFETY: `$pos` cannot go out of bounds in this loop (see above).
                        // Also see above about UTF-8 character boundaries invariant.
                        $pos = unsafe { $pos.add(1) };
                    }
                    // No match in batch - search next batch
                    continue 'outer;
                };

                // Found match. Check if should continue.
                {
                    let $continue_byte = $match_byte;
                    if $should_continue {
                        // Not a match after all - continue searching.
                        // SAFETY: `pos` is not at end of source, so safe to advance 1 byte.
                        // See above about UTF-8 character boundaries invariant.
                        $pos = unsafe { $pos.add(1) };
                        continue;
                    }
                }

                // Advance `lexer.source`'s position up to `$pos`, consuming unmatched bytes.
                // SAFETY: See above about UTF-8 character boundaries invariant.
                $lexer.source.set_position($pos);

                let $match_start = $start;
                return $match_handler;
            } else {
                // Not enough bytes remaining to process as a batch.
                // This branch marked `#[cold]` as should be very uncommon in normal-length JS files.
                // Very short JS files will be penalized, but they'll be very fast to parse anyway.
                // TODO: Could extend very short files with padding during parser initialization
                // to remove that problem.
                return crate::lexer::cold_branch(|| {
                    let end_addr = $lexer.source.end_addr();
                    while $pos.addr() < end_addr {
                        // SAFETY: `pos` is not at end of source, so safe to read a byte
                        let $match_byte = unsafe { $pos.read() };
                        if $table.matches($match_byte) {
                            // Found match.
                            // Check if should continue.
                            {
                                let $continue_byte = $match_byte;
                                if $should_continue {
                                    // Not a match after all - continue searching.
                                    // SAFETY: `pos` is not at end of source, so safe to advance 1 byte.
                                    // See above about UTF-8 character boundaries invariant.
                                    $pos = unsafe { $pos.add(1) };
                                    continue;
                                }
                            }

                            // Advance `lexer.source`'s position up to `pos`, consuming unmatched bytes.
                            // SAFETY: See above about UTF-8 character boundaries invariant.
                            $lexer.source.set_position($pos);

                            let $match_start = $start;
                            return $match_handler;
                        }

                        // No match - continue searching
                        // SAFETY: `pos` is not at end of source, so safe to advance 1 byte.
                        // See above about UTF-8 character boundaries invariant.
                        $pos = unsafe { $pos.add(1) };
                    }

                    // EOF.
                    // Advance `lexer.source`'s position to end of file.
                    $lexer.source.set_position($pos);

                    let $eof_start = $start;
                    $eof_handler
                });
            }
        }
    }};
}
pub(crate) use byte_search;
