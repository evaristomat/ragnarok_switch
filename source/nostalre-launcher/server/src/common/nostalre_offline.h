#ifndef NOSTALRE_OFFLINE_H
#define NOSTALRE_OFFLINE_H
#include <stddef.h>
#include <stdint.h>
#if defined(__SWITCH__) || defined(NOSTALRE_OFFLINE_TEST)
#define NOSTALRE_OFFLINE_BUILD 1
#else
#define NOSTALRE_OFFLINE_BUILD 0
#endif
#ifdef __cplusplus
extern "C" {
#endif
typedef struct NostalreOfflineProfile {
    uint32_t account_id;
    uint32_t session_id1;
    uint32_t session_id2;
    uint32_t group_id;
    uint8_t sex;
    uint8_t character_slots;
    uint8_t reserved[2];
    char email[40];
    char birthdate[11];
    char name[24];
} NostalreOfflineProfile;
/* Called once before workers start. Reads the existing save without changing
   account data; produces process-local session ids, without passwords/tokens. */
int nostalre_offline_initialize(const char *path, NostalreOfflineProfile *out, char *error, size_t capacity);
const NostalreOfflineProfile *nostalre_offline_get_profile(void);
int nostalre_offline_validate(uint32_t account_id, uint32_t session_id1, uint32_t session_id2, uint8_t sex, uint32_t peer_ip);
#ifdef __cplusplus
}
#endif
#endif
