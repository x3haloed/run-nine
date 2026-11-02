/* Project-authored native oracle: OS9 expands CMPD; EXPT holds owned bytes.
 * This probe contains no decompressor or application/OS implementation code.
 */
#include <Multiverse.h>
#include <stdio.h>
#include <string.h>

static short output;
static short write_line(const char *text) {
    long length = strlen(text);
    return FSWrite(output, &length, text);
}

int main(void) {
    short count, index, batch, batches = 1, app, external = 0, ref;
    char line[1024];
    unsigned char path[128];
    Handle config;
    OSErr error;
    app = CurResFile();
    config = Get1Resource('BATS', 128);
    if (config && GetHandleSize(config) == 2) {
        batches = ((unsigned char)(*config)[0] << 8) | (unsigned char)(*config)[1];
        external = 1;
    }
    if (config) ReleaseResource(config);
    error = Create("\pResourceCodecProbe.jsonl", 0, 'R9Pr', 'TEXT');
    if (error || FSOpen("\pResourceCodecProbe.jsonl", 0, &output)) return 2;
    sprintf(line, "{\"schema\":1,\"event\":\"start\",\"batches\":%d}\n", batches);
    if (write_line(line)) return 3;
    for (batch = 0; batch < batches; batch++) {
    ref = app;
    if (external) {
        sprintf((char*)path + 1, ":::Reference Files:CodecCases%03d", batch);
        path[0] = strlen((char*)path + 1);
        ref = OpenResFile(path);
        error = ResError();
        sprintf(line, "{\"event\":\"open\",\"batch\":%d,\"error\":%d,\"valid_ref\":%s}\n", batch, error, ref != -1 ? "true" : "false");
        if (write_line(line)) return 7;
        if (ref == -1 || error) { FSClose(output); FlushVol(0, 0); return 8; }
    }
    count = Count1Resources('CMPD');
    sprintf(line, "{\"event\":\"batch\",\"batch\":%d,\"cases\":%d}\n", batch, count);
    if (write_line(line)) return 9;
    for (index = 0; index < count; index++) {
        Handle actual, expected;
        char actual_hex[65] = "", expected_hex[65] = "";
        long actual_size = -1, expected_size = -1, first = -1, differences = 0, i, common;
        short native_error, expected_error, attrs = -1;
        sprintf(line, "{\"event\":\"begin\",\"batch\":%d,\"case\":%d}\n", batch, index);
        if (write_line(line)) return 4;
        FlushVol(0, 0);
        actual = Get1Resource('CMPD', 128 + index);
        native_error = ResError();
        if (actual) {
            actual_size = GetHandleSize(actual);
            if (actual_size >= 0) {
                HLock(actual);
                attrs = GetResAttrs(actual);
            }
        }
        expected = Get1Resource('EXPT', 128 + index);
        expected_error = ResError();
        if (expected) {
            expected_size = GetHandleSize(expected);
            if (expected_size >= 0) HLock(expected);
        }
        if (actual && expected && actual_size >= 0 && expected_size >= 0) {
            common = actual_size < expected_size ? actual_size : expected_size;
            for (i = 0; i < common; i++) {
                if ((unsigned char)(*actual)[i] != (unsigned char)(*expected)[i]) {
                    if (first == -1) first = i;
                    differences++;
                }
            }
            if (actual_size != expected_size) {
                if (first == -1) first = common;
                differences += actual_size > expected_size ? actual_size - expected_size : expected_size - actual_size;
            }
        }
        /* Bounded diagnostics for isolated owned hypotheses. Original-app runs
         * and all observation files remain private, including these bytes. */
        if (first >= 0 && actual && expected && !native_error && !expected_error && actual_size >= 0 && expected_size >= 0) {
            for (i = 0; i < 32 && first + i < actual_size; i++)
                sprintf(actual_hex + 2 * i, "%02x", (unsigned char)(*actual)[first + i]);
            for (i = 0; i < 32 && first + i < expected_size; i++)
                sprintf(expected_hex + 2 * i, "%02x", (unsigned char)(*expected)[first + i]);
        }
        sprintf(line, "{\"event\":\"result\",\"batch\":%d,\"case\":%d,\"native_error\":%d,\"expected_error\":%d,\"actual_size\":%ld,\"expected_size\":%ld,\"attrs_after_load\":%d,\"equal\":%s,\"different_bytes\":%ld,\"first_difference\":%ld,\"actual_at_difference\":\"%s\",\"expected_at_difference\":\"%s\"}\n",
                batch, index, native_error, expected_error, actual_size, expected_size, attrs,
                actual && expected && actual_size >= 0 && expected_size >= 0 && !native_error && !expected_error && first == -1 ? "true" : "false",
                differences, first, actual_hex, expected_hex);
        if (write_line(line)) return 5;
        if ((actual && actual_size < 0) || (expected && expected_size < 0)) {
            /* Do not release a malformed native handle or continue a tainted
             * heap experiment; preserve its result and stop this corpus. */
            FSClose(output); FlushVol(0, 0); return 10;
        }
        if (actual) { HUnlock(actual); ReleaseResource(actual); }
        if (expected) { HUnlock(expected); ReleaseResource(expected); }
        FlushVol(0, 0);
    }
    if (external) { CloseResFile(ref); UseResFile(app); }
    }
    if (write_line("{\"event\":\"complete\"}\n")) return 6;
    FSClose(output);
    FlushVol(0, 0);
    return 0;
}
