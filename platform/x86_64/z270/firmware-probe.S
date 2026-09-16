/* RAM-only reset-vector probe; no PCI, disk, firmware services or host reset.
 * GNU as --32, ld -m elf_i386 -Ttext=0, objcopy -O binary -j .text.
 * The branch's CS.base=ffff0000 and IP=fff0 fetch from GPA fffffff0.
 */
.code16
.section .text
.global _start
_start:
    cli
    movw $0x3f8, %dx
    /* No stack, string reads, or UART interrupts needed for polled log. */
    movb $'H', %al
    outb %al, %dx
    movb $'V', %al
    outb %al, %dx
    movb $'F', %al
    outb %al, %dx
    movb $'W', %al
    outb %al, %dx
    movb $'0', %al
    outb %al, %dx
    movb $'\r', %al
    outb %al, %dx
    movb $'\n', %al
    outb %al, %dx
1:  hlt
    jmp 1b
.org 0xfff0, 0xff
    jmp _start
.org 0x10000, 0xff
