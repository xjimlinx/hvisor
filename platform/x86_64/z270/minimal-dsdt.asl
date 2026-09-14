// Trusted Zone0 native PCI topology. Platform control is not zone-isolated.
DefinitionBlock ("", "DSDT", 2, "HVISOR", "Z270ALL", 1)
{
    Method (_PIC, 1, NotSerialized) { }
    Scope (_SB)
    {
        Device (CP00) { Name (_HID, "ACPI0007") Name (_UID, Zero) }
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
                    0, 0, 6, 0, 7)
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    NonCacheable, ReadWrite, 0, 0xDE000000, 0xDF3FFFFF, 0, 0x1400000)
                // Integrated Kaby Lake display BAR0.  Keep the firmware
                // assignment visible to a guest so Linux does not relocate
                // it into an un-mapped high MMIO address (the previous
                // 0xfef9xxxx fault was exactly such a relocation).
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    NonCacheable, ReadWrite, 0, 0xDD000000, 0xDDFFFFFF, 0, 0x1000000)
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    Prefetchable, ReadWrite, 0, 0xC0000000, 0xD1FFFFFF, 0, 0x12000000)
                // Integrated display stolen/GTT aperture BAR2.
                DWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
                    Prefetchable, ReadWrite, 0, 0xB0000000, 0xBFFFFFFF, 0, 0x10000000)
                WordIO (ResourceProducer, MinFixed, MaxFixed, PosDecode, EntireRange,
                    0, 0xE000, 0xFFFF, 0, 0x2000)
            })
            Name (_PRT, Package ()
            {
                // Native AR00 upstream bridge routes (A..D -> GSI16..19).
                Package () { 0x0001FFFF, Zero, Zero, 0x10 },
                Package () { 0x0001FFFF, One, Zero, 0x11 },
                Package () { 0x0001FFFF, 0x02, Zero, 0x12 },
                Package () { 0x0001FFFF, 0x03, Zero, 0x13 },
                Package () { 0x001BFFFF, Zero, Zero, 0x10 },
                Package () { 0x001BFFFF, One, Zero, 0x11 },
                Package () { 0x001BFFFF, 0x02, Zero, 0x12 },
                Package () { 0x001BFFFF, 0x03, Zero, 0x13 },
                Package () { 0x001CFFFF, Zero, Zero, 0x10 },
                Package () { 0x001CFFFF, One, Zero, 0x11 },
                Package () { 0x001CFFFF, 0x02, Zero, 0x12 },
                Package () { 0x001CFFFF, 0x03, Zero, 0x13 },
                Package () { 0x001DFFFF, Zero, Zero, 0x10 },
                Package () { 0x001DFFFF, One, Zero, 0x11 },
                Package () { 0x001DFFFF, 0x02, Zero, 0x12 },
                Package () { 0x001DFFFF, 0x03, Zero, 0x13 },
                // Native DSDT AR00, PICM/APIC routing. Keep all four pins:
                // firmware/driver initialization may change advertised pins.
                Package () { 0x0014FFFF, Zero, Zero, 0x10 },
                Package () { 0x0014FFFF, One, Zero, 0x11 },
                Package () { 0x0014FFFF, 0x02, Zero, 0x12 },
                Package () { 0x0014FFFF, 0x03, Zero, 0x13 },
                Package () { 0x0016FFFF, Zero, Zero, 0x10 },
                Package () { 0x0016FFFF, One, Zero, 0x11 },
                Package () { 0x0016FFFF, 0x02, Zero, 0x12 },
                Package () { 0x0016FFFF, 0x03, Zero, 0x13 },
                Package () { 0x0017FFFF, Zero, Zero, 0x10 },
                Package () { 0x001FFFFF, Zero, Zero, 0x10 },
                // Preserve the working pin-B route: native SMBus IRQ16.
                // Raw firmware AR00 says 17; do not silently switch the live
                // controller contract. SMBus remains polling, HDA/NIC MSI.
                Package () { 0x001FFFFF, One, Zero, 0x10 },
                // The integrated Kaby Lake display endpoint is INT A.  The
                // native board routes it through AR00 to GSI16; without this
                // entry Linux sees no IRQ for 00:02.0 and the i915 HPD setup
                // takes an invalid path after the DDI-A probe fails.
                Package () { 0x0002FFFF, Zero, Zero, 0x10 }
            })
            Device (SAT0) { Name (_ADR, 0x00170000) }
            Device (XHC0) { Name (_ADR, 0x00140000) }
            Device (HECI) { Name (_ADR, 0x00160000) }
            Device (LPCB) { Name (_ADR, 0x001F0000) }
            Device (PMC0) { Name (_ADR, 0x001F0002) }
            Device (HDA1) { Name (_ADR, 0x001F0003) }
            Device (SBUS) { Name (_ADR, 0x001F0004) }
            Device (GLAN) { Name (_ADR, 0x001F0006) }
            Device (PEG0)
            {
                Name (_ADR, 0x00010000)
                Name (_PRT, Package ()
                {
                    Package () { 0x0000FFFF, Zero, Zero, 0x10 },
                    Package () { 0x0000FFFF, One, Zero, 0x11 },
                    // Keep earlier HDMI pin-C fallback; active drivers use MSI.
                    Package () { 0x0000FFFF, 0x02, Zero, 0x11 }
                })
                Device (GFX0) { Name (_ADR, Zero) }
                Device (HDA0) { Name (_ADR, One) }
            }
            Device (RP05)
            {
                Name (_ADR, 0x001C0004)
                // Native RP05._PRT -> AR08.
                Name (_PRT, Package ()
                {
                    Package () { 0x0000FFFF, Zero, Zero, 0x10 },
                    Package () { 0x0000FFFF, One, Zero, 0x11 },
                    Package () { 0x0000FFFF, 0x02, Zero, 0x12 },
                    Package () { 0x0000FFFF, 0x03, Zero, 0x13 }
                })
                Device (XHC1) { Name (_ADR, Zero) }
            }
            Device (RP08)
            {
                Name (_ADR, 0x001C0007)
                // Native RP08._PRT -> AR0B.
                Name (_PRT, Package ()
                {
                    Package () { 0x0000FFFF, Zero, Zero, 0x13 },
                    Package () { 0x0000FFFF, One, Zero, 0x10 },
                    Package () { 0x0000FFFF, 0x02, Zero, 0x11 },
                    Package () { 0x0000FFFF, 0x03, Zero, 0x12 }
                })
                Device (WLAN) { Name (_ADR, Zero) }
            }
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
