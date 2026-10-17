/*
 * neohid: UMDF2 HID minidriver for the TCA-NEO bridge virtual NEO.
 * Derived from Microsoft Windows-driver-samples hid/vhidmini2 (MS-PL; see LICENSE-vhidmini2.txt),
 * commit 2dc3fd3a0cc84a2933f2194e7ec0871584979071. Changes: NEO descriptor/attributes/strings,
 * removal of the sample's control collection and registry descriptor, and a periodic timer that
 * reads frames from neo-daemon's pipe and completes pending input reads (Pi f_hid behaviour:
 * one report per bInterval, silent feature/output handling, zero-filled GET_REPORT).
 */
#include "neohid.h"
#include "descriptor.h"

static const WCHAR PipeName[] = L"\\\\.\\pipe\\neo-hid-v1";

NTSTATUS DriverEntry(_In_ PDRIVER_OBJECT DriverObject, _In_ PUNICODE_STRING RegistryPath)
{
    WDF_DRIVER_CONFIG config;
    WDF_DRIVER_CONFIG_INIT(&config, EvtDeviceAdd);
    return WdfDriverCreate(DriverObject, RegistryPath, WDF_NO_OBJECT_ATTRIBUTES, &config, WDF_NO_HANDLE);
}

static NTSTATUS CopyToRequest(_In_ WDFREQUEST Request, _In_reads_bytes_(Length) const void *Source, _In_ size_t Length)
{
    WDFMEMORY memory;
    size_t available;
    NTSTATUS status = WdfRequestRetrieveOutputMemory(Request, &memory);
    if (!NT_SUCCESS(status)) return status;
    WdfMemoryGetBuffer(memory, &available);
    if (available < Length) return STATUS_INVALID_BUFFER_SIZE;
    status = WdfMemoryCopyFromBuffer(memory, 0, (PVOID)Source, Length);
    if (NT_SUCCESS(status)) WdfRequestSetInformation(Request, Length);
    return status;
}

static void Snapshot(_In_ PDEVICE_CONTEXT Context, _Out_writes_bytes_(NEO_REPORT_LEN) UCHAR *Report)
{
    WdfSpinLockAcquire(Context->Lock);
    RtlCopyMemory(Report, Context->Latest, NEO_REPORT_LEN);
    WdfSpinLockRelease(Context->Lock);
}

static void ClosePipe(_In_ PDEVICE_CONTEXT Context)
{
    if (Context->Pipe != INVALID_HANDLE_VALUE) {
        CloseHandle(Context->Pipe);
        Context->Pipe = INVALID_HANDLE_VALUE;
    }
    Context->Stream.Used = 0;
    WdfSpinLockAcquire(Context->Lock);
    neo_neutral(Context->Latest);   /* daemon gone: release every control */
    WdfSpinLockRelease(Context->Lock);
}

/* Drain whatever the daemon wrote since the last tick, never blocking the timer. */
static void PumpPipe(_In_ PDEVICE_CONTEXT Context)
{
    UCHAR chunk[NEO_FRAME_LEN * 4], latest[NEO_REPORT_LEN];
    DWORD available = 0, read = 0;
    if (Context->Pipe == INVALID_HANDLE_VALUE) {
        if (++Context->ReconnectTicks < NEOHID_RECONNECT_TICKS) return;
        Context->ReconnectTicks = 0;
        Context->Pipe = CreateFileW(PipeName, GENERIC_READ, 0, NULL, OPEN_EXISTING, 0, NULL);
        if (Context->Pipe == INVALID_HANDLE_VALUE) return;
    }
    Snapshot(Context, latest);
    for (;;) {
        if (!PeekNamedPipe(Context->Pipe, NULL, 0, NULL, &available, NULL)) {
            ClosePipe(Context);
            return;
        }
        if (available == 0) break;
        if (!ReadFile(Context->Pipe, chunk, min(available, (DWORD)sizeof chunk), &read, NULL) || read == 0) {
            ClosePipe(Context);
            return;
        }
        neo_stream_push(&Context->Stream, chunk, read, latest);
    }
    WdfSpinLockAcquire(Context->Lock);
    RtlCopyMemory(Context->Latest, latest, NEO_REPORT_LEN);
    WdfSpinLockRelease(Context->Lock);
}

VOID EvtTimer(_In_ WDFTIMER Timer)
{
    PDEVICE_CONTEXT context = GetDeviceContext((WDFDEVICE)WdfTimerGetParentObject(Timer));
    WDFREQUEST request;
    UCHAR report[NEO_REPORT_LEN];
    PumpPipe(context);
    /* One input report per interval, like an interrupt IN endpoint polled every bInterval. */
    if (NT_SUCCESS(WdfIoQueueRetrieveNextRequest(context->ManualQueue, &request))) {
        Snapshot(context, report);
        WdfRequestComplete(request, CopyToRequest(request, report, sizeof report));
    }
}

VOID EvtDeviceCleanup(_In_ WDFOBJECT Object)
{
    PDEVICE_CONTEXT context = GetDeviceContext((WDFDEVICE)Object);
    WdfTimerStop(context->Timer, TRUE);
    if (context->Pipe != INVALID_HANDLE_VALUE) CloseHandle(context->Pipe);
    context->Pipe = INVALID_HANDLE_VALUE;
}

/* HID_XFER_PACKET as rebuilt by mshidumdf: report id in the input buffer, report buffer in the output buffer. */
static NTSTATUS ReadPacket(_In_ WDFREQUEST Request, _Out_ UCHAR *ReportId, _Out_ WDFMEMORY *Output, _Out_ size_t *Length)
{
    WDFMEMORY input;
    size_t inputLength;
    PVOID buffer;
    NTSTATUS status = WdfRequestRetrieveInputMemory(Request, &input);
    if (!NT_SUCCESS(status)) return status;
    buffer = WdfMemoryGetBuffer(input, &inputLength);
    if (inputLength < sizeof(UCHAR)) return STATUS_INVALID_BUFFER_SIZE;
    *ReportId = *(PUCHAR)buffer;
    status = WdfRequestRetrieveOutputMemory(Request, Output);
    if (!NT_SUCCESS(status)) return status;
    WdfMemoryGetBuffer(*Output, Length);
    return STATUS_SUCCESS;
}

/* GET_FEATURE / GET_INPUT_REPORT: report 01 returns the latest state, anything else is zero-filled. */
static NTSTATUS GetReport(_In_ PDEVICE_CONTEXT Context, _In_ WDFREQUEST Request, _In_ BOOLEAN Input)
{
    UCHAR id, report[NEO_REPORT_LEN] = {0};
    WDFMEMORY output;
    size_t length, count;
    NTSTATUS status = ReadPacket(Request, &id, &output, &length);
    if (!NT_SUCCESS(status)) return status;
    if (Input && id == 1) {
        Snapshot(Context, report);
    } else {
        report[0] = id;
    }
    count = min(length, sizeof report);
    if (count == 0) return STATUS_INVALID_BUFFER_SIZE;
    status = WdfMemoryCopyFromBuffer(output, 0, report, count);
    if (NT_SUCCESS(status)) WdfRequestSetInformation(Request, count);
    return status;
}

/* SET_FEATURE / SET_OUTPUT_REPORT / WRITE_REPORT: accepted and discarded (silent BF baseline). */
static NTSTATUS AcceptWrite(_In_ WDFREQUEST Request)
{
    WDFMEMORY input;
    size_t length = 0;
    NTSTATUS status = WdfRequestRetrieveInputMemory(Request, &input);
    if (!NT_SUCCESS(status)) return status;
    WdfMemoryGetBuffer(input, &length);
    WdfRequestSetInformation(Request, length);
    return STATUS_SUCCESS;
}

static NTSTATUS GetString(_In_ WDFREQUEST Request)
{
    WDFMEMORY input;
    size_t length;
    PVOID buffer;
    ULONG id;
    NTSTATUS status = WdfRequestRetrieveInputMemory(Request, &input);
    if (!NT_SUCCESS(status)) return status;
    buffer = WdfMemoryGetBuffer(input, &length);
    if (length < sizeof(ULONG)) return STATUS_INVALID_BUFFER_SIZE;
    id = *(PULONG)buffer & 0xffff;
    switch (id) {
    case HID_STRING_ID_IMANUFACTURER: return CopyToRequest(Request, NEOHID_MANUFACTURER, sizeof NEOHID_MANUFACTURER);
    case HID_STRING_ID_IPRODUCT: return CopyToRequest(Request, NEOHID_PRODUCT, sizeof NEOHID_PRODUCT);
    case HID_STRING_ID_ISERIALNUMBER: return CopyToRequest(Request, NEOHID_SERIAL, sizeof NEOHID_SERIAL);
    default: return STATUS_INVALID_PARAMETER;
    }
}

VOID EvtIoDeviceControl(_In_ WDFQUEUE Queue, _In_ WDFREQUEST Request, _In_ size_t OutputBufferLength, _In_ size_t InputBufferLength, _In_ ULONG IoControlCode)
{
    PDEVICE_CONTEXT context = GetDeviceContext(WdfIoQueueGetDevice(Queue));
    NTSTATUS status;
    UNREFERENCED_PARAMETER(OutputBufferLength);
    UNREFERENCED_PARAMETER(InputBufferLength);
    switch (IoControlCode) {
    case IOCTL_HID_GET_DEVICE_DESCRIPTOR:
        status = CopyToRequest(Request, &context->HidDescriptor, context->HidDescriptor.bLength);
        break;
    case IOCTL_HID_GET_DEVICE_ATTRIBUTES:
        status = CopyToRequest(Request, &context->Attributes, sizeof context->Attributes);
        break;
    case IOCTL_HID_GET_REPORT_DESCRIPTOR:
        status = CopyToRequest(Request, NeoReportDescriptor, sizeof NeoReportDescriptor);
        break;
    case IOCTL_HID_READ_REPORT:
        status = WdfRequestForwardToIoQueue(Request, context->ManualQueue);
        if (NT_SUCCESS(status)) return;   /* completed by EvtTimer */
        break;
    case IOCTL_UMDF_HID_GET_FEATURE:
        status = GetReport(context, Request, FALSE);
        break;
    case IOCTL_UMDF_HID_GET_INPUT_REPORT:
        status = GetReport(context, Request, TRUE);
        break;
    case IOCTL_HID_WRITE_REPORT:
    case IOCTL_UMDF_HID_SET_FEATURE:
    case IOCTL_UMDF_HID_SET_OUTPUT_REPORT:
        status = AcceptWrite(Request);
        break;
    case IOCTL_HID_GET_STRING:
        status = GetString(Request);
        break;
    default:
        status = STATUS_NOT_IMPLEMENTED;
        break;
    }
    WdfRequestComplete(Request, status);
}

NTSTATUS EvtDeviceAdd(_In_ WDFDRIVER Driver, _Inout_ PWDFDEVICE_INIT DeviceInit)
{
    WDF_OBJECT_ATTRIBUTES attributes;
    WDF_IO_QUEUE_CONFIG queueConfig;
    WDF_TIMER_CONFIG timerConfig;
    WDFDEVICE device;
    WDFQUEUE queue;
    PDEVICE_CONTEXT context;
    NTSTATUS status;
    UNREFERENCED_PARAMETER(Driver);

    WdfFdoInitSetFilter(DeviceInit);   /* mshidumdf is the function driver */
    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, DEVICE_CONTEXT);
    attributes.EvtCleanupCallback = EvtDeviceCleanup;
    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status)) return status;

    context = GetDeviceContext(device);
    context->Device = device;
    context->Pipe = INVALID_HANDLE_VALUE;
    context->ReconnectTicks = NEOHID_RECONNECT_TICKS;   /* try the pipe on the first tick */
    context->Stream.Used = 0;
    neo_neutral(context->Latest);
    context->Attributes.Size = sizeof(HID_DEVICE_ATTRIBUTES);
    context->Attributes.VendorID = NEOHID_VID;
    context->Attributes.ProductID = NEOHID_PID;
    context->Attributes.VersionNumber = NEOHID_VERSION;
    context->HidDescriptor.bLength = sizeof(HID_DESCRIPTOR);
    context->HidDescriptor.bDescriptorType = HID_HID_DESCRIPTOR_TYPE;
    context->HidDescriptor.bcdHID = 0x0111;
    context->HidDescriptor.bCountry = 0;
    context->HidDescriptor.bNumDescriptors = 1;
    context->HidDescriptor.DescriptorList[0].bReportType = HID_REPORT_DESCRIPTOR_TYPE;
    context->HidDescriptor.DescriptorList[0].wReportLength = sizeof NeoReportDescriptor;

    WDF_OBJECT_ATTRIBUTES_INIT(&attributes);
    attributes.ParentObject = device;
    status = WdfSpinLockCreate(&attributes, &context->Lock);
    if (!NT_SUCCESS(status)) return status;

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchParallel);
    queueConfig.EvtIoDeviceControl = EvtIoDeviceControl;   /* mshidumdf converts internal IOCTLs */
    status = WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, &queue);
    if (!NT_SUCCESS(status)) return status;

    WDF_IO_QUEUE_CONFIG_INIT(&queueConfig, WdfIoQueueDispatchManual);
    status = WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, &context->ManualQueue);
    if (!NT_SUCCESS(status)) return status;

    WDF_TIMER_CONFIG_INIT_PERIODIC(&timerConfig, EvtTimer, NEOHID_PERIOD_MS);
    WDF_OBJECT_ATTRIBUTES_INIT(&attributes);
    attributes.ParentObject = device;
    status = WdfTimerCreate(&timerConfig, &attributes, &context->Timer);
    if (!NT_SUCCESS(status)) return status;
    WdfTimerStart(context->Timer, WDF_REL_TIMEOUT_IN_MS(NEOHID_PERIOD_MS));
    return STATUS_SUCCESS;
}
