#undef main
#include "common/nostalre_offline.h"
#include <cstdio>
#include <cstdlib>
#include <string>
extern int nostalre_test_server_main(int, char **);
int main(int argc, char **argv) {
    const char *directory = getenv("RA_SQLITE_DIR");
    const char *session_path = getenv("NOSTALRE_TEST_SESSION_PATH");
    if (!directory || !session_path) return 90;
    NostalreOfflineProfile profile{}; char error[256]{};
    const std::string database = std::string(directory) + "/ragnarok.db";
    if (nostalre_offline_initialize(database.c_str(), &profile, error, sizeof(error))) {
        fprintf(stderr, "%s\n", error); return 91;
    }
    FILE *output = fopen(session_path, "wb");
    if (!output) return 92;
    bool ok = fwrite(&profile, sizeof(profile), 1, output) == 1;
    if (fclose(output) || !ok) return 93;
    return nostalre_test_server_main(argc, argv);
}
