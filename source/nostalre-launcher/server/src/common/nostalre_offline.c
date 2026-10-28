#include "nostalre_offline.h"
#include "sqlite3.h"
#include <stdio.h>
#include <string.h>
static NostalreOfflineProfile profile;
static int ready;
const NostalreOfflineProfile *nostalre_offline_get_profile(void) { return ready ? &profile : NULL; }
int nostalre_offline_validate(uint32_t id, uint32_t first, uint32_t second, uint8_t sex, uint32_t ip) {
    return ready && ip == 0x7f000001u && id == profile.account_id &&
        first == profile.session_id1 && second == profile.session_id2 && sex == profile.sex;
}
static void copy_column(sqlite3_stmt *statement, int column, char *out, size_t capacity) {
    const unsigned char *text = sqlite3_column_text(statement, column);
    if (text) snprintf(out, capacity, "%s", text);
}
int nostalre_offline_initialize(const char *path, NostalreOfflineProfile *out, char *error, size_t capacity) {
    if (!path || !out || ready) return SQLITE_MISUSE;
    sqlite3 *db = NULL;
    sqlite3_stmt *statement = NULL;
    NostalreOfflineProfile found = {0};
    const char *phase = "abertura do perfil";
    int rc = sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX,
#if defined(__SWITCH__) || defined(NOSTALRE_TEST_NATIVE_VFS)
        "nostalre"
#else
        NULL
#endif
    );
    if (rc == SQLITE_OK) rc = sqlite3_busy_timeout(db, 10000);
    if (rc == SQLITE_OK) {
        phase = "save nao pertence a edicao Pre-Renewal";
        sqlite3_stmt *edition = NULL;
        rc = sqlite3_prepare_v2(db, "SELECT edition FROM edition_identity WHERE id=1", -1, &edition, NULL);
        if (rc == SQLITE_OK) {
            const unsigned char *identity = sqlite3_step(edition) == SQLITE_ROW ? sqlite3_column_text(edition, 0) : NULL;
            if (!identity || strcmp((const char *)identity, "ragswitch-prerenewal") != 0) rc = SQLITE_MISMATCH;
        }
        if (edition) sqlite3_finalize(edition);
    }
    if (rc == SQLITE_OK) {
        phase = "leitura do perfil local";
        rc = sqlite3_prepare_v2(db,
            "SELECT account_id,sex,group_id,character_slots,email,birthdate,userid "
            "FROM login WHERE account_id>=2000000 AND sex IN ('M','F') ORDER BY account_id LIMIT 1",
            -1, &statement, NULL);
    }
    if (rc == SQLITE_OK) {
        int step = sqlite3_step(statement);
        if (step != SQLITE_ROW) {
            phase = step == SQLITE_DONE ? "perfil local ausente" : "leitura do perfil local";
            rc = step == SQLITE_DONE ? SQLITE_NOTFOUND : step;
        } else {
            sqlite3_int64 id = sqlite3_column_int64(statement, 0);
            const unsigned char *sex = sqlite3_column_text(statement, 1);
            if (id < 2000000 || id > UINT32_MAX || !sex || (sex[0] != 'M' && sex[0] != 'F')) rc = SQLITE_CORRUPT;
            else {
                found.account_id = (uint32_t)id;
                found.sex = sex[0] == 'M';
                found.group_id = (uint32_t)sqlite3_column_int64(statement, 2);
                int slots = sqlite3_column_int(statement, 3);
                found.character_slots = slots < 0 ? 0 : slots > 255 ? 255 : (uint8_t)slots;
                copy_column(statement, 4, found.email, sizeof(found.email));
                copy_column(statement, 5, found.birthdate, sizeof(found.birthdate));
                copy_column(statement, 6, found.name, sizeof(found.name));
            }
        }
    }
    if (rc != SQLITE_OK && capacity) snprintf(error, capacity, "%s: SQLite %d (%s). Save preservado.",
        phase, rc, db ? sqlite3_errmsg(db) : "allocation failed");
    if (statement) sqlite3_finalize(statement);
    if (db) sqlite3_close(db);
    if (rc == SQLITE_OK) {
        sqlite3_randomness(sizeof(found.session_id1), &found.session_id1);
        sqlite3_randomness(sizeof(found.session_id2), &found.session_id2);
        if (!found.session_id1) found.session_id1 = 1;
        if (!found.session_id2) found.session_id2 = 1;
        profile = found; *out = found; ready = 1;
    }
    return rc;
}
