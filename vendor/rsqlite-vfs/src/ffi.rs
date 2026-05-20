pub type sqlite3_int64 = ::core::ffi::c_longlong;
pub type sqlite3_filename = *const ::core::ffi::c_char;
pub type sqlite3_syscall_ptr = ::core::option::Option<unsafe extern "C" fn()>;

pub const SQLITE_OK: i32 = 0;
pub const SQLITE_ERROR: i32 = 1;
pub const SQLITE_INTERNAL: i32 = 2;
pub const SQLITE_PERM: i32 = 3;
pub const SQLITE_ABORT: i32 = 4;
pub const SQLITE_BUSY: i32 = 5;
pub const SQLITE_LOCKED: i32 = 6;
pub const SQLITE_NOMEM: i32 = 7;
pub const SQLITE_READONLY: i32 = 8;
pub const SQLITE_INTERRUPT: i32 = 9;
pub const SQLITE_IOERR: i32 = 10;
pub const SQLITE_CORRUPT: i32 = 11;
pub const SQLITE_NOTFOUND: i32 = 12;
pub const SQLITE_FULL: i32 = 13;
pub const SQLITE_CANTOPEN: i32 = 14;
pub const SQLITE_PROTOCOL: i32 = 15;
pub const SQLITE_EMPTY: i32 = 16;
pub const SQLITE_SCHEMA: i32 = 17;
pub const SQLITE_TOOBIG: i32 = 18;
pub const SQLITE_CONSTRAINT: i32 = 19;
pub const SQLITE_MISMATCH: i32 = 20;
pub const SQLITE_MISUSE: i32 = 21;
pub const SQLITE_NOLFS: i32 = 22;
pub const SQLITE_AUTH: i32 = 23;
pub const SQLITE_FORMAT: i32 = 24;
pub const SQLITE_RANGE: i32 = 25;
pub const SQLITE_NOTADB: i32 = 26;
pub const SQLITE_NOTICE: i32 = 27;
pub const SQLITE_WARNING: i32 = 28;
pub const SQLITE_ROW: i32 = 100;
pub const SQLITE_DONE: i32 = 101;
pub const SQLITE_ERROR_MISSING_COLLSEQ: i32 = 257;
pub const SQLITE_ERROR_RETRY: i32 = 513;
pub const SQLITE_ERROR_SNAPSHOT: i32 = 769;
pub const SQLITE_ERROR_RESERVESIZE: i32 = 1025;
pub const SQLITE_ERROR_KEY: i32 = 1281;
pub const SQLITE_ERROR_UNABLE: i32 = 1537;
pub const SQLITE_IOERR_READ: i32 = 266;
pub const SQLITE_IOERR_SHORT_READ: i32 = 522;
pub const SQLITE_IOERR_WRITE: i32 = 778;
pub const SQLITE_IOERR_FSYNC: i32 = 1034;
pub const SQLITE_IOERR_DIR_FSYNC: i32 = 1290;
pub const SQLITE_IOERR_TRUNCATE: i32 = 1546;
pub const SQLITE_IOERR_FSTAT: i32 = 1802;
pub const SQLITE_IOERR_UNLOCK: i32 = 2058;
pub const SQLITE_IOERR_RDLOCK: i32 = 2314;
pub const SQLITE_IOERR_DELETE: i32 = 2570;
pub const SQLITE_IOERR_BLOCKED: i32 = 2826;
pub const SQLITE_IOERR_NOMEM: i32 = 3082;
pub const SQLITE_IOERR_ACCESS: i32 = 3338;
pub const SQLITE_IOERR_CHECKRESERVEDLOCK: i32 = 3594;
pub const SQLITE_IOERR_LOCK: i32 = 3850;
pub const SQLITE_IOERR_CLOSE: i32 = 4106;
pub const SQLITE_IOERR_DIR_CLOSE: i32 = 4362;
pub const SQLITE_IOERR_SHMOPEN: i32 = 4618;
pub const SQLITE_IOERR_SHMSIZE: i32 = 4874;
pub const SQLITE_IOERR_SHMLOCK: i32 = 5130;
pub const SQLITE_IOERR_SHMMAP: i32 = 5386;
pub const SQLITE_IOERR_SEEK: i32 = 5642;
pub const SQLITE_IOERR_DELETE_NOENT: i32 = 5898;
pub const SQLITE_IOERR_MMAP: i32 = 6154;
pub const SQLITE_IOERR_GETTEMPPATH: i32 = 6410;
pub const SQLITE_IOERR_CONVPATH: i32 = 6666;
pub const SQLITE_IOERR_VNODE: i32 = 6922;
pub const SQLITE_IOERR_AUTH: i32 = 7178;
pub const SQLITE_IOERR_BEGIN_ATOMIC: i32 = 7434;
pub const SQLITE_IOERR_COMMIT_ATOMIC: i32 = 7690;
pub const SQLITE_IOERR_ROLLBACK_ATOMIC: i32 = 7946;
pub const SQLITE_IOERR_DATA: i32 = 8202;
pub const SQLITE_IOERR_CORRUPTFS: i32 = 8458;
pub const SQLITE_IOERR_IN_PAGE: i32 = 8714;
pub const SQLITE_IOERR_BADKEY: i32 = 8970;
pub const SQLITE_IOERR_CODEC: i32 = 9226;
pub const SQLITE_LOCKED_SHAREDCACHE: i32 = 262;
pub const SQLITE_LOCKED_VTAB: i32 = 518;
pub const SQLITE_BUSY_RECOVERY: i32 = 261;
pub const SQLITE_BUSY_SNAPSHOT: i32 = 517;
pub const SQLITE_BUSY_TIMEOUT: i32 = 773;
pub const SQLITE_CANTOPEN_NOTEMPDIR: i32 = 270;
pub const SQLITE_CANTOPEN_ISDIR: i32 = 526;
pub const SQLITE_CANTOPEN_FULLPATH: i32 = 782;
pub const SQLITE_CANTOPEN_CONVPATH: i32 = 1038;
pub const SQLITE_CANTOPEN_DIRTYWAL: i32 = 1294;
pub const SQLITE_CANTOPEN_SYMLINK: i32 = 1550;
pub const SQLITE_CORRUPT_VTAB: i32 = 267;
pub const SQLITE_CORRUPT_SEQUENCE: i32 = 523;
pub const SQLITE_CORRUPT_INDEX: i32 = 779;
pub const SQLITE_READONLY_RECOVERY: i32 = 264;
pub const SQLITE_READONLY_CANTLOCK: i32 = 520;
pub const SQLITE_READONLY_ROLLBACK: i32 = 776;
pub const SQLITE_READONLY_DBMOVED: i32 = 1032;
pub const SQLITE_READONLY_CANTINIT: i32 = 1288;
pub const SQLITE_READONLY_DIRECTORY: i32 = 1544;
pub const SQLITE_ABORT_ROLLBACK: i32 = 516;
pub const SQLITE_CONSTRAINT_CHECK: i32 = 275;
pub const SQLITE_CONSTRAINT_COMMITHOOK: i32 = 531;
pub const SQLITE_CONSTRAINT_FOREIGNKEY: i32 = 787;
pub const SQLITE_CONSTRAINT_FUNCTION: i32 = 1043;
pub const SQLITE_CONSTRAINT_NOTNULL: i32 = 1299;
pub const SQLITE_CONSTRAINT_PRIMARYKEY: i32 = 1555;
pub const SQLITE_CONSTRAINT_TRIGGER: i32 = 1811;
pub const SQLITE_CONSTRAINT_UNIQUE: i32 = 2067;
pub const SQLITE_CONSTRAINT_VTAB: i32 = 2323;
pub const SQLITE_CONSTRAINT_ROWID: i32 = 2579;
pub const SQLITE_CONSTRAINT_PINNED: i32 = 2835;
pub const SQLITE_CONSTRAINT_DATATYPE: i32 = 3091;
pub const SQLITE_NOTICE_RECOVER_WAL: i32 = 283;
pub const SQLITE_NOTICE_RECOVER_ROLLBACK: i32 = 539;
pub const SQLITE_NOTICE_RBU: i32 = 795;
pub const SQLITE_WARNING_AUTOINDEX: i32 = 284;
pub const SQLITE_AUTH_USER: i32 = 279;
pub const SQLITE_OK_LOAD_PERMANENTLY: i32 = 256;
pub const SQLITE_OK_SYMLINK: i32 = 512;
pub const SQLITE_OPEN_READONLY: i32 = 1;
pub const SQLITE_OPEN_READWRITE: i32 = 2;
pub const SQLITE_OPEN_CREATE: i32 = 4;
pub const SQLITE_OPEN_DELETEONCLOSE: i32 = 8;
pub const SQLITE_OPEN_EXCLUSIVE: i32 = 16;
pub const SQLITE_OPEN_AUTOPROXY: i32 = 32;
pub const SQLITE_OPEN_URI: i32 = 64;
pub const SQLITE_OPEN_MEMORY: i32 = 128;
pub const SQLITE_OPEN_MAIN_DB: i32 = 256;
pub const SQLITE_OPEN_TEMP_DB: i32 = 512;
pub const SQLITE_OPEN_TRANSIENT_DB: i32 = 1024;
pub const SQLITE_OPEN_MAIN_JOURNAL: i32 = 2048;
pub const SQLITE_OPEN_TEMP_JOURNAL: i32 = 4096;
pub const SQLITE_OPEN_SUBJOURNAL: i32 = 8192;
pub const SQLITE_OPEN_SUPER_JOURNAL: i32 = 16384;
pub const SQLITE_OPEN_NOMUTEX: i32 = 32768;
pub const SQLITE_OPEN_FULLMUTEX: i32 = 65536;
pub const SQLITE_OPEN_SHAREDCACHE: i32 = 131072;
pub const SQLITE_OPEN_PRIVATECACHE: i32 = 262144;
pub const SQLITE_OPEN_WAL: i32 = 524288;
pub const SQLITE_OPEN_NOFOLLOW: i32 = 16777216;
pub const SQLITE_OPEN_EXRESCODE: i32 = 33554432;
pub const SQLITE_OPEN_MASTER_JOURNAL: i32 = 16384;
pub const SQLITE_IOCAP_ATOMIC: i32 = 1;
pub const SQLITE_IOCAP_ATOMIC512: i32 = 2;
pub const SQLITE_IOCAP_ATOMIC1K: i32 = 4;
pub const SQLITE_IOCAP_ATOMIC2K: i32 = 8;
pub const SQLITE_IOCAP_ATOMIC4K: i32 = 16;
pub const SQLITE_IOCAP_ATOMIC8K: i32 = 32;
pub const SQLITE_IOCAP_ATOMIC16K: i32 = 64;
pub const SQLITE_IOCAP_ATOMIC32K: i32 = 128;
pub const SQLITE_IOCAP_ATOMIC64K: i32 = 256;
pub const SQLITE_IOCAP_SAFE_APPEND: i32 = 512;
pub const SQLITE_IOCAP_SEQUENTIAL: i32 = 1024;
pub const SQLITE_IOCAP_UNDELETABLE_WHEN_OPEN: i32 = 2048;
pub const SQLITE_IOCAP_POWERSAFE_OVERWRITE: i32 = 4096;
pub const SQLITE_IOCAP_IMMUTABLE: i32 = 8192;
pub const SQLITE_IOCAP_BATCH_ATOMIC: i32 = 16384;
pub const SQLITE_IOCAP_SUBPAGE_READ: i32 = 32768;
pub const SQLITE_LOCK_NONE: i32 = 0;
pub const SQLITE_LOCK_SHARED: i32 = 1;
pub const SQLITE_LOCK_RESERVED: i32 = 2;
pub const SQLITE_LOCK_PENDING: i32 = 3;
pub const SQLITE_LOCK_EXCLUSIVE: i32 = 4;
pub const SQLITE_SYNC_NORMAL: i32 = 2;
pub const SQLITE_SYNC_FULL: i32 = 3;
pub const SQLITE_SYNC_DATAONLY: i32 = 16;
pub const SQLITE_FCNTL_LOCKSTATE: i32 = 1;
pub const SQLITE_FCNTL_GET_LOCKPROXYFILE: i32 = 2;
pub const SQLITE_FCNTL_SET_LOCKPROXYFILE: i32 = 3;
pub const SQLITE_FCNTL_LAST_ERRNO: i32 = 4;
pub const SQLITE_FCNTL_SIZE_HINT: i32 = 5;
pub const SQLITE_FCNTL_CHUNK_SIZE: i32 = 6;
pub const SQLITE_FCNTL_FILE_POINTER: i32 = 7;
pub const SQLITE_FCNTL_SYNC_OMITTED: i32 = 8;
pub const SQLITE_FCNTL_WIN32_AV_RETRY: i32 = 9;
pub const SQLITE_FCNTL_PERSIST_WAL: i32 = 10;
pub const SQLITE_FCNTL_OVERWRITE: i32 = 11;
pub const SQLITE_FCNTL_VFSNAME: i32 = 12;
pub const SQLITE_FCNTL_POWERSAFE_OVERWRITE: i32 = 13;
pub const SQLITE_FCNTL_PRAGMA: i32 = 14;
pub const SQLITE_FCNTL_BUSYHANDLER: i32 = 15;
pub const SQLITE_FCNTL_TEMPFILENAME: i32 = 16;
pub const SQLITE_FCNTL_MMAP_SIZE: i32 = 18;
pub const SQLITE_FCNTL_TRACE: i32 = 19;
pub const SQLITE_FCNTL_HAS_MOVED: i32 = 20;
pub const SQLITE_FCNTL_SYNC: i32 = 21;
pub const SQLITE_FCNTL_COMMIT_PHASETWO: i32 = 22;
pub const SQLITE_FCNTL_WIN32_SET_HANDLE: i32 = 23;
pub const SQLITE_FCNTL_WAL_BLOCK: i32 = 24;
pub const SQLITE_FCNTL_ZIPVFS: i32 = 25;
pub const SQLITE_FCNTL_RBU: i32 = 26;
pub const SQLITE_FCNTL_VFS_POINTER: i32 = 27;
pub const SQLITE_FCNTL_JOURNAL_POINTER: i32 = 28;
pub const SQLITE_FCNTL_WIN32_GET_HANDLE: i32 = 29;
pub const SQLITE_FCNTL_PDB: i32 = 30;
pub const SQLITE_FCNTL_BEGIN_ATOMIC_WRITE: i32 = 31;
pub const SQLITE_FCNTL_COMMIT_ATOMIC_WRITE: i32 = 32;
pub const SQLITE_FCNTL_ROLLBACK_ATOMIC_WRITE: i32 = 33;
pub const SQLITE_FCNTL_LOCK_TIMEOUT: i32 = 34;
pub const SQLITE_FCNTL_DATA_VERSION: i32 = 35;
pub const SQLITE_FCNTL_SIZE_LIMIT: i32 = 36;
pub const SQLITE_FCNTL_CKPT_DONE: i32 = 37;
pub const SQLITE_FCNTL_RESERVE_BYTES: i32 = 38;
pub const SQLITE_FCNTL_CKPT_START: i32 = 39;
pub const SQLITE_FCNTL_EXTERNAL_READER: i32 = 40;
pub const SQLITE_FCNTL_CKSM_FILE: i32 = 41;
pub const SQLITE_FCNTL_RESET_CACHE: i32 = 42;
pub const SQLITE_FCNTL_NULL_IO: i32 = 43;
pub const SQLITE_FCNTL_BLOCK_ON_CONNECT: i32 = 44;
pub const SQLITE_FCNTL_FILESTAT: i32 = 45;
pub const SQLITE_GET_LOCKPROXYFILE: i32 = 2;
pub const SQLITE_SET_LOCKPROXYFILE: i32 = 3;
pub const SQLITE_LAST_ERRNO: i32 = 4;
pub const SQLITE_ACCESS_EXISTS: i32 = 0;
pub const SQLITE_ACCESS_READWRITE: i32 = 1;
pub const SQLITE_ACCESS_READ: i32 = 2;
pub const SQLITE_SHM_UNLOCK: i32 = 1;
pub const SQLITE_SHM_LOCK: i32 = 2;
pub const SQLITE_SHM_SHARED: i32 = 4;
pub const SQLITE_SHM_EXCLUSIVE: i32 = 8;
pub const SQLITE_SHM_NLOCK: i32 = 8;
pub const SQLITE_CONFIG_SINGLETHREAD: i32 = 1;
pub const SQLITE_CONFIG_MULTITHREAD: i32 = 2;
pub const SQLITE_CONFIG_SERIALIZED: i32 = 3;
pub const SQLITE_CONFIG_MALLOC: i32 = 4;
pub const SQLITE_CONFIG_GETMALLOC: i32 = 5;
pub const SQLITE_CONFIG_SCRATCH: i32 = 6;
pub const SQLITE_CONFIG_PAGECACHE: i32 = 7;
pub const SQLITE_CONFIG_HEAP: i32 = 8;
pub const SQLITE_CONFIG_MEMSTATUS: i32 = 9;
pub const SQLITE_CONFIG_MUTEX: i32 = 10;
pub const SQLITE_CONFIG_GETMUTEX: i32 = 11;
pub const SQLITE_CONFIG_LOOKASIDE: i32 = 13;
pub const SQLITE_CONFIG_PCACHE: i32 = 14;
pub const SQLITE_CONFIG_GETPCACHE: i32 = 15;
pub const SQLITE_CONFIG_LOG: i32 = 16;
pub const SQLITE_CONFIG_URI: i32 = 17;
pub const SQLITE_CONFIG_PCACHE2: i32 = 18;
pub const SQLITE_CONFIG_GETPCACHE2: i32 = 19;
pub const SQLITE_CONFIG_COVERING_INDEX_SCAN: i32 = 20;
pub const SQLITE_CONFIG_SQLLOG: i32 = 21;
pub const SQLITE_CONFIG_MMAP_SIZE: i32 = 22;
pub const SQLITE_CONFIG_WIN32_HEAPSIZE: i32 = 23;
pub const SQLITE_CONFIG_PCACHE_HDRSZ: i32 = 24;
pub const SQLITE_CONFIG_PMASZ: i32 = 25;
pub const SQLITE_CONFIG_STMTJRNL_SPILL: i32 = 26;
pub const SQLITE_CONFIG_SMALL_MALLOC: i32 = 27;
pub const SQLITE_CONFIG_SORTERREF_SIZE: i32 = 28;
pub const SQLITE_CONFIG_MEMDB_MAXSIZE: i32 = 29;
pub const SQLITE_CONFIG_ROWID_IN_VIEW: i32 = 30;
pub const SQLITE_DBCONFIG_MAINDBNAME: i32 = 1000;
pub const SQLITE_DBCONFIG_LOOKASIDE: i32 = 1001;
pub const SQLITE_DBCONFIG_ENABLE_FKEY: i32 = 1002;
pub const SQLITE_DBCONFIG_ENABLE_TRIGGER: i32 = 1003;
pub const SQLITE_DBCONFIG_ENABLE_FTS3_TOKENIZER: i32 = 1004;
pub const SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION: i32 = 1005;
pub const SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE: i32 = 1006;
pub const SQLITE_DBCONFIG_ENABLE_QPSG: i32 = 1007;
pub const SQLITE_DBCONFIG_TRIGGER_EQP: i32 = 1008;
pub const SQLITE_DBCONFIG_RESET_DATABASE: i32 = 1009;
pub const SQLITE_DBCONFIG_DEFENSIVE: i32 = 1010;
pub const SQLITE_DBCONFIG_WRITABLE_SCHEMA: i32 = 1011;
pub const SQLITE_DBCONFIG_LEGACY_ALTER_TABLE: i32 = 1012;
pub const SQLITE_DBCONFIG_DQS_DML: i32 = 1013;
pub const SQLITE_DBCONFIG_DQS_DDL: i32 = 1014;
pub const SQLITE_DBCONFIG_ENABLE_VIEW: i32 = 1015;
pub const SQLITE_DBCONFIG_LEGACY_FILE_FORMAT: i32 = 1016;
pub const SQLITE_DBCONFIG_TRUSTED_SCHEMA: i32 = 1017;
pub const SQLITE_DBCONFIG_STMT_SCANSTATUS: i32 = 1018;
pub const SQLITE_DBCONFIG_REVERSE_SCANORDER: i32 = 1019;
pub const SQLITE_DBCONFIG_ENABLE_ATTACH_CREATE: i32 = 1020;
pub const SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE: i32 = 1021;
pub const SQLITE_DBCONFIG_ENABLE_COMMENTS: i32 = 1022;
pub const SQLITE_DBCONFIG_MAX: i32 = 1022;
pub const SQLITE_SETLK_BLOCK_ON_CONNECT: i32 = 1;
pub const SQLITE_DENY: i32 = 1;
pub const SQLITE_IGNORE: i32 = 2;
pub const SQLITE_CREATE_INDEX: i32 = 1;
pub const SQLITE_CREATE_TABLE: i32 = 2;
pub const SQLITE_CREATE_TEMP_INDEX: i32 = 3;
pub const SQLITE_CREATE_TEMP_TABLE: i32 = 4;
pub const SQLITE_CREATE_TEMP_TRIGGER: i32 = 5;
pub const SQLITE_CREATE_TEMP_VIEW: i32 = 6;
pub const SQLITE_CREATE_TRIGGER: i32 = 7;
pub const SQLITE_CREATE_VIEW: i32 = 8;
pub const SQLITE_DELETE: i32 = 9;
pub const SQLITE_DROP_INDEX: i32 = 10;
pub const SQLITE_DROP_TABLE: i32 = 11;
pub const SQLITE_DROP_TEMP_INDEX: i32 = 12;
pub const SQLITE_DROP_TEMP_TABLE: i32 = 13;
pub const SQLITE_DROP_TEMP_TRIGGER: i32 = 14;
pub const SQLITE_DROP_TEMP_VIEW: i32 = 15;
pub const SQLITE_DROP_TRIGGER: i32 = 16;
pub const SQLITE_DROP_VIEW: i32 = 17;
pub const SQLITE_INSERT: i32 = 18;
pub const SQLITE_PRAGMA: i32 = 19;
pub const SQLITE_READ: i32 = 20;
pub const SQLITE_SELECT: i32 = 21;
pub const SQLITE_TRANSACTION: i32 = 22;
pub const SQLITE_UPDATE: i32 = 23;
pub const SQLITE_ATTACH: i32 = 24;
pub const SQLITE_DETACH: i32 = 25;
pub const SQLITE_ALTER_TABLE: i32 = 26;
pub const SQLITE_REINDEX: i32 = 27;
pub const SQLITE_ANALYZE: i32 = 28;
pub const SQLITE_CREATE_VTABLE: i32 = 29;
pub const SQLITE_DROP_VTABLE: i32 = 30;
pub const SQLITE_FUNCTION: i32 = 31;
pub const SQLITE_SAVEPOINT: i32 = 32;
pub const SQLITE_COPY: i32 = 0;
pub const SQLITE_RECURSIVE: i32 = 33;
pub const SQLITE_TRACE_STMT: ::core::ffi::c_uint = 1;
pub const SQLITE_TRACE_PROFILE: ::core::ffi::c_uint = 2;
pub const SQLITE_TRACE_ROW: ::core::ffi::c_uint = 4;
pub const SQLITE_TRACE_CLOSE: ::core::ffi::c_uint = 8;
pub const SQLITE_LIMIT_LENGTH: i32 = 0;
pub const SQLITE_LIMIT_SQL_LENGTH: i32 = 1;
pub const SQLITE_LIMIT_COLUMN: i32 = 2;
pub const SQLITE_LIMIT_EXPR_DEPTH: i32 = 3;
pub const SQLITE_LIMIT_COMPOUND_SELECT: i32 = 4;
pub const SQLITE_LIMIT_VDBE_OP: i32 = 5;
pub const SQLITE_LIMIT_FUNCTION_ARG: i32 = 6;
pub const SQLITE_LIMIT_ATTACHED: i32 = 7;
pub const SQLITE_LIMIT_LIKE_PATTERN_LENGTH: i32 = 8;
pub const SQLITE_LIMIT_VARIABLE_NUMBER: i32 = 9;
pub const SQLITE_LIMIT_TRIGGER_DEPTH: i32 = 10;
pub const SQLITE_LIMIT_WORKER_THREADS: i32 = 11;
pub const SQLITE_PREPARE_PERSISTENT: ::core::ffi::c_uint = 1;
pub const SQLITE_PREPARE_NORMALIZE: ::core::ffi::c_uint = 2;
pub const SQLITE_PREPARE_NO_VTAB: ::core::ffi::c_uint = 4;
pub const SQLITE_PREPARE_DONT_LOG: ::core::ffi::c_uint = 16;
pub const SQLITE_INTEGER: i32 = 1;
pub const SQLITE_FLOAT: i32 = 2;
pub const SQLITE_BLOB: i32 = 4;
pub const SQLITE_NULL: i32 = 5;
pub const SQLITE_TEXT: i32 = 3;
pub const SQLITE3_TEXT: i32 = 3;
pub const SQLITE_UTF8: i32 = 1;
pub const SQLITE_UTF16LE: i32 = 2;
pub const SQLITE_UTF16BE: i32 = 3;
pub const SQLITE_UTF16: i32 = 4;
pub const SQLITE_ANY: i32 = 5;
pub const SQLITE_UTF16_ALIGNED: i32 = 8;
pub const SQLITE_DETERMINISTIC: i32 = 2048;
pub const SQLITE_DIRECTONLY: i32 = 524288;
pub const SQLITE_SUBTYPE: i32 = 1048576;
pub const SQLITE_INNOCUOUS: i32 = 2097152;
pub const SQLITE_RESULT_SUBTYPE: i32 = 16777216;
pub const SQLITE_SELFORDER1: i32 = 33554432;
pub const SQLITE_WIN32_DATA_DIRECTORY_TYPE: i32 = 1;
pub const SQLITE_WIN32_TEMP_DIRECTORY_TYPE: i32 = 2;
pub const SQLITE_TXN_NONE: i32 = 0;
pub const SQLITE_TXN_READ: i32 = 1;
pub const SQLITE_TXN_WRITE: i32 = 2;
pub const SQLITE_INDEX_SCAN_UNIQUE: i32 = 1;
pub const SQLITE_INDEX_SCAN_HEX: i32 = 2;
pub const SQLITE_INDEX_CONSTRAINT_EQ: i32 = 2;
pub const SQLITE_INDEX_CONSTRAINT_GT: i32 = 4;
pub const SQLITE_INDEX_CONSTRAINT_LE: i32 = 8;
pub const SQLITE_INDEX_CONSTRAINT_LT: i32 = 16;
pub const SQLITE_INDEX_CONSTRAINT_GE: i32 = 32;
pub const SQLITE_INDEX_CONSTRAINT_MATCH: i32 = 64;
pub const SQLITE_INDEX_CONSTRAINT_LIKE: i32 = 65;
pub const SQLITE_INDEX_CONSTRAINT_GLOB: i32 = 66;
pub const SQLITE_INDEX_CONSTRAINT_REGEXP: i32 = 67;
pub const SQLITE_INDEX_CONSTRAINT_NE: i32 = 68;
pub const SQLITE_INDEX_CONSTRAINT_ISNOT: i32 = 69;
pub const SQLITE_INDEX_CONSTRAINT_ISNOTNULL: i32 = 70;
pub const SQLITE_INDEX_CONSTRAINT_ISNULL: i32 = 71;
pub const SQLITE_INDEX_CONSTRAINT_IS: i32 = 72;
pub const SQLITE_INDEX_CONSTRAINT_LIMIT: i32 = 73;
pub const SQLITE_INDEX_CONSTRAINT_OFFSET: i32 = 74;
pub const SQLITE_INDEX_CONSTRAINT_FUNCTION: i32 = 150;
pub const SQLITE_MUTEX_FAST: i32 = 0;
pub const SQLITE_MUTEX_RECURSIVE: i32 = 1;
pub const SQLITE_MUTEX_STATIC_MAIN: i32 = 2;
pub const SQLITE_MUTEX_STATIC_MEM: i32 = 3;
pub const SQLITE_MUTEX_STATIC_MEM2: i32 = 4;
pub const SQLITE_MUTEX_STATIC_OPEN: i32 = 4;
pub const SQLITE_MUTEX_STATIC_PRNG: i32 = 5;
pub const SQLITE_MUTEX_STATIC_LRU: i32 = 6;
pub const SQLITE_MUTEX_STATIC_LRU2: i32 = 7;
pub const SQLITE_MUTEX_STATIC_PMEM: i32 = 7;
pub const SQLITE_MUTEX_STATIC_APP1: i32 = 8;
pub const SQLITE_MUTEX_STATIC_APP2: i32 = 9;
pub const SQLITE_MUTEX_STATIC_APP3: i32 = 10;
pub const SQLITE_MUTEX_STATIC_VFS1: i32 = 11;
pub const SQLITE_MUTEX_STATIC_VFS2: i32 = 12;
pub const SQLITE_MUTEX_STATIC_VFS3: i32 = 13;
pub const SQLITE_MUTEX_STATIC_MASTER: i32 = 2;
pub const SQLITE_TESTCTRL_FIRST: i32 = 5;
pub const SQLITE_TESTCTRL_PRNG_SAVE: i32 = 5;
pub const SQLITE_TESTCTRL_PRNG_RESTORE: i32 = 6;
pub const SQLITE_TESTCTRL_PRNG_RESET: i32 = 7;
pub const SQLITE_TESTCTRL_FK_NO_ACTION: i32 = 7;
pub const SQLITE_TESTCTRL_BITVEC_TEST: i32 = 8;
pub const SQLITE_TESTCTRL_FAULT_INSTALL: i32 = 9;
pub const SQLITE_TESTCTRL_BENIGN_MALLOC_HOOKS: i32 = 10;
pub const SQLITE_TESTCTRL_PENDING_BYTE: i32 = 11;
pub const SQLITE_TESTCTRL_ASSERT: i32 = 12;
pub const SQLITE_TESTCTRL_ALWAYS: i32 = 13;
pub const SQLITE_TESTCTRL_RESERVE: i32 = 14;
pub const SQLITE_TESTCTRL_JSON_SELFCHECK: i32 = 14;
pub const SQLITE_TESTCTRL_OPTIMIZATIONS: i32 = 15;
pub const SQLITE_TESTCTRL_ISKEYWORD: i32 = 16;
pub const SQLITE_TESTCTRL_GETOPT: i32 = 16;
pub const SQLITE_TESTCTRL_SCRATCHMALLOC: i32 = 17;
pub const SQLITE_TESTCTRL_INTERNAL_FUNCTIONS: i32 = 17;
pub const SQLITE_TESTCTRL_LOCALTIME_FAULT: i32 = 18;
pub const SQLITE_TESTCTRL_EXPLAIN_STMT: i32 = 19;
pub const SQLITE_TESTCTRL_ONCE_RESET_THRESHOLD: i32 = 19;
pub const SQLITE_TESTCTRL_NEVER_CORRUPT: i32 = 20;
pub const SQLITE_TESTCTRL_VDBE_COVERAGE: i32 = 21;
pub const SQLITE_TESTCTRL_BYTEORDER: i32 = 22;
pub const SQLITE_TESTCTRL_ISINIT: i32 = 23;
pub const SQLITE_TESTCTRL_SORTER_MMAP: i32 = 24;
pub const SQLITE_TESTCTRL_IMPOSTER: i32 = 25;
pub const SQLITE_TESTCTRL_PARSER_COVERAGE: i32 = 26;
pub const SQLITE_TESTCTRL_RESULT_INTREAL: i32 = 27;
pub const SQLITE_TESTCTRL_PRNG_SEED: i32 = 28;
pub const SQLITE_TESTCTRL_EXTRA_SCHEMA_CHECKS: i32 = 29;
pub const SQLITE_TESTCTRL_SEEK_COUNT: i32 = 30;
pub const SQLITE_TESTCTRL_TRACEFLAGS: i32 = 31;
pub const SQLITE_TESTCTRL_TUNE: i32 = 32;
pub const SQLITE_TESTCTRL_LOGEST: i32 = 33;
pub const SQLITE_TESTCTRL_USELONGDOUBLE: i32 = 34;
pub const SQLITE_TESTCTRL_LAST: i32 = 34;
pub const SQLITE_STATUS_MEMORY_USED: i32 = 0;
pub const SQLITE_STATUS_PAGECACHE_USED: i32 = 1;
pub const SQLITE_STATUS_PAGECACHE_OVERFLOW: i32 = 2;
pub const SQLITE_STATUS_SCRATCH_USED: i32 = 3;
pub const SQLITE_STATUS_SCRATCH_OVERFLOW: i32 = 4;
pub const SQLITE_STATUS_MALLOC_SIZE: i32 = 5;
pub const SQLITE_STATUS_PARSER_STACK: i32 = 6;
pub const SQLITE_STATUS_PAGECACHE_SIZE: i32 = 7;
pub const SQLITE_STATUS_SCRATCH_SIZE: i32 = 8;
pub const SQLITE_STATUS_MALLOC_COUNT: i32 = 9;
pub const SQLITE_DBSTATUS_LOOKASIDE_USED: i32 = 0;
pub const SQLITE_DBSTATUS_CACHE_USED: i32 = 1;
pub const SQLITE_DBSTATUS_SCHEMA_USED: i32 = 2;
pub const SQLITE_DBSTATUS_STMT_USED: i32 = 3;
pub const SQLITE_DBSTATUS_LOOKASIDE_HIT: i32 = 4;
pub const SQLITE_DBSTATUS_LOOKASIDE_MISS_SIZE: i32 = 5;
pub const SQLITE_DBSTATUS_LOOKASIDE_MISS_FULL: i32 = 6;
pub const SQLITE_DBSTATUS_CACHE_HIT: i32 = 7;
pub const SQLITE_DBSTATUS_CACHE_MISS: i32 = 8;
pub const SQLITE_DBSTATUS_CACHE_WRITE: i32 = 9;
pub const SQLITE_DBSTATUS_DEFERRED_FKS: i32 = 10;
pub const SQLITE_DBSTATUS_CACHE_USED_SHARED: i32 = 11;
pub const SQLITE_DBSTATUS_CACHE_SPILL: i32 = 12;
pub const SQLITE_DBSTATUS_TEMPBUF_SPILL: i32 = 13;
pub const SQLITE_DBSTATUS_MAX: i32 = 13;
pub const SQLITE_STMTSTATUS_FULLSCAN_STEP: i32 = 1;
pub const SQLITE_STMTSTATUS_SORT: i32 = 2;
pub const SQLITE_STMTSTATUS_AUTOINDEX: i32 = 3;
pub const SQLITE_STMTSTATUS_VM_STEP: i32 = 4;
pub const SQLITE_STMTSTATUS_REPREPARE: i32 = 5;
pub const SQLITE_STMTSTATUS_RUN: i32 = 6;
pub const SQLITE_STMTSTATUS_FILTER_MISS: i32 = 7;
pub const SQLITE_STMTSTATUS_FILTER_HIT: i32 = 8;
pub const SQLITE_STMTSTATUS_MEMUSED: i32 = 99;
pub const SQLITE_CHECKPOINT_NOOP: i32 = -1;
pub const SQLITE_CHECKPOINT_PASSIVE: i32 = 0;
pub const SQLITE_CHECKPOINT_FULL: i32 = 1;
pub const SQLITE_CHECKPOINT_RESTART: i32 = 2;
pub const SQLITE_CHECKPOINT_TRUNCATE: i32 = 3;
pub const SQLITE_VTAB_CONSTRAINT_SUPPORT: i32 = 1;
pub const SQLITE_VTAB_INNOCUOUS: i32 = 2;
pub const SQLITE_VTAB_DIRECTONLY: i32 = 3;
pub const SQLITE_VTAB_USES_ALL_SCHEMAS: i32 = 4;
pub const SQLITE_ROLLBACK: i32 = 1;
pub const SQLITE_FAIL: i32 = 3;
pub const SQLITE_REPLACE: i32 = 5;
pub const SQLITE_SCANSTAT_NLOOP: i32 = 0;
pub const SQLITE_SCANSTAT_NVISIT: i32 = 1;
pub const SQLITE_SCANSTAT_EST: i32 = 2;
pub const SQLITE_SCANSTAT_NAME: i32 = 3;
pub const SQLITE_SCANSTAT_EXPLAIN: i32 = 4;
pub const SQLITE_SCANSTAT_SELECTID: i32 = 5;
pub const SQLITE_SCANSTAT_PARENTID: i32 = 6;
pub const SQLITE_SCANSTAT_NCYCLE: i32 = 7;
pub const SQLITE_SCANSTAT_COMPLEX: i32 = 1;
pub const SQLITE_SERIALIZE_NOCOPY: ::core::ffi::c_uint = 1;
pub const SQLITE_DESERIALIZE_FREEONCLOSE: ::core::ffi::c_uint = 1;
pub const SQLITE_DESERIALIZE_RESIZEABLE: ::core::ffi::c_uint = 2;
pub const SQLITE_DESERIALIZE_READONLY: ::core::ffi::c_uint = 4;
pub const SQLITE_CARRAY_INT32: i32 = 0;
pub const SQLITE_CARRAY_INT64: i32 = 1;
pub const SQLITE_CARRAY_DOUBLE: i32 = 2;
pub const SQLITE_CARRAY_TEXT: i32 = 3;
pub const SQLITE_CARRAY_BLOB: i32 = 4;
pub const CARRAY_INT32: i32 = 0;
pub const CARRAY_INT64: i32 = 1;
pub const CARRAY_DOUBLE: i32 = 2;
pub const CARRAY_TEXT: i32 = 3;
pub const CARRAY_BLOB: i32 = 4;
pub const NOT_WITHIN: i32 = 0;
pub const PARTLY_WITHIN: i32 = 1;
pub const FULLY_WITHIN: i32 = 2;
pub const SQLITE_SESSION_OBJCONFIG_SIZE: i32 = 1;
pub const SQLITE_SESSION_OBJCONFIG_ROWID: i32 = 2;
pub const SQLITE_CHANGESETSTART_INVERT: i32 = 2;
pub const SQLITE_CHANGESETAPPLY_NOSAVEPOINT: i32 = 1;
pub const SQLITE_CHANGESETAPPLY_INVERT: i32 = 2;
pub const SQLITE_CHANGESETAPPLY_IGNORENOOP: i32 = 4;
pub const SQLITE_CHANGESETAPPLY_FKNOACTION: i32 = 8;
pub const SQLITE_CHANGESET_DATA: i32 = 1;
pub const SQLITE_CHANGESET_NOTFOUND: i32 = 2;
pub const SQLITE_CHANGESET_CONFLICT: i32 = 3;
pub const SQLITE_CHANGESET_CONSTRAINT: i32 = 4;
pub const SQLITE_CHANGESET_FOREIGN_KEY: i32 = 5;
pub const SQLITE_CHANGESET_OMIT: i32 = 0;
pub const SQLITE_CHANGESET_REPLACE: i32 = 1;
pub const SQLITE_CHANGESET_ABORT: i32 = 2;
pub const SQLITE_SESSION_CONFIG_STRMSIZE: i32 = 1;
pub const FTS5_TOKENIZE_QUERY: i32 = 1;
pub const FTS5_TOKENIZE_PREFIX: i32 = 2;
pub const FTS5_TOKENIZE_DOCUMENT: i32 = 4;
pub const FTS5_TOKENIZE_AUX: i32 = 8;
pub const FTS5_TOKEN_COLOCATED: i32 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct sqlite3_file {
    pub pMethods: *const sqlite3_io_methods,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct sqlite3_io_methods {
    pub iVersion: ::core::ffi::c_int,
    pub xClose: ::core::option::Option<
        unsafe extern "C" fn(arg1: *mut sqlite3_file) -> ::core::ffi::c_int,
    >,
    pub xRead: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            arg2: *mut ::core::ffi::c_void,
            iAmt: ::core::ffi::c_int,
            iOfst: sqlite3_int64,
        ) -> ::core::ffi::c_int,
    >,
    pub xWrite: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            arg2: *const ::core::ffi::c_void,
            iAmt: ::core::ffi::c_int,
            iOfst: sqlite3_int64,
        ) -> ::core::ffi::c_int,
    >,
    pub xTruncate: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            size: sqlite3_int64,
        ) -> ::core::ffi::c_int,
    >,
    pub xSync: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            flags: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xFileSize: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            pSize: *mut sqlite3_int64,
        ) -> ::core::ffi::c_int,
    >,
    pub xLock: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            arg2: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xUnlock: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            arg2: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xCheckReservedLock: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            pResOut: *mut ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xFileControl: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            op: ::core::ffi::c_int,
            pArg: *mut ::core::ffi::c_void,
        ) -> ::core::ffi::c_int,
    >,
    pub xSectorSize: ::core::option::Option<
        unsafe extern "C" fn(arg1: *mut sqlite3_file) -> ::core::ffi::c_int,
    >,
    pub xDeviceCharacteristics: ::core::option::Option<
        unsafe extern "C" fn(arg1: *mut sqlite3_file) -> ::core::ffi::c_int,
    >,
    pub xShmMap: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            iPg: ::core::ffi::c_int,
            pgsz: ::core::ffi::c_int,
            arg2: ::core::ffi::c_int,
            arg3: *mut *mut ::core::ffi::c_void,
        ) -> ::core::ffi::c_int,
    >,
    pub xShmLock: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            offset: ::core::ffi::c_int,
            n: ::core::ffi::c_int,
            flags: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xShmBarrier: ::core::option::Option<
        unsafe extern "C" fn(arg1: *mut sqlite3_file),
    >,
    pub xShmUnmap: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            deleteFlag: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xFetch: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            iOfst: sqlite3_int64,
            iAmt: ::core::ffi::c_int,
            pp: *mut *mut ::core::ffi::c_void,
        ) -> ::core::ffi::c_int,
    >,
    pub xUnfetch: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_file,
            iOfst: sqlite3_int64,
            p: *mut ::core::ffi::c_void,
        ) -> ::core::ffi::c_int,
    >,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct sqlite3_vfs {
    pub iVersion: ::core::ffi::c_int,
    pub szOsFile: ::core::ffi::c_int,
    pub mxPathname: ::core::ffi::c_int,
    pub pNext: *mut sqlite3_vfs,
    pub zName: *const ::core::ffi::c_char,
    pub pAppData: *mut ::core::ffi::c_void,
    pub xOpen: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: sqlite3_filename,
            arg2: *mut sqlite3_file,
            flags: ::core::ffi::c_int,
            pOutFlags: *mut ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xDelete: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: *const ::core::ffi::c_char,
            syncDir: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xAccess: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: *const ::core::ffi::c_char,
            flags: ::core::ffi::c_int,
            pResOut: *mut ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xFullPathname: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: *const ::core::ffi::c_char,
            nOut: ::core::ffi::c_int,
            zOut: *mut ::core::ffi::c_char,
        ) -> ::core::ffi::c_int,
    >,
    pub xDlOpen: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zFilename: *const ::core::ffi::c_char,
        ) -> *mut ::core::ffi::c_void,
    >,
    pub xDlError: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            nByte: ::core::ffi::c_int,
            zErrMsg: *mut ::core::ffi::c_char,
        ),
    >,
    pub xDlSym: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            arg2: *mut ::core::ffi::c_void,
            zSymbol: *const ::core::ffi::c_char,
        ) -> ::core::option::Option<
            unsafe extern "C" fn(
                arg1: *mut sqlite3_vfs,
                arg2: *mut ::core::ffi::c_void,
                zSymbol: *const ::core::ffi::c_char,
            ),
        >,
    >,
    pub xDlClose: ::core::option::Option<
        unsafe extern "C" fn(arg1: *mut sqlite3_vfs, arg2: *mut ::core::ffi::c_void),
    >,
    pub xRandomness: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            nByte: ::core::ffi::c_int,
            zOut: *mut ::core::ffi::c_char,
        ) -> ::core::ffi::c_int,
    >,
    pub xSleep: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            microseconds: ::core::ffi::c_int,
        ) -> ::core::ffi::c_int,
    >,
    pub xCurrentTime: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            arg2: *mut f64,
        ) -> ::core::ffi::c_int,
    >,
    pub xGetLastError: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            arg2: ::core::ffi::c_int,
            arg3: *mut ::core::ffi::c_char,
        ) -> ::core::ffi::c_int,
    >,
    pub xCurrentTimeInt64: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            arg2: *mut sqlite3_int64,
        ) -> ::core::ffi::c_int,
    >,
    pub xSetSystemCall: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: *const ::core::ffi::c_char,
            arg2: sqlite3_syscall_ptr,
        ) -> ::core::ffi::c_int,
    >,
    pub xGetSystemCall: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: *const ::core::ffi::c_char,
        ) -> sqlite3_syscall_ptr,
    >,
    pub xNextSystemCall: ::core::option::Option<
        unsafe extern "C" fn(
            arg1: *mut sqlite3_vfs,
            zName: *const ::core::ffi::c_char,
        ) -> *const ::core::ffi::c_char,
    >,
}

unsafe extern "C" {
    pub fn sqlite3_vfs_find(zVfsName: *const ::core::ffi::c_char) -> *mut sqlite3_vfs;
}

unsafe extern "C" {
    pub fn sqlite3_vfs_register(
        arg1: *mut sqlite3_vfs,
        makeDflt: ::core::ffi::c_int,
    ) -> ::core::ffi::c_int;
}

unsafe extern "C" {
    pub fn sqlite3_vfs_unregister(arg1: *mut sqlite3_vfs) -> ::core::ffi::c_int;
}
