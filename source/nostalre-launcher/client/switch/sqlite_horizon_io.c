/* SQLite storage using libnx FS operations with explicit offsets. */
#include "sqlite3.h"
#include <switch.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    sqlite3_file base;
    FsFile handle;
    char *path;
    int remove_on_close;
} NativeFile;
static sqlite3_vfs native_vfs;
static FsFileSystem *sd;
static int nx_error(const char *operation, Result result, int code) {
    fprintf(stderr, "[SQLite/native] %s failed: SQLite=%d fs=%08x\n", operation, code, result);
    return code;
}
static int missing(Result result) { return result == (2u | (1u << 9)); }
/* libnx sends FS_MAX_PATH bytes, not strlen(path)+1, for every path IPC. */
static Result delete_path(const char *path) {
    char ipc_path[FS_MAX_PATH] = {0};
    size_t length = strlen(path);
    if (length >= sizeof(ipc_path)) return 0x177a02;
    memcpy(ipc_path, path, length + 1);
    return fsFsDeleteFile(sd, ipc_path);
}
static int nx_close(sqlite3_file *file) {
    NativeFile *f = (NativeFile *)file;
    fsFileClose(&f->handle);
    Result result = 0;
    if (f->remove_on_close) result = delete_path(f->path);
    free(f->path); f->base.pMethods = NULL;
    return R_FAILED(result) && !missing(result) ? nx_error("close/delete", result, SQLITE_IOERR_DELETE) : SQLITE_OK;
}
static Result read_bytes(NativeFile *file, void *data, int size, sqlite3_int64 offset, u64 *total) {
    u64 read = 0;
    Result result = fsFileRead(&file->handle, offset, data, size, FsReadOption_None, &read);
    /* FS cannot map every allocator's memory as an IPC user buffer. libnx's
       stdio uses the same stack-copy fallback for Kernel result 0xD401. */
    if ((result & 0x3fffff) == 0xd401) {
        _Alignas(16) unsigned char buffer[4096];
        read = 0;
        while (read < (u64)size) {
            u64 part = (u64)size - read;
            if (part > sizeof(buffer)) part = sizeof(buffer);
            u64 actual = 0;
            result = fsFileRead(&file->handle, offset + read, buffer, part, FsReadOption_None, &actual);
            if (R_FAILED(result)) break;
            if (actual > part) return 0x402;
            memcpy((char *)data + read, buffer, actual);
            read += actual;
            if (actual < part) break;
        }
    }
    *total = read;
    return result;
}
static int nx_read(sqlite3_file *file, void *data, int size, sqlite3_int64 offset) {
    if (size < 0 || offset < 0) return SQLITE_IOERR_READ;
    if (size == 0) return SQLITE_OK;
    NativeFile *f = (NativeFile *)file;
    u64 read = 0;
    Result result = read_bytes(f, data, size, offset, &read);
    /* SQLite deliberately probes beyond EOF, including a new empty database
       and journal headers. Horizon reports OutOfRange there, whereas SQLite
       requires zero-filled output and SQLITE_IOERR_SHORT_READ. Only translate
       this result after confirming the actual file length. */
    if ((result & 0x3fffff) == 0x177a02) {
        s64 length = 0;
        Result stat = fsFileGetSize(&f->handle, &length);
        if (R_FAILED(stat)) return nx_error("read/size", stat, SQLITE_IOERR_FSTAT);
        if (length >= 0 && offset >= length) {
            read = 0; result = 0;
        } else if (length > offset && length - offset < size) {
            result = read_bytes(f, data, (int)(length - offset), offset, &read);
        }
    }
    if (R_FAILED(result)) {
        fprintf(stderr, "[SQLite/native] read range: path=%s offset=%lld requested=%d\n",
            f->path, (long long)offset, size);
        return nx_error("read", result, SQLITE_IOERR_READ);
    }
    if (read > (u64)size) return SQLITE_IOERR_READ;
    if (read < (u64)size) { memset((char *)data + read, 0, size - read); return SQLITE_IOERR_SHORT_READ; }
    return SQLITE_OK;
}
static int nx_write(sqlite3_file *file, const void *data, int size, sqlite3_int64 offset) {
    Result result = fsFileWrite(&((NativeFile *)file)->handle, offset, data, size, FsWriteOption_None);
    if ((result & 0x3fffff) == 0xd401) {
        _Alignas(16) unsigned char buffer[4096];
        int written = 0;
        while (written < size) {
            int part = size - written;
            if (part > (int)sizeof(buffer)) part = sizeof(buffer);
            memcpy(buffer, (const char *)data + written, part);
            result = fsFileWrite(&((NativeFile *)file)->handle, offset + written, buffer, part, FsWriteOption_None);
            if (R_FAILED(result)) break;
            written += part;
        }
    }
    return R_FAILED(result) ? nx_error("write", result, SQLITE_IOERR_WRITE) : SQLITE_OK;
}
static int nx_truncate(sqlite3_file *file, sqlite3_int64 size) {
    Result result = fsFileSetSize(&((NativeFile *)file)->handle, size);
    return R_FAILED(result) ? nx_error("truncate", result, SQLITE_IOERR_TRUNCATE) : SQLITE_OK;
}
static int nx_sync(sqlite3_file *file, int flags) {
    (void)flags;
    Result result = fsFileFlush(&((NativeFile *)file)->handle);
    return R_FAILED(result) ? nx_error("flush", result, SQLITE_IOERR_FSYNC) : SQLITE_OK;
}
static int nx_size(sqlite3_file *file, sqlite3_int64 *size) {
    s64 value;
    Result result = fsFileGetSize(&((NativeFile *)file)->handle, &value);
    if (R_FAILED(result)) return nx_error("size", result, SQLITE_IOERR_FSTAT);
    *size = value; return SQLITE_OK;
}
/* The outer nostalre VFS owns inter-connection locking. */
static int nx_lock(sqlite3_file *file, int level) { (void)file; (void)level; return SQLITE_OK; }
static int nx_reserved(sqlite3_file *file, int *reserved) { (void)file; *reserved = 0; return SQLITE_OK; }
static int nx_control(sqlite3_file *file, int op, void *arg) { (void)file; (void)op; (void)arg; return SQLITE_NOTFOUND; }
static int nx_sector(sqlite3_file *file) { (void)file; return 4096; }
static int nx_characteristics(sqlite3_file *file) { (void)file; return 0; }
static const sqlite3_io_methods native_methods = {
    .iVersion = 1, .xClose = nx_close, .xRead = nx_read, .xWrite = nx_write,
    .xTruncate = nx_truncate, .xSync = nx_sync, .xFileSize = nx_size,
    .xLock = nx_lock, .xUnlock = nx_lock, .xCheckReservedLock = nx_reserved,
    .xFileControl = nx_control, .xSectorSize = nx_sector, .xDeviceCharacteristics = nx_characteristics,
};
static int sd_path(const char *name, char path[FS_MAX_PATH]) {
    if (!name || strncmp(name, "sdmc:/", 6) != 0) return 0;
    size_t length = strlen(name + 5);
    if (length >= FS_MAX_PATH) return 0;
    memset(path, 0, FS_MAX_PATH);
    memcpy(path, name + 5, length + 1);
    return 1;
}
static int nx_open(sqlite3_vfs *v, const char *name, sqlite3_file *file, int flags, int *out) {
    (void)v;
    NativeFile *f = (NativeFile *)file;
    memset(f, 0, sizeof(*f));
    char temporary[FS_MAX_PATH];
    if (!name) {
        const char *root = getenv("RA_SQLITE_DIR");
        if (!root) root = "sdmc:/switch/ragswitch-pre";
        unsigned long long nonce; sqlite3_randomness(sizeof(nonce), &nonce);
        if (snprintf(temporary, sizeof(temporary), "%s/.sqlite-temp-%016llx", root, nonce) >= (int)sizeof(temporary)) return SQLITE_CANTOPEN;
        name = temporary;
        flags |= SQLITE_OPEN_CREATE | SQLITE_OPEN_EXCLUSIVE | SQLITE_OPEN_DELETEONCLOSE;
    }
    char path[FS_MAX_PATH];
    if (!sd_path(name, path)) return SQLITE_CANTOPEN;
    FsDirEntryType type;
    Result result = fsFsGetEntryType(sd, path, &type);
    if (R_SUCCEEDED(result)) {
        if (type != FsDirEntryType_File || (flags & SQLITE_OPEN_EXCLUSIVE)) return SQLITE_CANTOPEN;
    } else if (missing(result) && (flags & SQLITE_OPEN_CREATE)) {
        result = fsFsCreateFile(sd, path, 0, 0);
        if (R_FAILED(result)) return nx_error("create", result, SQLITE_CANTOPEN);
    } else return nx_error("open/entry", result, SQLITE_CANTOPEN);
    f->path = strdup(path);
    if (!f->path) return SQLITE_NOMEM;
    u32 mode = FsOpenMode_Read;
    if (flags & SQLITE_OPEN_READWRITE) mode |= FsOpenMode_Write | FsOpenMode_Append;
    result = fsFsOpenFile(sd, path, mode, &f->handle);
    if (R_FAILED(result)) {
        free(f->path); f->path = NULL;
        return nx_error("open", result, SQLITE_CANTOPEN);
    }
    f->remove_on_close = !!(flags & SQLITE_OPEN_DELETEONCLOSE);
    f->base.pMethods = &native_methods;
    if (out) *out = flags;
    return SQLITE_OK;
}
static int nx_delete(sqlite3_vfs *v, const char *name, int sync) {
    (void)v; (void)sync;
    char path[FS_MAX_PATH];
    if (!sd_path(name, path)) return SQLITE_IOERR_DELETE;
    Result result = fsFsDeleteFile(sd, path);
    if (missing(result)) return SQLITE_IOERR_DELETE_NOENT;
    return R_FAILED(result) ? nx_error("delete", result, SQLITE_IOERR_DELETE) : SQLITE_OK;
}
static int nx_access(sqlite3_vfs *v, const char *name, int flags, int *exists) {
    (void)v; (void)flags;
    char path[FS_MAX_PATH];
    if (!sd_path(name, path)) return SQLITE_IOERR_ACCESS;
    FsDirEntryType type;
    Result result = fsFsGetEntryType(sd, path, &type);
    *exists = R_SUCCEEDED(result);
    if (missing(result)) return SQLITE_OK;
    return R_FAILED(result) ? nx_error("access", result, SQLITE_IOERR_ACCESS) : SQLITE_OK;
}
sqlite3_vfs *nostalre_horizon_io(sqlite3_vfs *fallback) {
    sd = fsdevGetDeviceFileSystem("sdmc");
    if (!sd) return NULL;
    native_vfs = *fallback;
    native_vfs.zName = "horizon-sd";
    native_vfs.szOsFile = sizeof(NativeFile);
    native_vfs.xOpen = nx_open; native_vfs.xDelete = nx_delete; native_vfs.xAccess = nx_access;
    return &native_vfs;
}
