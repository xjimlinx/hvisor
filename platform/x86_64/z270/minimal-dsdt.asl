// Zone0 minimal Z270 hardware contract. No firmware AML/OperationRegions.
// PCI bus 0 only; AHCI/xHCI BARs and INTx routes match captured native data.
DefinitionBlock ("", "DSDT", 2, "HVISOR", "Z270MIN", 1)
{
    Method (_PIC, 1, NotSerialized) { }
    Scope (_SB)
    {
        Device (CP00)
        {
            Name (_HID, "ACPI0007")
            Name (_UID, Zero)
        }
        Device (PCI0)
        {
            Name (_HID, EisaId ("PNP0A08"))
            Name (_CID, EisaId ("PNP0A03"))
            Name (_SEG, Zero)
            Name (_BBN, Zero)
            Name (_UID, Zero)
            Name (_CRS, ResourceTemplate ()
            {
                WordBusNumber (ResourceProducer, MinFixed, MaxFixed, PosDecode,
                    0, 0, 0, 0, 1)
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    NonCacheable, ReadWrite, 0, 0xDF330000, 0xDF33FFFF, 0, 0x10000)
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    NonCacheable, ReadWrite, 0, 0xDF348000, 0xDF349FFF, 0, 0x2000)
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    NonCacheable, ReadWrite, 0, 0xDF34B000, 0xDF34BFFF, 0, 0x1000)
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    NonCacheable, ReadWrite, 0, 0xDF34C000, 0xDF34CFFF, 0, 0x1000)
                WordIO (ResourceProducer, MinFixed, MaxFixed, PosDecode, EntireRange,
                    0, 0xF020, 0xF03F, 0, 0x20)
                WordIO (ResourceProducer, MinFixed, MaxFixed, PosDecode, EntireRange,
                    0, 0xF040, 0xF043, 0, 4)
                WordIO (ResourceProducer, MinFixed, MaxFixed, PosDecode, EntireRange,
                    0, 0xF050, 0xF057, 0, 8)
            })
            Name (_PRT, Package ()
            {
                Package () { 0x0017FFFF, Zero, Zero, 0x10 },
                Package () { 0x0014FFFF, Zero, Zero, 0x10 },
                Package () { 0x0014FFFF, One, Zero, 0x11 },
                Package () { 0x0014FFFF, 0x02, Zero, 0x12 },
                Package () { 0x0014FFFF, 0x03, Zero, 0x13 }
            })
            Device (SAT0) { Name (_ADR, 0x00170000) }
            Device (XHC0) { Name (_ADR, 0x00140000) }
        }
        Device (HPET)
        {
            Name (_HID, EisaId ("PNP0103"))
            Name (_UID, Zero)
            Name (_CRS, ResourceTemplate ()
            {
                Memory32Fixed (ReadWrite, 0xFED00000, 0x400)
            })
        }
    }
}
