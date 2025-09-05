#include <stdarg.h>
#include <stdio.h>

extern "C" int tsuki_snprintf(char *buffer, size_t count, const char *format, ...)
{
    va_list args;
    int ret;

    va_start(args, format);
    ret = vsnprintf(buffer, count, format, args);
    va_end(args);

    return ret;
}
