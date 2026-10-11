#pragma once
#include <cstdio>
#include <string>

// Eye logging. Every line goes to stderr (read by the tray UI) and to
// <log dir>\palmsync-eye.log in the same format as the daemon, so the log
// viewer can merge both:
//   2026-10-11 14:08:47.123 [INFO] eye: Camera 0 opened via DirectShow
// A leading "[Eye] " in the message is stripped. A leading "[debug] " makes
// the line DEBUG. Lines containing "FAIL", "failed" or "could not" are WARN.

// Opens the log file (rotating it if it's grown large). `dir` may contain
// any Unicode characters (Turkish user names, OneDrive "Belgeler"...);
// empty = %LOCALAPPDATA%\PalmSync\logs.
void eye_log_init(const std::wstring& dir = L"");
void eye_log_close();
void eye_log(const char* fmt, ...);

#define EYE_LOG(...) eye_log(__VA_ARGS__)
