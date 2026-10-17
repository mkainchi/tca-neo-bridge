/* Unit test for frame.h; built and run by build.ps1. Exit code 0 means every check passed. */
#include <stdio.h>
#include <string.h>
#include "frame.h"

static int failures;
#define CHECK(x) do { if (!(x)) { printf("FAIL line %d: %s\n", __LINE__, #x); failures++; } } while (0)

static void make_frame(unsigned char frame[NEO_FRAME_LEN], unsigned char marker)
{
    memset(frame, 0, NEO_FRAME_LEN);
    frame[0] = 'N'; frame[1] = 'E'; frame[2] = 1; frame[3] = 1; frame[4] = 1;
    frame[10] = marker;
}

int main(void)
{
    unsigned char latest[NEO_REPORT_LEN], neutral[NEO_REPORT_LEN], frame[NEO_FRAME_LEN], two[2 * NEO_FRAME_LEN + 3];
    NEO_STREAM stream = {0};
    unsigned int i;

    /* Neutral must equal neo::neutral() in the daemon: 01 08 00 00 then seven 0x8000 axes. */
    neo_neutral(neutral);
    CHECK(neutral[0] == 1 && neutral[1] == 8 && neutral[2] == 0 && neutral[3] == 0);
    for (i = 0; i < 7; i++) CHECK(neutral[4 + 2 * i] == 0 && neutral[5 + 2 * i] == 0x80);
    for (i = 18; i < NEO_REPORT_LEN; i++) CHECK(neutral[i] == 0);

    memcpy(latest, neutral, sizeof latest);
    make_frame(frame, 0x42);
    CHECK(neo_stream_push(&stream, frame, NEO_FRAME_LEN, latest) == 1);
    CHECK(latest[0] == 1 && latest[6] == 0x42 && stream.Used == 0);

    /* Split delivery. */
    make_frame(frame, 0x43);
    CHECK(neo_stream_push(&stream, frame, 30, latest) == 0);
    CHECK(latest[6] == 0x42);
    CHECK(neo_stream_push(&stream, frame + 30, NEO_FRAME_LEN - 30, latest) == 1);
    CHECK(latest[6] == 0x43);

    /* Garbage prefix, then two frames: resynchronises and keeps the last. */
    two[0] = 'x'; two[1] = 'N'; two[2] = 'Z';
    make_frame(two + 3, 0x50);
    make_frame(two + 3 + NEO_FRAME_LEN, 0x51);
    CHECK(neo_stream_push(&stream, two, sizeof two, latest) == 2);
    CHECK(latest[6] == 0x51);

    /* Wrong version, kind and report id are all rejected. */
    for (i = 2; i <= 4; i++) {
        NEO_STREAM fresh = {0};
        make_frame(frame, 0x60);
        frame[i] = 9;
        CHECK(neo_stream_push(&fresh, frame, NEO_FRAME_LEN, latest) == 0);
        CHECK(latest[6] == 0x51);
    }
    frame[0] = 'M';
    CHECK(neo_frame_valid(frame) == 0);
    make_frame(frame, 0);
    frame[1] = 'F';
    CHECK(neo_frame_valid(frame) == 0);

    printf(failures ? "frame_test: %d failure(s)\n" : "frame_test: all checks passed\n", failures);
    return failures != 0;
}
