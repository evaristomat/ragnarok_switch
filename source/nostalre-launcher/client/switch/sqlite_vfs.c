/* In-process database locks for the three embedded servers. GPL-3.0-or-later.
 * Horizon file I/O uses libnx directly; host tests also support unix-none.
 * All database connections in the
 * NRO must share this one SQLite library and this VFS instance.
 */
#include "sqlite3.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#ifdef __SWITCH__
#include <switch.h>
#endif

typedef struct LockState LockState;
typedef struct NostalreFile NostalreFile;
struct LockState {
    char *name;
    unsigned refs, readers;
    NostalreFile *reserved, *pending, *exclusive;
    sqlite3_file *real;
    pthread_mutex_t io_mutex;
    int open_flags;
    LockState *next;
};
struct NostalreFile {
    sqlite3_file base;
    sqlite3_file *real;
    LockState *shared;
    int lock;
    pthread_mutex_t io_mutex;
};
static pthread_mutex_t locks_mutex = PTHREAD_MUTEX_INITIALIZER;
static LockState *locks;
static sqlite3_vfs vfs;
static sqlite3_vfs *underlying;

static int unlock_file(sqlite3_file *file, int level) {
    NostalreFile *f = (NostalreFile *)file;
    pthread_mutex_lock(&locks_mutex);
    LockState *s = f->shared;
    if (s) {
        if (level < SQLITE_LOCK_EXCLUSIVE && s->exclusive == f) s->exclusive = NULL;
        if (level < SQLITE_LOCK_PENDING && s->pending == f) s->pending = NULL;
        if (level < SQLITE_LOCK_RESERVED && s->reserved == f) s->reserved = NULL;
        if (level == SQLITE_LOCK_NONE && f->lock >= SQLITE_LOCK_SHARED) --s->readers;
    }
    f->lock = level;
    pthread_mutex_unlock(&locks_mutex);
    return SQLITE_OK;
}
static int close_file(sqlite3_file *file) {
    NostalreFile *f = (NostalreFile *)file;
    unlock_file(file, SQLITE_LOCK_NONE);
    int rc = SQLITE_OK;
    pthread_mutex_lock(&locks_mutex);
    if (f->shared && --f->shared->refs == 0) {
        LockState **p = &locks;
        while (*p && *p != f->shared) p = &(*p)->next;
        if (*p) *p = f->shared->next;
        rc = f->real->pMethods->xClose(f->real);
        pthread_mutex_destroy(&f->shared->io_mutex);
        free(f->shared->real);
        free(f->shared->name); free(f->shared);
    }
    if (!f->shared) rc = f->real->pMethods->xClose(f->real);
    pthread_mutex_unlock(&locks_mutex);
    pthread_mutex_destroy(&f->io_mutex);
    f->base.pMethods = NULL;
    return rc;
}
/* unix-none may implement read/write with lseek plus read/write. Connections
   share one physical handle, so every underlying operation is serialized. */
#define IO_CALL(file, method, ...) \
    NostalreFile *io_file = (NostalreFile *)(file); \
    pthread_mutex_t *mutex = io_file->shared ? &io_file->shared->io_mutex : &io_file->io_mutex; \
    pthread_mutex_lock(mutex); \
    int rc = io_file->real->pMethods->method(io_file->real, ##__VA_ARGS__); \
    pthread_mutex_unlock(mutex); \
    return rc
static int read_file(sqlite3_file *f, void *p, int n, sqlite3_int64 offset) {
    IO_CALL(f, xRead, p, n, offset);
}
static int write_file(sqlite3_file *f, const void *p, int n, sqlite3_int64 offset) {
    IO_CALL(f, xWrite, p, n, offset);
}
static int truncate_file(sqlite3_file *f, sqlite3_int64 n) {
    IO_CALL(f, xTruncate, n);
}
static int sync_file(sqlite3_file *f, int flags) {
    IO_CALL(f, xSync, flags);
}
static int size_file(sqlite3_file *f, sqlite3_int64 *n) {
    IO_CALL(f, xFileSize, n);
}
static int lock_file(sqlite3_file *file, int level) {
    NostalreFile *f = (NostalreFile *)file;
    int rc = SQLITE_OK;
    pthread_mutex_lock(&locks_mutex);
    LockState *s = f->shared;
    if (!s || level <= f->lock) goto done;
    if (f->lock == SQLITE_LOCK_NONE) {
        if ((s->pending && s->pending != f) || (s->exclusive && s->exclusive != f)) {
            rc = SQLITE_BUSY; goto done;
        }
        ++s->readers; f->lock = SQLITE_LOCK_SHARED;
    }
    if (level >= SQLITE_LOCK_RESERVED) {
        if (s->reserved && s->reserved != f) { rc = SQLITE_BUSY; goto done; }
        s->reserved = f; f->lock = SQLITE_LOCK_RESERVED;
    }
    if (level >= SQLITE_LOCK_PENDING) {
        if (s->pending && s->pending != f) { rc = SQLITE_BUSY; goto done; }
        s->pending = f; f->lock = SQLITE_LOCK_PENDING;
    }
    if (level == SQLITE_LOCK_EXCLUSIVE) {
        if (s->readers != 1) { rc = SQLITE_BUSY; goto done; }
        s->exclusive = f; f->lock = SQLITE_LOCK_EXCLUSIVE;
    }
done:
    pthread_mutex_unlock(&locks_mutex);
    return rc;
}
static int check_reserved(sqlite3_file *file, int *reserved) {
    NostalreFile *f = (NostalreFile *)file;
    pthread_mutex_lock(&locks_mutex);
    *reserved = f->shared && (f->shared->reserved || f->shared->pending || f->shared->exclusive);
    pthread_mutex_unlock(&locks_mutex);
    return SQLITE_OK;
}
static int control_file(sqlite3_file *file, int op, void *arg) {
    if (op == SQLITE_FCNTL_LOCKSTATE) { *(int *)arg = ((NostalreFile *)file)->lock; return SQLITE_OK; }
    IO_CALL(file, xFileControl, op, arg);
}
static int sector_file(sqlite3_file *file) {
    IO_CALL(file, xSectorSize);
}
static int characteristics(sqlite3_file *file) {
    IO_CALL(file, xDeviceCharacteristics);
}
static const sqlite3_io_methods methods = {
    .iVersion = 1, .xClose = close_file, .xRead = read_file, .xWrite = write_file,
    .xTruncate = truncate_file, .xSync = sync_file, .xFileSize = size_file,
    .xLock = lock_file, .xUnlock = unlock_file, .xCheckReservedLock = check_reserved,
    .xFileControl = control_file, .xSectorSize = sector_file,
    .xDeviceCharacteristics = characteristics,
};
static int open_file(sqlite3_vfs *v, const char *name, sqlite3_file *file, int flags, int *out) {
    (void)v;
    NostalreFile *f = (NostalreFile *)file;
    memset(f, 0, sizeof(*f));
    pthread_mutex_init(&f->io_mutex, NULL);
    f->real = (sqlite3_file *)(f + 1);
    int rc = SQLITE_OK;
    if (name && (flags & SQLITE_OPEN_MAIN_DB)) {
        pthread_mutex_lock(&locks_mutex);
        LockState *s;
        for (s = locks; s && strcmp(name, s->name); s = s->next) {}
        if (!s) {
            s = calloc(1, sizeof(*s));
            if (s) {
                s->name = strdup(name);
                s->real = calloc(1, underlying->szOsFile);
                if (!s->name || !s->real) { free(s->name); free(s->real); free(s); s = NULL; }
                else {
                    pthread_mutex_init(&s->io_mutex, NULL);
                    rc = underlying->xOpen(underlying, name, s->real, flags, &s->open_flags);
                    if (rc != SQLITE_OK) {
                        if (s->real->pMethods) s->real->pMethods->xClose(s->real);
                        pthread_mutex_destroy(&s->io_mutex);
                        free(s->name); free(s->real); free(s); s = NULL;
                    } else { s->next = locks; locks = s; }
                }
            }
        }
        if (s && (flags & SQLITE_OPEN_READWRITE) && !(s->open_flags & SQLITE_OPEN_READWRITE)) {
            rc = SQLITE_READONLY;
            if (!s->refs) {
                locks = s->next;
                s->real->pMethods->xClose(s->real);
                pthread_mutex_destroy(&s->io_mutex);
                free(s->name); free(s->real); free(s); s = NULL;
            }
        } else if (s) {
            ++s->refs; f->shared = s; f->real = s->real;
            if (out) *out = (s->open_flags & ~(SQLITE_OPEN_READONLY | SQLITE_OPEN_READWRITE))
                | (flags & (SQLITE_OPEN_READONLY | SQLITE_OPEN_READWRITE));
        }
        pthread_mutex_unlock(&locks_mutex);
        if (!s && rc == SQLITE_OK) rc = SQLITE_NOMEM;
    } else {
        rc = underlying->xOpen(underlying, name, f->real, flags, out);
        if (rc != SQLITE_OK && f->real->pMethods) f->real->pMethods->xClose(f->real);
    }
    if (rc != SQLITE_OK) {
        int saved_errno = errno;
#ifdef __SWITCH__
        unsigned result = fsdevGetLastResult();
#else
        unsigned result = 0;
#endif
        fprintf(stderr, "[SQLite] open failed: %s, rc=%d errno=%d fs=%08x\n", name ? name : "temporary", rc, saved_errno, result);
        pthread_mutex_destroy(&f->io_mutex);
        return rc;
    }
    f->base.pMethods = &methods;
    return SQLITE_OK;
}
static int delete_file(sqlite3_vfs *v, const char *name, int sync) {
    (void)v; return underlying->xDelete(underlying, name, sync);
}
static int access_file(sqlite3_vfs *v, const char *name, int flags, int *result) {
    (void)v; return underlying->xAccess(underlying, name, flags, result);
}
static int path_file(sqlite3_vfs *v, const char *name, int n, char *result) {
    /* libnx device paths are absolute. The Unix VFS instead prepends getcwd()
       and corrupts sdmc:/... into /sdmc:/.../sdmc:/..., before any file I/O. */
    if (strncmp(name, "sdmc:/", 6) == 0) {
        size_t length = strlen(name);
        if (n <= 0 || length >= (size_t)n) return SQLITE_CANTOPEN;
        memcpy(result, name, length + 1);
        return SQLITE_OK;
    }
    (void)v; return underlying->xFullPathname(underlying, name, n, result);
}
static int randomness(sqlite3_vfs *v, int n, char *result) {
    (void)v; return underlying->xRandomness(underlying, n, result);
}
static int sleep_vfs(sqlite3_vfs *v, int microseconds) {
    (void)v; return underlying->xSleep(underlying, microseconds);
}
static int time_vfs(sqlite3_vfs *v, double *result) {
    (void)v; return underlying->xCurrentTime(underlying, result);
}
#ifdef __SWITCH__
extern sqlite3_vfs *nostalre_horizon_io(sqlite3_vfs *);
#endif
int nostalre_sqlite_init(void) {
    int rc = sqlite3_initialize();
    if (rc != SQLITE_OK) return rc;
    underlying = sqlite3_vfs_find("unix-none");
    if (!underlying) return SQLITE_NOTFOUND;
#ifdef __SWITCH__
    underlying = nostalre_horizon_io(underlying);
    if (!underlying) return SQLITE_CANTOPEN;
#endif
    memset(&vfs, 0, sizeof(vfs));
    vfs.iVersion = 1;
    vfs.szOsFile = sizeof(NostalreFile) + underlying->szOsFile;
    vfs.mxPathname = underlying->mxPathname;
    vfs.zName = "nostalre";
    vfs.xOpen = open_file; vfs.xDelete = delete_file; vfs.xAccess = access_file;
    vfs.xFullPathname = path_file; vfs.xRandomness = randomness;
    vfs.xSleep = sleep_vfs; vfs.xCurrentTime = time_vfs;
    return sqlite3_vfs_register(&vfs, 0);
}

/* Test COMMIT on a private file in the same directory. ROLLBACK on the real
   save does not exercise journal/database fsync, so it alone cannot prove
   that the filesystem can persist a newly created character. */
static int check_commit(const char *path, char *error, size_t capacity) {
    size_t length = strlen(path) + 48;
    char *probe_path = malloc(length);
    char *journal_path = malloc(length + 16);
    if (!probe_path || !journal_path) {
        free(probe_path); free(journal_path); return SQLITE_NOMEM;
    }
    int fd = -1;
    for (int attempt = 0; attempt < 8; ++attempt) {
        unsigned long long nonce;
        sqlite3_randomness(sizeof(nonce), &nonce);
        snprintf(probe_path, length, "%s.write-check-%016llx", path, nonce);
        fd = open(probe_path, O_CREAT | O_EXCL | O_RDWR, 0600);
        if (fd >= 0 || errno != EEXIST) break;
    }
    if (fd < 0) {
        if (capacity) snprintf(error, capacity, "Nao foi possivel criar teste de gravacao (errno %d)", errno);
        free(probe_path); free(journal_path); return SQLITE_CANTOPEN;
    }
    close(fd);
    snprintf(journal_path, length + 16, "%s-journal", probe_path);
    sqlite3 *db = NULL;
    sqlite3_stmt *statement = NULL;
    int rc = sqlite3_open_v2(probe_path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX, "nostalre");
    if (rc == SQLITE_OK) rc = sqlite3_exec(db,
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;"
        "BEGIN IMMEDIATE; CREATE TABLE write_check(value INTEGER);"
        "INSERT INTO write_check VALUES(87531); COMMIT;", NULL, NULL, NULL);
    if (rc != SQLITE_OK && capacity) snprintf(error, capacity,
        "Falha ao confirmar gravacao no SD: SQLite %d (%s)",
        db ? sqlite3_extended_errcode(db) : rc, db ? sqlite3_errmsg(db) : "allocation failed");
    if (db) { sqlite3_close(db); db = NULL; }
    if (rc == SQLITE_OK) {
        rc = sqlite3_open_v2(probe_path, &db, SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, "nostalre");
        if (rc == SQLITE_OK) rc = sqlite3_prepare_v2(db, "SELECT value FROM write_check", -1, &statement, NULL);
        if (rc == SQLITE_OK) {
            int step = sqlite3_step(statement);
            if (step != SQLITE_ROW || sqlite3_column_int(statement, 0) != 87531) rc = SQLITE_IOERR;
        }
        if (rc != SQLITE_OK && capacity) snprintf(error, capacity,
            "Falha ao reabrir gravacao no SD: SQLite %d (%s)", rc, db ? sqlite3_errmsg(db) : "allocation failed");
    }
    if (statement) sqlite3_finalize(statement);
    if (db) sqlite3_close(db);
    int cleanup = vfs.xDelete(&vfs, journal_path, 0);
    if (cleanup == SQLITE_OK || cleanup == SQLITE_IOERR_DELETE_NOENT) cleanup = vfs.xDelete(&vfs, probe_path, 0);
    if (rc == SQLITE_OK && cleanup != SQLITE_OK && cleanup != SQLITE_IOERR_DELETE_NOENT) {
        rc = cleanup;
        if (capacity) snprintf(error, capacity, "Falha ao remover arquivo temporario de teste: SQLite %d", rc);
    }
    free(probe_path); free(journal_path);
    return rc;
}

/* Validate the existing save before starting embedded workers. A filesystem
   error can then be reported on screen without taking down the process. */
int nostalre_sqlite_check_database(const char *path, char *error, size_t capacity) {
    sqlite3 *db = NULL;
    sqlite3 *second = NULL;
    sqlite3_stmt *statement = NULL;
    int rc = sqlite3_open_v2(path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX, "nostalre");
    if (rc == SQLITE_OK) rc = sqlite3_prepare_v2(db, "PRAGMA quick_check", -1, &statement, NULL);
    if (rc == SQLITE_OK) {
        int step = sqlite3_step(statement);
        if (step != SQLITE_ROW) rc = step == SQLITE_DONE ? SQLITE_CORRUPT : step;
        else if (strcmp((const char *)sqlite3_column_text(statement, 0), "ok")) rc = SQLITE_CORRUPT;
    }
    if (statement) { sqlite3_finalize(statement); statement = NULL; }
    if (rc == SQLITE_OK) {
        rc = sqlite3_open_v2(path, &second, SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX, "nostalre");
        if (rc == SQLITE_OK) rc = sqlite3_exec(second, "SELECT count(*) FROM sqlite_master", NULL, NULL, NULL);
    }
    /* A readable database does not prove its rollback journal can be opened.
       Exercise a real write, then roll it back before any server starts. */
    if (rc == SQLITE_OK) rc = sqlite3_exec(db,
        "BEGIN IMMEDIATE;"
        "CREATE TABLE __nostalre_write_probe(ok INTEGER);"
        "INSERT INTO __nostalre_write_probe VALUES(1);"
        "ROLLBACK;",
        NULL, NULL, NULL);
    if (rc != SQLITE_OK && capacity) {
        snprintf(error, capacity, "SQLite %d: %s", rc,
                 rc == SQLITE_CORRUPT ? "database integrity check failed" :
                 second && sqlite3_errcode(second) != SQLITE_OK ? sqlite3_errmsg(second) :
                 db ? sqlite3_errmsg(db) : "allocation failed");
    }
    if (statement) sqlite3_finalize(statement);
    if (second) sqlite3_close(second);
    if (db) sqlite3_close(db);
    if (rc == SQLITE_OK) rc = check_commit(path, error, capacity);
    return rc;
}

/* quick_check proves SQLite integrity, not that this is a Ragnarok save.
   Validate the structures used before the first character enters a map. */
int nostalre_sqlite_check_schema(const char *path, char *error, size_t capacity) {
    static const struct { const char *name, *query; } checks[] = {
        {"login", "SELECT account_id,userid,user_pass,sex,email,group_id,state,unban_time,expiration_time,logincount,lastlogin,last_ip,birthdate,character_slots,pincode,pincode_change,web_auth_token,web_auth_token_enabled FROM login LIMIT 0"},
        {"char", "SELECT char_id,account_id,char_num,name,class,base_level,job_level,zeny,last_map,last_x,last_y,sex,delete_date,rename,online FROM char LIMIT 0"},
        {"charlog", "SELECT time,char_msg,account_id,char_num,name,str,agi,vit,`int`,dex,luk,hair,hair_color FROM charlog LIMIT 0"},
        {"global_acc_reg_num", "SELECT account_id,`key`,`index`,value FROM global_acc_reg_num LIMIT 0"},
        {"global_acc_reg_str", "SELECT account_id,`key`,`index`,value FROM global_acc_reg_str LIMIT 0"},
        {"char_reg_num", "SELECT * FROM char_reg_num LIMIT 0"},
        {"char_reg_str", "SELECT * FROM char_reg_str LIMIT 0"},
        {"loginlog", "SELECT * FROM loginlog LIMIT 0"},
        {"ipbanlist", "SELECT * FROM ipbanlist LIMIT 0"},
        {"inventory", "SELECT * FROM inventory LIMIT 0"},
        {"cart_inventory", "SELECT * FROM cart_inventory LIMIT 0"},
        {"storage", "SELECT * FROM storage LIMIT 0"},
        {"skill", "SELECT * FROM skill LIMIT 0"},
        {"memo", "SELECT * FROM memo LIMIT 0"},
        {"hotkey", "SELECT * FROM hotkey LIMIT 0"},
        {"interlog", "SELECT * FROM interlog LIMIT 0"},
    };
    sqlite3 *db = NULL;
    int rc = sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, "nostalre");
    const char *name = "abertura";
    for (size_t i = 0; rc == SQLITE_OK && i < sizeof(checks)/sizeof(checks[0]); ++i) {
        sqlite3_stmt *statement = NULL;
        name = checks[i].name;
        rc = sqlite3_prepare_v2(db, checks[i].query, -1, &statement, NULL);
        if (statement) sqlite3_finalize(statement);
    }
    if (rc != SQLITE_OK && capacity) snprintf(error, capacity,
        "Estrutura do banco incompativel (%s): SQLite %d (%s). O banco foi preservado.",
        name, db ? sqlite3_extended_errcode(db) : rc, db ? sqlite3_errmsg(db) : "allocation failed");
    if (db) sqlite3_close(db);
    return rc;
}
