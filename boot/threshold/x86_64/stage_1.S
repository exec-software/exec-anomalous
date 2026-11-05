.set manifest_address, 0x0600
.set manifest_magic_low, 0x45524854
.set manifest_magic_high, 0x444c4853
.set manifest_version, 1
.set manifest_size, 96
.set manifest_hash_offset, 16
.set manifest_hashed_offset, 20
.set stage_2_lba_offset, 24
.set stage_2_file_size_offset, 32
.set stage_2_sector_count_offset, 36
.set stage_2_load_address_offset, 40
.set stage_2_entry_address_offset, 44
.set stage_2_hash_offset, 48
.set stage_2_load_address, 0x10000
.set maximum_stage_2_sectors, 127
.set fnv_offset_basis, 0x811c9dc5
.set fnv_prime, 0x01000193

.section .boot,"ax"
.code16
.global stage_1_start
.type stage_1_start, @function
stage_1_start:
    cli
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw $0x7c00, %sp
    movb %dl, boot_drive
    sti
    cld

    # Require enhanced-disk reads; both Threshold stages use explicit LBAs.
    movw $0x55aa, %bx
    movb $0x41, %ah
    movb boot_drive, %dl
    int $0x13
    jc boot_error
    cmpw $0xaa55, %bx
    jne boot_error
    testw $1, %cx
    jz boot_error
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es

    # LBA 1 is a fixed, versioned manifest. Stage 1 trusts no load parameters
    # until the manifest header and its FNV-1a invariant have been checked.
    movw $disk_address_packet, %si
    movb $0x42, %ah
    movb boot_drive, %dl
    int $0x13
    jc boot_error
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es

    cmpl $manifest_magic_low, manifest_address
    jne manifest_error
    cmpl $manifest_magic_high, manifest_address + 4
    jne manifest_error
    cmpl $manifest_version, manifest_address + 8
    jne manifest_error
    cmpl $manifest_size, manifest_address + 12
    jne manifest_error

    movw $(manifest_address + manifest_hashed_offset), %si
    movw $(manifest_size - manifest_hashed_offset), %cx
    call fnv_1a
    cmpl manifest_address + manifest_hash_offset, %edx
    jne manifest_error

    cmpl $0, manifest_address + stage_2_lba_offset + 4
    jne manifest_error
    cmpl $2, manifest_address + stage_2_lba_offset
    jb manifest_error
    movl manifest_address + stage_2_sector_count_offset, %eax
    testl %eax, %eax
    jz manifest_error
    cmpl $maximum_stage_2_sectors, %eax
    ja manifest_error
    movl manifest_address + stage_2_file_size_offset, %eax
    testl %eax, %eax
    jz manifest_error
    cmpl $(maximum_stage_2_sectors * 512), %eax
    ja manifest_error
    addl $511, %eax
    shrl $9, %eax
    cmpl manifest_address + stage_2_sector_count_offset, %eax
    jne manifest_error
    cmpl $stage_2_load_address, manifest_address + stage_2_load_address_offset
    jne manifest_error
    cmpl $stage_2_load_address, manifest_address + stage_2_entry_address_offset
    jne manifest_error

    movw manifest_address + stage_2_sector_count_offset, %ax
    movw %ax, disk_address_packet + 2
    movw $0, disk_address_packet + 4
    movw $(stage_2_load_address >> 4), disk_address_packet + 6
    movl manifest_address + stage_2_lba_offset, %eax
    movl %eax, disk_address_packet + 8
    movl manifest_address + stage_2_lba_offset + 4, %eax
    movl %eax, disk_address_packet + 12
    movw $disk_address_packet, %si
    movb $0x42, %ah
    movb boot_drive, %dl
    int $0x13
    jc boot_error
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es

    movl manifest_address + stage_2_hash_offset, %ebp
    movw manifest_address + stage_2_file_size_offset, %cx
    movw $(stage_2_load_address >> 4), %ax
    movw %ax, %ds
    xorw %si, %si
    call fnv_1a
    xorw %ax, %ax
    movw %ax, %ds
    cmpl %ebp, %edx
    jne manifest_error

    movb boot_drive, %dl
    ljmp $(stage_2_load_address >> 4), $0x0000

fnv_1a:
    movl $fnv_offset_basis, %edx
fnv_1a_next:
    lodsb
    movzbl %al, %eax
    xorl %eax, %edx
    imull $fnv_prime, %edx, %edx
    loop fnv_1a_next
    ret

manifest_error:
    movw $manifest_error_message, %si
    jmp print_error

boot_error:
    movw $boot_error_message, %si
print_error:
    xorw %ax, %ax
    movw %ax, %ds
print_character:
    lodsb
    testb %al, %al
    jz halt
    movb $0x0e, %ah
    movw $0x0007, %bx
    int $0x10
    jmp print_character

halt:
    cli
    hlt
    jmp halt

.balign 4
disk_address_packet:
    .byte 0x10
    .byte 0
    .word 1
    .word manifest_address
    .word 0
    .quad 1

boot_drive:
    .byte 0
boot_error_message:
    .asciz "Threshold: disk error\r\n"
manifest_error_message:
    .asciz "Threshold: invalid manifest\r\n"

.org 510
.word 0xaa55
.size stage_1_start, . - stage_1_start

.section .note.GNU-stack,"",@progbits
