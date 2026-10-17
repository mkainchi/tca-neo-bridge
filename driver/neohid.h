/*
 * TCA-NEO bridge: UMDF2 HID minidriver presenting a T.Flight Hotas NEO-compatible HID personality.
 * Derived from Microsoft Windows-driver-samples hid/vhidmini2 (MS-PL), commit 2dc3fd3a0cc84a2933f2194e7ec0871584979071.
 * Input comes only from neo-daemon over \\.\pipe\neo-hid-v1 (see ../daemon/src/protocol.rs).
 */
#pragma once
#include <windows.h>
#include <wdf.h>
#include <hidport.h>
#include "frame.h"

#define NEOHID_VID      0x044F
#define NEOHID_PID      0x2800
#define NEOHID_VERSION  0x2800   /* revision 2800: avoids the official driver's 2801/2802 matches */
#define NEOHID_PERIOD_MS 10      /* full-speed interrupt IN bInterval used by the Pi gadget */
#define NEOHID_RECONNECT_TICKS 10

#define NEOHID_MANUFACTURER L"TCA-NEO bridge"
#define NEOHID_PRODUCT      L"Virtual NEO"
#define NEOHID_SERIAL       L"0000000001"

typedef struct _DEVICE_CONTEXT {
    WDFDEVICE              Device;
    WDFQUEUE               ManualQueue;
    WDFTIMER               Timer;
    WDFSPINLOCK            Lock;
    HID_DEVICE_ATTRIBUTES  Attributes;
    HID_DESCRIPTOR         HidDescriptor;
    HANDLE                 Pipe;
    ULONG                  ReconnectTicks;
    NEO_STREAM             Stream;
    UCHAR                  Latest[NEO_REPORT_LEN];
} DEVICE_CONTEXT, *PDEVICE_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(DEVICE_CONTEXT, GetDeviceContext);

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD EvtDeviceAdd;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL EvtIoDeviceControl;
EVT_WDF_TIMER EvtTimer;
EVT_WDF_OBJECT_CONTEXT_CLEANUP EvtDeviceCleanup;
