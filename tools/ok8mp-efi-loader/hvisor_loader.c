// SPDX-License-Identifier: MIT
// Minimal AArch64 EFI hand-off loader for Hvisor on the Forlinx OK8MP-C.
// The single supported deployment profile is desktop-2g5.

typedef unsigned char UINT8;
typedef unsigned short UINT16;
typedef unsigned int UINT32;
typedef unsigned long long UINT64;
typedef unsigned long long UINTN;
typedef UINT16 CHAR16;
typedef void *EFI_HANDLE;
typedef UINT64 EFI_STATUS;
typedef UINT64 EFI_PHYSICAL_ADDRESS;
typedef UINT32 EFI_MEMORY_TYPE;
typedef UINT32 EFI_INTERFACE_TYPE;

#define EFI_SUCCESS 0
#define EFI_BUFFER_TOO_SMALL 0x8000000000000005ULL
#define EFI_ERROR(status) (((status) & 0x8000000000000000ULL) != 0)
#define EFI_FILE_MODE_READ 0x0000000000000001ULL
#define EFI_LOADER_DATA 2
#define EFI_PAGE_SIZE 4096ULL

#define HVISOR_LOAD_ADDR 0x40400000ULL
#define BOARD_DTB_ADDR 0x40000000ULL
#define ZONE0_DTB_ADDR 0xa0000000ULL
#define LINUX_LOAD_ADDR 0xa0400000ULL
#define HVISOR_SCRATCH_ADDR 0x41000000ULL

typedef struct {
    UINT32 Data1;
    UINT16 Data2;
    UINT16 Data3;
    UINT8 Data4[8];
} EFI_GUID;

typedef struct {
    UINT64 Signature;
    UINT32 Revision;
    UINT32 HeaderSize;
    UINT32 CRC32;
    UINT32 Reserved;
} EFI_TABLE_HEADER;

typedef struct EFI_FILE_PROTOCOL EFI_FILE_PROTOCOL;
typedef EFI_STATUS (*EFI_FILE_OPEN)(EFI_FILE_PROTOCOL *, EFI_FILE_PROTOCOL **, CHAR16 *, UINT64, UINT64);
typedef EFI_STATUS (*EFI_FILE_CLOSE)(EFI_FILE_PROTOCOL *);
typedef EFI_STATUS (*EFI_FILE_READ)(EFI_FILE_PROTOCOL *, UINTN *, void *);
typedef EFI_STATUS (*EFI_FILE_GET_INFO)(EFI_FILE_PROTOCOL *, EFI_GUID *, UINTN *, void *);

struct EFI_FILE_PROTOCOL {
    UINT64 Revision;
    EFI_FILE_OPEN Open;
    EFI_FILE_CLOSE Close;
    void *Delete;
    EFI_FILE_READ Read;
    void *Write;
    void *GetPosition;
    void *SetPosition;
    EFI_FILE_GET_INFO GetInfo;
    void *SetInfo;
    void *Flush;
    void *OpenEx;
    void *ReadEx;
    void *WriteEx;
    void *FlushEx;
};

typedef struct {
    UINT64 Revision;
    EFI_STATUS (*OpenVolume)(void *, EFI_FILE_PROTOCOL **);
} EFI_SIMPLE_FILE_SYSTEM_PROTOCOL;

typedef struct {
    UINT64 Size;
    UINT64 FileSize;
    UINT64 PhysicalSize;
    UINT8 CreateTime[16];
    UINT8 LastAccessTime[16];
    UINT8 ModificationTime[16];
    UINT64 Attribute;
    CHAR16 FileName[1];
} EFI_FILE_INFO;

typedef struct {
    UINT32 Type;
    UINT32 Pad;
    EFI_PHYSICAL_ADDRESS PhysicalStart;
    UINT64 VirtualStart;
    UINT64 NumberOfPages;
    UINT64 Attribute;
} EFI_MEMORY_DESCRIPTOR;

typedef struct {
    EFI_TABLE_HEADER Hdr;
    void *RaiseTPL;
    void *RestoreTPL;
    EFI_STATUS (*AllocatePages)(UINTN, EFI_MEMORY_TYPE, UINTN, EFI_PHYSICAL_ADDRESS *);
    EFI_STATUS (*FreePages)(EFI_PHYSICAL_ADDRESS, UINTN);
    EFI_STATUS (*GetMemoryMap)(UINTN *, EFI_MEMORY_DESCRIPTOR *, UINTN *, UINTN *, UINT32 *);
    void *AllocatePool;
    void *FreePool;
    void *CreateEvent;
    void *SetTimer;
    void *WaitForEvent;
    void *SignalEvent;
    void *CloseEvent;
    void *CheckEvent;
    void *InstallProtocolInterface;
    void *ReinstallProtocolInterface;
    void *UninstallProtocolInterface;
    EFI_STATUS (*HandleProtocol)(EFI_HANDLE, EFI_GUID *, void **);
    void *Reserved;
    void *RegisterProtocolNotify;
    void *LocateHandle;
    void *LocateDevicePath;
    void *InstallConfigurationTable;
    void *LoadImage;
    void *StartImage;
    void *Exit;
    void *UnloadImage;
    EFI_STATUS (*ExitBootServices)(EFI_HANDLE, UINTN);
} EFI_BOOT_SERVICES;

typedef struct {
    EFI_TABLE_HEADER Hdr;
    CHAR16 *FirmwareVendor;
    UINT32 FirmwareRevision;
    EFI_HANDLE ConsoleInHandle;
    void *ConIn;
    EFI_HANDLE ConsoleOutHandle;
    void *ConOut;
    EFI_HANDLE StandardErrorHandle;
    void *StdErr;
    void *RuntimeServices;
    EFI_BOOT_SERVICES *BootServices;
    UINTN NumberOfTableEntries;
    void *ConfigurationTable;
} EFI_SYSTEM_TABLE;

typedef struct {
    UINT32 Revision;
    EFI_HANDLE ParentHandle;
    EFI_SYSTEM_TABLE *SystemTable;
    EFI_HANDLE DeviceHandle;
    void *FilePath;
    void *Reserved;
    UINT32 LoadOptionsSize;
    void *LoadOptions;
    void *ImageBase;
    UINT64 ImageSize;
} EFI_LOADED_IMAGE_PROTOCOL;

static EFI_GUID loaded_image_guid = {
    0x5b1b31a1, 0x9562, 0x11d2, {0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b}
};
static EFI_GUID simple_fs_guid = {
    0x964e5b22, 0x6459, 0x11d2, {0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b}
};
static EFI_GUID file_info_guid = {
    0x09576e92, 0x6d3f, 0x11d2, {0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b}
};

static UINT8 memory_map[65536] __attribute__((aligned(8)));
static UINT8 file_info[1024] __attribute__((aligned(8)));

static const CHAR16 board_dtb_path[] = u"/boot/mainline/OK8MP-C-mainline.dtb";
static const CHAR16 linux_path[] = u"/boot/mainline/Image-7.2";
static const CHAR16 hvisor_path[] = u"/boot/hvisor-profiles/desktop-2g5/hvisor.bin";
static const CHAR16 zone0_path[] = u"/boot/hvisor-profiles/desktop-2g5/zone0.dtb";

static void copy_bytes(void *dst, const void *src, UINTN size) {
    UINT8 *d = dst;
    const UINT8 *s = src;
    while (size--) *d++ = *s++;
}

static UINT32 be32(const UINT8 *p) {
    return ((UINT32)p[0] << 24) | ((UINT32)p[1] << 16) | ((UINT32)p[2] << 8) | p[3];
}

typedef struct { void *Reset; EFI_STATUS (*OutputString)(void *, const CHAR16 *); } CONSOLE;
static EFI_SYSTEM_TABLE *st;
static void say(const CHAR16 *s) {
    CONSOLE *c = st->ConOut;
    if (c) c->OutputString(c, s);
}
static void status_hex(EFI_STATUS value) {
    CHAR16 s[21] = u"0x0000000000000000\r\n";
    const CHAR16 digits[] = u"0123456789abcdef";
    for (UINTN i = 0; i < 16; i++) s[17-i] = digits[(value >> (i*4)) & 15];
    say(s);
}
static struct { UINT64 address, bytes, buffer, used; } regions[] = {
    {BOARD_DTB_ADDR, 2*1024*1024, 0, 0}, {HVISOR_LOAD_ADDR, 8*1024*1024, 0, 0},
    {HVISOR_SCRATCH_ADDR, 2*1024*1024, 0, 0}, {ZONE0_DTB_ADDR, 2*1024*1024, 0, 0},
    {LINUX_LOAD_ADDR, 128*1024*1024, 0, 0},
};
static UINT64 loader_start, loader_end;
static int overlap(UINT64 a, UINT64 len, UINT64 b, UINT64 end) {
    return a < end && a + len > b;
}
static int reclaimable(EFI_BOOT_SERVICES *bs, UINT64 start, UINT64 bytes) {
    UINTN size = sizeof(memory_map), key, stride;
    UINT32 version;
    UINT64 sp;
    __asm__ volatile("mov %0, sp" : "=r"(sp));
    if (overlap(start, bytes, loader_start, loader_end) ||
        overlap(start, bytes, sp - 65536, sp + 65536)) return 0;
    EFI_STATUS ret = bs->GetMemoryMap(&size, (void *)memory_map, &key, &stride, &version);
    if (EFI_ERROR(ret) || stride < sizeof(EFI_MEMORY_DESCRIPTOR)) return 0;
    for (UINTN off = 0; off + stride <= size; off += stride) {
        EFI_MEMORY_DESCRIPTOR *d = (void *)(memory_map + off);
        if (d->Type == 4 && !(d->Attribute & (1ULL << 63)) &&
            d->PhysicalStart <= start &&
            d->PhysicalStart + d->NumberOfPages * EFI_PAGE_SIZE >= start + bytes) return 1;
    }
    return 0;
}
static UINTN allocated;
static void explain_region(EFI_BOOT_SERVICES *bs, UINT64 start, UINT64 bytes) {
    UINTN size = sizeof(memory_map), key, stride;
    UINT32 version;
    EFI_STATUS ret = bs->GetMemoryMap(&size, (EFI_MEMORY_DESCRIPTOR *)memory_map,
                                     &key, &stride, &version);
    if (EFI_ERROR(ret) || stride < sizeof(EFI_MEMORY_DESCRIPTOR)) {
        say(u"Cannot inspect EFI map: "); status_hex(ret); return;
    }
    say(u"Requested length: "); status_hex(bytes);
    UINTN matches = 0;
    for (UINTN offset = 0; offset + stride <= size; offset += stride) {
        EFI_MEMORY_DESCRIPTOR *d = (void *)(memory_map + offset);
        UINT64 end = d->PhysicalStart + d->NumberOfPages * EFI_PAGE_SIZE;
        if (end <= start || d->PhysicalStart >= start + bytes) continue;
        ++matches;
        say(u"EFI descriptor type (7=free): "); status_hex(d->Type);
        say(u"  start: "); status_hex(d->PhysicalStart);
        say(u"  end exclusive: "); status_hex(end);
        say(u"  attributes: "); status_hex(d->Attribute);
    }
    if (!matches) say(u"Requested range is absent from EFI memory map\r\n");
}
static void release_regions(EFI_BOOT_SERVICES *bs) {
    while (allocated) {
        --allocated;
        bs->FreePages(regions[allocated].buffer, regions[allocated].bytes / EFI_PAGE_SIZE);
    }
}
static EFI_STATUS reserve_regions(EFI_BOOT_SERVICES *bs) {
    for (; allocated < sizeof(regions)/sizeof(regions[0]); allocated++) {
        UINT64 address = regions[allocated].address;
        EFI_STATUS ret = bs->AllocatePages(2 /* AllocateAddress */, EFI_LOADER_DATA,
                                          regions[allocated].bytes / EFI_PAGE_SIZE, &address);
        if (EFI_ERROR(ret)) {
            if (allocated >= 3 && reclaimable(bs, regions[allocated].address, regions[allocated].bytes)) {
                address = 0;
                ret = bs->AllocatePages(0, EFI_LOADER_DATA, regions[allocated].bytes / EFI_PAGE_SIZE, &address);
                if (!EFI_ERROR(ret)) {
                    say(u"R4: staging until ExitBootServices for destination "); status_hex(regions[allocated].address);
                    regions[allocated].buffer = address;
                    continue;
                }
            }
            say(u"Hvisor: cannot reserve address "); status_hex(regions[allocated].address);
            explain_region(bs, regions[allocated].address, regions[allocated].bytes);
            release_regions(bs);
            return ret;
        }
        regions[allocated].buffer = address;
    }
    return EFI_SUCCESS;
}

/* This board's U-Boot uses identity mappings. Refuse other mappings instead
 * of disabling the MMU underneath a virtually relocated EFI application. */
static int identity(UINT64 address) {
    UINT64 par;
    __asm__ volatile("at s1e2r, %1; isb; mrs %0, par_el1" : "=r"(par) : "r"(address) : "memory");
    return !(par & 1) && ((par & 0x0000fffffffff000ULL) == (address & ~4095ULL));
}

/* Cortex-A53 / ARMv8.0 set-way clean, before Hvisor invalidates caches.
 * No EFI calls or stack accesses after MMU/cache disable. */
__attribute__((naked, noreturn)) static void handoff(UINT64 kernel, UINT64 kernel_size, UINT64 dtb, UINT64 dtb_size) {
    __asm__ volatile(
        "mov x12, x0\n mov x13, x1\n mov x14, x2\n mov x15, x3\n"
        "msr daifset, #15\n"
        "dsb sy\n"
        "mrs x0, clidr_el1\n"
        "ubfx x3, x0, #24, #3\n"
        "mov x1, #0\n"
        "1: cmp x1, x3\n b.ge 5f\n"
        "add x2, x1, x1, lsl #1\n lsr x2, x0, x2\n and x2, x2, #7\n"
        "cmp x2, #2\n b.lt 4f\n"
        "lsl x10, x1, #1\n msr csselr_el1, x10\n isb\n mrs x2, ccsidr_el1\n"
        "and x4, x2, #7\n add x4, x4, #4\n"
        "ubfx x5, x2, #3, #10\n clz w6, w5\n ubfx x7, x2, #13, #15\n"
        "2: mov x8, x5\n"
        "3: lsl x9, x7, x4\n lsl x11, x8, x6\n orr x9, x9, x11\n orr x9, x9, x10\n"
        "dc cisw, x9\n subs x8, x8, #1\n b.ge 3b\n subs x7, x7, #1\n b.ge 2b\n"
        "4: add x1, x1, #1\n b 1b\n"
        "5: dsb sy\n msr csselr_el1, xzr\n"
        "mrs x0, sctlr_el2\n bic x0, x0, #1\n bic x0, x0, #4\n bic x0, x0, #4096\n"
        "msr sctlr_el2, x0\n isb\n ic iallu\n dsb sy\n isb\n"
        "mov x4, #0xa0400000\n cmp x12, x4\n b.eq 7f\n"
        /* Both buffers are page aligned. Scalar aligned 64-bit accesses
         * also work with the Device attributes used while MMU is off.
         * Avoid SIMD and unaligned/pair accesses in this handoff. */
        "6: cmp x13, #32\n b.lo 10f\n"
        "ldr x5, [x12]\n ldr x6, [x12, #8]\n ldr x7, [x12, #16]\n ldr x8, [x12, #24]\n"
        "str x5, [x4]\n str x6, [x4, #8]\n str x7, [x4, #16]\n str x8, [x4, #24]\n"
        "add x12, x12, #32\n add x4, x4, #32\n sub x13, x13, #32\n b 6b\n"
        "10: cmp x13, #8\n b.lo 11f\n ldr x5, [x12], #8\n str x5, [x4], #8\n sub x13, x13, #8\n b 10b\n"
        "11: cbz x13, 7f\n ldrb w5, [x12], #1\n strb w5, [x4], #1\n sub x13, x13, #1\n b 11b\n"
        "7: mov x4, #0xa0000000\n cmp x14, x4\n b.eq 9f\n"
        "8: cbz x15, 9f\n ldrb w5, [x14], #1\n strb w5, [x4], #1\n sub x15, x15, #1\n b 8b\n"
        "9: dsb sy\n ic iallu\n dsb sy\n isb\n"
        "mov x0, #0x40000000\n mov x1, xzr\n mov x2, xzr\n mov x3, xzr\n"
        "mov x4, #0x40400000\n br x4\n");
}

static EFI_STATUS read_file(EFI_FILE_PROTOCOL *root, const CHAR16 *path, void *destination, UINTN limit, UINTN *size) {
    EFI_FILE_PROTOCOL *file = 0;
    EFI_STATUS status = root->Open(root, &file, (CHAR16 *)path, EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status)) { say(u"File open failed: "); say(path); say(u"\r\n"); return status; }

    UINTN info_size = sizeof(file_info);
    status = file->GetInfo(file, &file_info_guid, &info_size, file_info);
    if (EFI_ERROR(status)) { file->Close(file); return status; }
    UINTN bytes = (UINTN)((EFI_FILE_INFO *)file_info)->FileSize;
    if (bytes > limit) { file->Close(file); return 0x8000000000000001ULL; }

    UINTN read = bytes;
    status = file->Read(file, &read, destination);
    file->Close(file);
    if (EFI_ERROR(status) || read != bytes) return 0x8000000000000001ULL;
    *size = bytes;
    return EFI_SUCCESS;
}

static EFI_STATUS size_region(EFI_FILE_PROTOCOL *root, const CHAR16 *path, UINTN index) {
    EFI_FILE_PROTOCOL *file = 0;
    EFI_STATUS status = root->Open(root, &file, (CHAR16 *)path, EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status)) return status;
    UINTN info_size = sizeof(file_info);
    status = file->GetInfo(file, &file_info_guid, &info_size, file_info);
    file->Close(file);
    if (EFI_ERROR(status)) return status;
    UINT64 bytes = ((EFI_FILE_INFO *)file_info)->FileSize;
    if (!bytes || bytes > regions[index].bytes) return 0x8000000000000004ULL;
    regions[index].bytes = (bytes + EFI_PAGE_SIZE - 1) & ~(EFI_PAGE_SIZE - 1);
    say(u"R4: file "); say(path); say(u" bytes "); status_hex(bytes);
    return EFI_SUCCESS;
}

static EFI_STATUS exit_boot_services(EFI_HANDLE image, EFI_BOOT_SERVICES *bs) {
    UINTN map_size = sizeof(memory_map);
    UINTN map_key = 0;
    UINTN descriptor_size = 0;
    UINT32 descriptor_version = 0;
    EFI_STATUS status;
    for (UINTN attempt = 0; attempt < 2; attempt++) {
    map_size = sizeof(memory_map);
    status = bs->GetMemoryMap(&map_size, (EFI_MEMORY_DESCRIPTOR *)memory_map,
                                         &map_key, &descriptor_size, &descriptor_version);
    if (EFI_ERROR(status)) {
        if (attempt) for (;;) __asm__ volatile("wfe");
        return status;
    }
    status = bs->ExitBootServices(image, map_key);
    if (!EFI_ERROR(status)) return status;
    }
    /* Firmware may already have partially shut down. Never return to GRUB. */
    for (;;) __asm__ volatile("wfe");
}

__attribute__((noreturn)) static void enter_hvisor(void) {
    handoff(regions[4].buffer, regions[4].used, regions[3].buffer, regions[3].used);
}

EFI_STATUS efi_main(EFI_HANDLE image, EFI_SYSTEM_TABLE *system_table) {
    EFI_BOOT_SERVICES *bs = system_table->BootServices;
    EFI_LOADED_IMAGE_PROTOCOL *loaded = 0;
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *fs = 0;
    EFI_FILE_PROTOCOL *root = 0;
    UINTN size = 0;
    st = system_table;
    say(u"Hvisor EFI R4-fast: checking EL2 and memory ownership\r\n");
    UINT64 el, sctlr;
    __asm__ volatile("mrs %0, CurrentEL" : "=r"(el));
    if (el != 8) { say(u"Hvisor requires EL2; refusing handoff\r\n"); return 0x8000000000000003ULL; }
    __asm__ volatile("mrs %0, sctlr_el2" : "=r"(sctlr));
    if ((sctlr & 1) && (!identity((UINTN)handoff) || !identity((UINTN)handoff + 4096))) {
        say(u"Non-identity EFI mapping unsupported\r\n"); return 0x8000000000000003ULL;
    }

    EFI_STATUS status = bs->HandleProtocol(image, &loaded_image_guid, (void **)&loaded);
    if (EFI_ERROR(status)) return status;
    loader_start = (UINTN)loaded->ImageBase;
    loader_end = loader_start + loaded->ImageSize;
    status = bs->HandleProtocol(loaded->DeviceHandle, &simple_fs_guid, (void **)&fs);
    if (EFI_ERROR(status)) return status;
    status = fs->OpenVolume(fs, &root);
    if (EFI_ERROR(status)) return status;

    status = size_region(root, board_dtb_path, 0);
    if (EFI_ERROR(status)) goto fail;
    status = size_region(root, hvisor_path, 2);
    if (EFI_ERROR(status)) goto fail;
    status = size_region(root, zone0_path, 3);
    if (EFI_ERROR(status)) goto fail;
    status = size_region(root, linux_path, 4);
    if (EFI_ERROR(status)) goto fail;

    status = reserve_regions(bs);
    if (EFI_ERROR(status)) { root->Close(root); status_hex(status); return status; }
    for (UINTN i = 0; i < allocated; i++) {
        if ((sctlr & 1) && (!identity(regions[i].buffer) ||
            !identity(regions[i].buffer + regions[i].bytes - 1))) goto invalid;
        if (regions[i].buffer != regions[i].address) {
            for (UINTN j = 0; j < allocated; j++)
                if (overlap(regions[i].buffer, regions[i].bytes, regions[j].address,
                            regions[j].address + regions[j].bytes)) goto invalid;
        }
    }

    status = read_file(root, board_dtb_path, (void *)(UINTN)BOARD_DTB_ADDR, regions[0].bytes, &size);
    if (EFI_ERROR(status)) goto fail;
    if (size < 40 || be32((void *)(UINTN)BOARD_DTB_ADDR) != 0xd00dfeed) goto invalid;
    status = read_file(root, linux_path, (void *)(UINTN)regions[4].buffer, regions[4].bytes, &size);
    if (EFI_ERROR(status)) goto fail;
    regions[4].used = size;
    if (size < 64 || be32((UINT8 *)(UINTN)regions[4].buffer + 56) != 0x41524d64) goto invalid;
    status = read_file(root, zone0_path, (void *)(UINTN)regions[3].buffer, regions[3].bytes, &size);
    if (EFI_ERROR(status)) goto fail;
    regions[3].used = size;
    if (size < 40 || be32((void *)(UINTN)regions[3].buffer) != 0xd00dfeed) goto invalid;
    status = read_file(root, hvisor_path, (void *)(UINTN)HVISOR_SCRATCH_ADDR, regions[2].bytes, &size);
    if (EFI_ERROR(status)) goto fail;
    if (size < 64 || be32((void *)(UINTN)HVISOR_SCRATCH_ADDR) != 0x27051956) goto invalid;

    UINT32 payload_size = be32((const UINT8 *)(UINTN)HVISOR_SCRATCH_ADDR + 12);
    if (!payload_size || (UINTN)payload_size != size - 64 ||
        be32((UINT8 *)(UINTN)HVISOR_SCRATCH_ADDR + 16) != HVISOR_LOAD_ADDR ||
        be32((UINT8 *)(UINTN)HVISOR_SCRATCH_ADDR + 20) != HVISOR_LOAD_ADDR) goto invalid;
    copy_bytes((void *)(UINTN)HVISOR_LOAD_ADDR, (const void *)(UINTN)(HVISOR_SCRATCH_ADDR + 64), payload_size);

    root->Close(root);
    root = 0;
    say(u"Hvisor EFI R4-fast: images staged; exiting EFI then relocating and entering EL2\r\n");
    status = exit_boot_services(image, bs);
    if (EFI_ERROR(status)) goto fail;
    enter_hvisor();
invalid:
    say(u"Invalid image header/size/address\r\n");
    status = 0x8000000000000001ULL;
fail:
    if (root) root->Close(root);
    release_regions(bs);
    status_hex(status);
    return status;
}
