/* Pipe frame parsing shared by the driver and its unit test. Layout: "NE", version 1, kind 1, report 01 (64 bytes). */
#pragma once

#define NEO_REPORT_LEN 64
#define NEO_FRAME_LEN  68

typedef struct _NEO_STREAM {
    unsigned char Buffer[NEO_FRAME_LEN];
    unsigned int  Used;
} NEO_STREAM;

/* Centred axes, no buttons, POV neutral. */
static void neo_neutral(unsigned char report[NEO_REPORT_LEN])
{
    unsigned int i;
    for (i = 0; i < NEO_REPORT_LEN; i++) report[i] = 0;
    report[0] = 1;
    report[1] = 8;
    for (i = 0; i < 7; i++) report[5 + 2 * i] = 0x80;
}

static int neo_frame_valid(const unsigned char *frame)
{
    return frame[0] == 'N' && frame[1] == 'E' && frame[2] == 1 && frame[3] == 1 && frame[4] == 1;
}

/* Feed received bytes; every complete valid frame replaces `latest`. Invalid data resynchronises byte by byte. */
static unsigned int neo_stream_push(NEO_STREAM *stream, const unsigned char *data, unsigned int length, unsigned char latest[NEO_REPORT_LEN])
{
    unsigned int accepted = 0, i, j;
    for (i = 0; i < length; i++) {
        stream->Buffer[stream->Used++] = data[i];
        if (stream->Used < NEO_FRAME_LEN) continue;
        if (neo_frame_valid(stream->Buffer)) {
            for (j = 0; j < NEO_REPORT_LEN; j++) latest[j] = stream->Buffer[4 + j];
            stream->Used = 0;
            accepted++;
        } else {
            for (j = 1; j < NEO_FRAME_LEN; j++) stream->Buffer[j - 1] = stream->Buffer[j];
            stream->Used = NEO_FRAME_LEN - 1;
        }
    }
    return accepted;
}
