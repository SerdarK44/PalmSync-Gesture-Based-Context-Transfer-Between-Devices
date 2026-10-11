#include "log.hpp"

#include <chrono>
#include <cstdarg>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <mutex>
#include <string>

#ifdef _WIN32
#include <windows.h>
#include <share.h>
#endif

namespace {

FILE* g_log = nullptr;
std::mutex g_mutex;

constexpr unsigned long long kMaxLogBytes = 2ull * 1024 * 1024;  // rotate to .1 above this

#ifdef _WIN32
// The old Eye wrote a new palmsync-eye-YYYYMMDD-HHMMSS.log on every start and
// never cleaned up. Remove those leftovers; everything now goes to one file.
void delete_legacy_logs(const std::wstring& dir) {
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"\\palmsync-eye-*.log").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE) return;
    do {
        DeleteFileW((dir + L"\\" + fd.cFileName).c_str());
    } while (FindNextFileW(h, &fd));
    FindClose(h);
}

// CreateDirectory for every level of the path ("C:\a\b\c").
void create_dirs(const std::wstring& dir) {
    for (size_t i = 3; i <= dir.size(); ++i) {
        if (i == dir.size() || dir[i] == L'\\' || dir[i] == L'/')
            CreateDirectoryW(dir.substr(0, i).c_str(), NULL);
    }
}
#endif

void timestamp(char* buf, size_t n) {
    using namespace std::chrono;
    auto now = system_clock::now();
    auto ms = duration_cast<milliseconds>(now.time_since_epoch()).count() % 1000;
    std::time_t t = system_clock::to_time_t(now);
    std::tm tm{};
#ifdef _WIN32
    localtime_s(&tm, &t);
#else
    localtime_r(&t, &tm);
#endif
    std::snprintf(buf, n, "%04d-%02d-%02d %02d:%02d:%02d.%03d",
                  tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday,
                  tm.tm_hour, tm.tm_min, tm.tm_sec, static_cast<int>(ms));
}

const char* level_of(std::string& msg) {
    if (msg.rfind("[debug] ", 0) == 0) {
        msg.erase(0, 8);
        return "DEBUG";
    }
    if (msg.find("FAIL") != std::string::npos || msg.find("failed") != std::string::npos ||
        msg.find("could not") != std::string::npos)
        return "WARN";
    return "INFO";
}

}  // namespace

void eye_log_init(const std::wstring& requested_dir) {
#ifdef _WIN32
    std::wstring dir = requested_dir;
    if (dir.empty()) {
        wchar_t buf[MAX_PATH * 2];
        DWORD n = GetEnvironmentVariableW(L"LOCALAPPDATA", buf, MAX_PATH * 2);
        if (n == 0 || n >= MAX_PATH * 2) return;
        dir = std::wstring(buf) + L"\\PalmSync\\logs";
    }
    while (!dir.empty() && (dir.back() == L'\\' || dir.back() == L'/')) dir.pop_back();
    create_dirs(dir);
    delete_legacy_logs(dir);

    std::wstring path = dir + L"\\palmsync-eye.log";
    WIN32_FILE_ATTRIBUTE_DATA attr;
    if (GetFileAttributesExW(path.c_str(), GetFileExInfoStandard, &attr)) {
        unsigned long long size =
            (static_cast<unsigned long long>(attr.nFileSizeHigh) << 32) | attr.nFileSizeLow;
        if (size > kMaxLogBytes)
            MoveFileExW(path.c_str(), (path + L".1").c_str(), MOVEFILE_REPLACE_EXISTING);
    }

    // _SH_DENYNO: the UI's log viewer reads the file while we write it.
    g_log = _wfsopen(path.c_str(), L"a", _SH_DENYNO);
#else
    (void)requested_dir;
#endif
}

void eye_log_close() {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_log) {
        std::fclose(g_log);
        g_log = nullptr;
    }
}

void eye_log(const char* fmt, ...) {
    char buf[2048];
    va_list ap;
    va_start(ap, fmt);
    std::vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);

    std::fputs(buf, stderr);

    std::string msg(buf);
    if (msg.rfind("[Eye] ", 0) == 0) msg.erase(0, 6);
    while (!msg.empty() && (msg.back() == '\n' || msg.back() == '\r')) msg.pop_back();
    if (msg.empty()) return;
    const char* level = level_of(msg);

    std::lock_guard<std::mutex> lock(g_mutex);
    if (!g_log) return;
    char ts[32];
    timestamp(ts, sizeof(ts));
    std::fprintf(g_log, "%s [%s] eye: %s\n", ts, level, msg.c_str());
    std::fflush(g_log);
}
