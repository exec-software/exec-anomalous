.set manifest_address, 0x0600
.set manifest_magic_low, 0x45524854
.set manifest_magic_high, 0x444c4853
.set manifest_version, 1
.set manifest_size, 96
.set manifest_hash_offset, 16
.set manifest_hashed_offset, 20
.set manifest_flags_offset, 20
.set stage_2_lba_offset, 24
.set stage_2_file_size_offset, 32
.set stage_2_sector_count_offset, 36
.set stage_2_load_address_offset, 40
.set stage_2_entry_address_offset, 44
.set stage_2_reserved_offset, 52
.set kernel_lba_offset, 56
.set kernel_file_size_offset, 64
.set kernel_memory_size_offset, 68
.set kernel_sector_count_offset, 72
.set kernel_staging_address_offset, 76
.set kernel_load_address_offset, 80
.set kernel_entry_address_offset, 84
.set kernel_hash_offset, 88
.set manifest_reserved_offset, 92

.set stage_2_load_address, 0x10000
.set maximum_stage_2_sectors, 127
.set kernel_staging_address, 0x30000
.set kernel_staging_capacity, 0x60000
.set maximum_kernel_sectors, kernel_staging_capacity / 512
.set kernel_load_address, 0x100000
.set maximum_kernel_memory_size, 0x04000000
.set disk_chunk_sectors, 64

.set fnv_offset_basis, 0x811c9dc5
.set fnv_prime, 0x01000193
.set boot_omen_magic_low, 0x4d4f4e41
.set boot_omen_magic_high, 0x544f4f42
.set boot_omen_version, 2
.set boot_omen_size, 88
.set boot_flag_framebuffer, 1
.set boot_flag_memory_map_complete, 1 << 1
.set vbe_mode, 0x118
.set vbe_success, 0x004f
.set vbe_info_request_signature, 0x32454256
.set vbe_info_response_signature, 0x41534556
.set vbe_version_2, 0x0200
.set vbe_version_3, 0x0300
.set vbe_mode_required_attributes, 0x0091
.set smap_signature, 0x534d4150
.set maximum_memory_entries, 64
.set memory_entry_size, 24
.set page_present, 1
.set page_writable, 2
.set page_huge, 128
.set efer_msr, 0xc0000080
.set efer_long_mode_enable, 1 << 8
.set cpuid_flag, 1 << 21
.set cpuid_long_mode, 1 << 29
.set cpuid_msr, 1 << 5
.set cpuid_pae, 1 << 6
.set cpuid_pse, 1 << 3
.set cr0_protected_mode, 1
.set cr0_paging, 1 << 31
.set cr4_pae, 1 << 5
.set nmi_mask, 1 << 7

.section .text.threshold,"ax"
.code16
.global threshold_entry
.type threshold_entry, @function
threshold_entry:
    cli
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %ss
    movw $0x7000, %sp
    sti
    cld

    # Threshold's mutable real-mode workspace remains addressable from CS.
    xorw %ax, %ax
    movw $(boot_workspace_start - threshold_entry), %di
    movw $((boot_workspace_end - boot_workspace_start + 1) / 2), %cx
    rep stosw
    movb %dl, boot_drive_work - threshold_entry
    movb $0x10, disk_address_packet - threshold_entry

    movl $boot_omen_magic_low, boot_omen - threshold_entry
    movl $boot_omen_magic_high, boot_omen + 4 - threshold_entry
    movl $boot_omen_version, boot_omen + 8 - threshold_entry
    movl $boot_omen_size, boot_omen + 12 - threshold_entry
    movzbl %dl, %eax
    movl %eax, boot_omen + 16 - threshold_entry

    call validate_manifest
    call validate_cpu
    call load_kernel
    call discover_memory
    call enable_a20
    call configure_display
    # VBE is the last firmware call; prove it did not disturb the gate before
    # copying the kernel above 1 MiB.
    call check_a20
    jc fatal_a20

    # No IDT exists during the milestone-0 mode switch. Mask NMI as well as
    # ordinary interrupts so an asynchronous event cannot force a triple fault.
    cli
    inb $0x70, %al
    orb $nmi_mask, %al
    outb %al, $0x70
    lgdt gdt_pointer - threshold_entry
    movl %cr0, %eax
    orl $cr0_protected_mode, %eax
    movl %eax, %cr0
    ljmpl $0x08, $protected_mode_entry

validate_manifest:
    cmpl $manifest_magic_low, %fs:manifest_address
    jne fatal_manifest
    cmpl $manifest_magic_high, %fs:manifest_address + 4
    jne fatal_manifest
    cmpl $manifest_version, %fs:manifest_address + 8
    jne fatal_manifest
    cmpl $manifest_size, %fs:manifest_address + 12
    jne fatal_manifest
    cmpl $0, %fs:manifest_address + manifest_flags_offset
    jne fatal_manifest
    cmpl $0, %fs:manifest_address + manifest_reserved_offset
    jne fatal_manifest

    pushw %ds
    xorw %ax, %ax
    movw %ax, %ds
    movw $(manifest_address + manifest_hashed_offset), %si
    movw $(manifest_size - manifest_hashed_offset), %cx
    call fnv_1a
    popw %ds
    cmpl %fs:manifest_address + manifest_hash_offset, %edx
    jne fatal_manifest

    cmpl $0, %fs:manifest_address + stage_2_lba_offset + 4
    jne fatal_manifest
    cmpl $2, %fs:manifest_address + stage_2_lba_offset
    jb fatal_manifest
    movl %fs:manifest_address + stage_2_sector_count_offset, %eax
    testl %eax, %eax
    jz fatal_manifest
    cmpl $maximum_stage_2_sectors, %eax
    ja fatal_manifest
    movl %fs:manifest_address + stage_2_file_size_offset, %eax
    testl %eax, %eax
    jz fatal_manifest
    cmpl $(maximum_stage_2_sectors * 512), %eax
    ja fatal_manifest
    addl $511, %eax
    shrl $9, %eax
    cmpl %fs:manifest_address + stage_2_sector_count_offset, %eax
    jne fatal_manifest
    cmpl $stage_2_load_address, %fs:manifest_address + stage_2_load_address_offset
    jne fatal_manifest
    cmpl $stage_2_load_address, %fs:manifest_address + stage_2_entry_address_offset
    jne fatal_manifest
    cmpl $0, %fs:manifest_address + stage_2_reserved_offset
    jne fatal_manifest

    cmpl $0, %fs:manifest_address + kernel_lba_offset + 4
    jne fatal_manifest
    movl %fs:manifest_address + stage_2_lba_offset, %eax
    addl %fs:manifest_address + stage_2_sector_count_offset, %eax
    jc fatal_manifest
    cmpl %fs:manifest_address + kernel_lba_offset, %eax
    jne fatal_manifest
    movl %fs:manifest_address + kernel_sector_count_offset, %eax
    testl %eax, %eax
    jz fatal_manifest
    cmpl $maximum_kernel_sectors, %eax
    ja fatal_manifest
    movl %fs:manifest_address + kernel_file_size_offset, %eax
    testl %eax, %eax
    jz fatal_manifest
    cmpl $kernel_staging_capacity, %eax
    ja fatal_manifest
    movl %eax, %edx
    addl $511, %eax
    shrl $9, %eax
    cmpl %fs:manifest_address + kernel_sector_count_offset, %eax
    jne fatal_manifest
    movl %fs:manifest_address + kernel_memory_size_offset, %eax
    cmpl %edx, %eax
    jb fatal_manifest
    cmpl $maximum_kernel_memory_size, %eax
    ja fatal_manifest
    cmpl $kernel_staging_address, %fs:manifest_address + kernel_staging_address_offset
    jne fatal_manifest
    cmpl $kernel_load_address, %fs:manifest_address + kernel_load_address_offset
    jne fatal_manifest
    movl %fs:manifest_address + kernel_entry_address_offset, %eax
    cmpl $kernel_load_address, %eax
    jb fatal_manifest
    movl %fs:manifest_address + kernel_load_address_offset, %edx
    addl %fs:manifest_address + kernel_file_size_offset, %edx
    jc fatal_manifest
    cmpl %edx, %eax
    jae fatal_manifest
    ret

validate_cpu:
    pushfl
    popl %eax
    movl %eax, %ecx
    xorl $cpuid_flag, %eax
    pushl %eax
    popfl
    pushfl
    popl %eax
    pushl %ecx
    popfl
    xorl %ecx, %eax
    testl $cpuid_flag, %eax
    jz fatal_cpu

    xorl %eax, %eax
    cpuid
    cmpl $1, %eax
    jb fatal_cpu
    movl $1, %eax
    cpuid
    testl $cpuid_msr, %edx
    jz fatal_cpu
    testl $cpuid_pae, %edx
    jz fatal_cpu
    testl $cpuid_pse, %edx
    jz fatal_cpu
    movl $0x80000000, %eax
    cpuid
    cmpl $0x80000001, %eax
    jb fatal_cpu
    movl $0x80000001, %eax
    cpuid
    testl $cpuid_long_mode, %edx
    jz fatal_cpu
    ret

load_kernel:
    # Refuse to consume the EBDA or video aperture on machines with less
    # conventional memory than this manifest instance requires.
    int $0x12
    movw %cs, %dx
    movw %dx, %ds
    movw %dx, %es
    movzwl %ax, %edx
    shll $10, %edx
    movl %fs:manifest_address + kernel_sector_count_offset, %eax
    shll $9, %eax
    addl $kernel_staging_address, %eax
    jc fatal_kernel
    cmpl %edx, %eax
    ja fatal_kernel

    movl %fs:manifest_address + kernel_lba_offset, %eax
    movl %eax, kernel_lba_work - threshold_entry
    movl %fs:manifest_address + kernel_lba_offset + 4, %eax
    movl %eax, kernel_lba_work + 4 - threshold_entry
    movl %fs:manifest_address + kernel_sector_count_offset, %eax
    movl %eax, kernel_sectors_remaining_work - threshold_entry
    movl %fs:manifest_address + kernel_file_size_offset, %eax
    movl %eax, kernel_bytes_remaining_work - threshold_entry
    movl $kernel_staging_address, kernel_staging_work - threshold_entry
    movl $fnv_offset_basis, kernel_hash_work - threshold_entry

kernel_read_next:
    movl kernel_sectors_remaining_work - threshold_entry, %eax
    testl %eax, %eax
    jz kernel_read_done
    cmpl $disk_chunk_sectors, %eax
    jbe kernel_chunk_selected
    movl $disk_chunk_sectors, %eax
kernel_chunk_selected:
    movw %ax, kernel_chunk_work - threshold_entry
    movw %ax, disk_address_packet + 2 - threshold_entry
    movw $0, disk_address_packet + 4 - threshold_entry
    movl kernel_staging_work - threshold_entry, %edx
    shrl $4, %edx
    movw %dx, disk_address_packet + 6 - threshold_entry
    movl kernel_lba_work - threshold_entry, %edx
    movl %edx, disk_address_packet + 8 - threshold_entry
    movl kernel_lba_work + 4 - threshold_entry, %edx
    movl %edx, disk_address_packet + 12 - threshold_entry
    movw $(disk_address_packet - threshold_entry), %si
    movb $0x42, %ah
    movb boot_drive_work - threshold_entry, %dl
    int $0x13
    jc fatal_kernel
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es

    movzwl kernel_chunk_work - threshold_entry, %eax
    shll $9, %eax
    movl kernel_bytes_remaining_work - threshold_entry, %ecx
    cmpl %eax, %ecx
    jbe kernel_hash_size_selected
    movl %eax, %ecx
kernel_hash_size_selected:
    movw %cx, kernel_hash_size_work - threshold_entry
    movl kernel_hash_work - threshold_entry, %edx
    movl kernel_staging_work - threshold_entry, %eax
    shrl $4, %eax
    movw kernel_hash_size_work - threshold_entry, %cx
    pushw %ds
    movw %ax, %ds
    xorw %si, %si
    call fnv_1a_continue
    popw %ds
    movl %edx, kernel_hash_work - threshold_entry

    movzwl kernel_hash_size_work - threshold_entry, %eax
    subl %eax, kernel_bytes_remaining_work - threshold_entry
    movzwl kernel_chunk_work - threshold_entry, %eax
    subl %eax, kernel_sectors_remaining_work - threshold_entry
    addl %eax, kernel_lba_work - threshold_entry
    adcl $0, kernel_lba_work + 4 - threshold_entry
    shll $9, %eax
    addl %eax, kernel_staging_work - threshold_entry
    jmp kernel_read_next

kernel_read_done:
    cmpl $0, kernel_bytes_remaining_work - threshold_entry
    jne fatal_kernel
    movl kernel_hash_work - threshold_entry, %eax
    cmpl %fs:manifest_address + kernel_hash_offset, %eax
    jne fatal_kernel
    ret

discover_memory:
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    xorl %ebx, %ebx
    # Version 2 publishes the table only after a valid result terminates the
    # firmware enumeration with EBX=0. Errors and capacity exhaustion fail
    # before the completeness flag or descriptor fields become visible.
memory_map_next:
    movzwl memory_entry_count_work - threshold_entry, %eax
    cmpl $maximum_memory_entries, %eax
    jae fatal_memory
    imull $memory_entry_size, %eax
    addw $(memory_map - threshold_entry), %ax
    movw %ax, %di
    movl $1, %es:20(%di)
    movl $0xe820, %eax
    movl $smap_signature, %edx
    movl $memory_entry_size, %ecx
    int $0x15
    jc fatal_memory
    cmpl $smap_signature, %eax
    jne fatal_memory
    cmpw $20, %cx
    jb fatal_memory
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    incw memory_entry_count_work - threshold_entry
    testl %ebx, %ebx
    jne memory_map_next

memory_map_complete:
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    orl $boot_flag_memory_map_complete, boot_omen + 20 - threshold_entry
    movl $memory_map, boot_omen + 72 - threshold_entry
    movl $0, boot_omen + 76 - threshold_entry
    movzwl memory_entry_count_work - threshold_entry, %eax
    movl %eax, boot_omen + 80 - threshold_entry
    movl $memory_entry_size, boot_omen + 84 - threshold_entry
    ret

enable_a20:
    call check_a20
    jnc a20_ready
    movw $0x2401, %ax
    int $0x15
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    call check_a20
    jnc a20_ready
    inb $0x92, %al
    orb $0x02, %al
    andb $0xfe, %al
    outb %al, $0x92
    call check_a20
    jc fatal_a20
a20_ready:
    ret

check_a20:
    pushf
    cli
    pushw %ds
    pushw %es
    pushw %si
    pushw %di
    pushw %bx
    xorw %ax, %ax
    movw %ax, %ds
    movw $0xffff, %ax
    movw %ax, %es
    movw $0x0500, %si
    movw $0x0510, %di
    movb (%si), %bl
    movb %es:(%di), %bh
    movb $0, (%si)
    movb $0xff, %es:(%di)
    movb $0, %cs:a20_result_work - threshold_entry
    cmpb $0xff, (%si)
    je a20_restore
    movb $1, %cs:a20_result_work - threshold_entry
a20_restore:
    movb %bh, %es:(%di)
    movb %bl, (%si)
    popw %bx
    popw %di
    popw %si
    popw %es
    popw %ds
    popf
    cmpb $1, %cs:a20_result_work - threshold_entry
    je a20_enabled
    stc
    ret
a20_enabled:
    clc
    ret

configure_display:
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    # The framebuffer-present bit is the publication gate. Leave it clear
    # until every VBE field satisfies the kernel's boot-omen contract and the
    # firmware has successfully entered the validated linear mode.
    andl $~boot_flag_framebuffer, boot_omen + 20 - threshold_entry

    # VBE 3.0 gives linear modes their own pitch and direct-color fields. Query
    # the controller version so a 3.x BIOS cannot be described with its legacy
    # banked-mode layout; VBE 2.x continues to use the original fields.
    movl $vbe_info_request_signature, vbe_controller_info - threshold_entry
    movw $(vbe_controller_info - threshold_entry), %di
    movw $0x4f00, %ax
    int $0x10
    movw %cs, %dx
    movw %dx, %ds
    movw %dx, %es
    cmpw $vbe_success, %ax
    jne display_fallback
    cmpl $vbe_info_response_signature, vbe_controller_info - threshold_entry
    jne display_fallback
    movzwl vbe_controller_info + 4 - threshold_entry, %eax
    cmpl $vbe_version_2, %eax
    jb display_fallback
    movw %ax, vbe_version_work - threshold_entry

    movw $vbe_mode, %cx
    movw $(vbe_mode_info - threshold_entry), %di
    movw $0x4f01, %ax
    int $0x10
    movw %cs, %dx
    movw %dx, %ds
    movw %dx, %es
    cmpw $vbe_success, %ax
    jne display_fallback
    movw vbe_mode_info - threshold_entry, %ax
    andw $vbe_mode_required_attributes, %ax
    cmpw $vbe_mode_required_attributes, %ax
    jne display_fallback
    cmpb $6, vbe_mode_info + 27 - threshold_entry
    jne display_fallback

    movzbl vbe_mode_info + 25 - threshold_entry, %ebx
    cmpl $24, %ebx
    jb display_fallback
    cmpl $32, %ebx
    ja display_fallback

    cmpw $vbe_version_3, vbe_version_work - threshold_entry
    jb display_legacy_pitch
    movzwl vbe_mode_info + 50 - threshold_entry, %esi
    jmp display_pitch_selected
display_legacy_pitch:
    movzwl vbe_mode_info + 16 - threshold_entry, %esi
display_pitch_selected:
    testl %esi, %esi
    jz display_fallback
    movzwl vbe_mode_info + 18 - threshold_entry, %eax
    testl %eax, %eax
    jz display_fallback
    movzwl vbe_mode_info + 20 - threshold_entry, %ecx
    testl %ecx, %ecx
    jz display_fallback
    movl vbe_mode_info + 40 - threshold_entry, %edi
    testl %edi, %edi
    jz display_fallback

    # Require pitch >= width * ceil(bits_per_pixel / 8), retaining explicit
    # overflow checks even though the current VBE field widths make the first
    # product fit in 32 bits.
    movl %ebx, %edx
    addl $7, %edx
    jc display_fallback
    shrl $3, %edx
    mull %edx
    testl %edx, %edx
    jnz display_fallback
    cmpl %eax, %esi
    jb display_fallback

    # The bootstrap page tables identity-map addresses strictly below 4 GiB.
    # Reject any framebuffer whose pitch * height or base + byte length wraps;
    # treating an exact 4-GiB end as unavailable is deliberately conservative.
    movl %esi, %eax
    mull %ecx
    testl %edx, %edx
    jnz display_fallback
    testl %eax, %eax
    jz display_fallback
    addl %edi, %eax
    jc display_fallback

    # Low memory and the loaded kernel form one protected interval from zero
    # through kernel_load_address + kernel_memory_size. The framebuffer must
    # begin at or beyond that exact manifest-defined end.
    movl %fs:manifest_address + kernel_load_address_offset, %eax
    addl %fs:manifest_address + kernel_memory_size_offset, %eax
    jc display_fallback
    cmpl %eax, %edi
    jb display_fallback

    movl %esi, boot_omen + 32 - threshold_entry
    cmpw $vbe_version_3, vbe_version_work - threshold_entry
    jb display_legacy_channels
    movzbl vbe_mode_info + 55 - threshold_entry, %eax
    movl %eax, boot_omen + 48 - threshold_entry
    movzbl vbe_mode_info + 54 - threshold_entry, %eax
    movl %eax, boot_omen + 52 - threshold_entry
    movzbl vbe_mode_info + 57 - threshold_entry, %eax
    movl %eax, boot_omen + 56 - threshold_entry
    movzbl vbe_mode_info + 56 - threshold_entry, %eax
    movl %eax, boot_omen + 60 - threshold_entry
    movzbl vbe_mode_info + 59 - threshold_entry, %eax
    movl %eax, boot_omen + 64 - threshold_entry
    movzbl vbe_mode_info + 58 - threshold_entry, %eax
    movl %eax, boot_omen + 68 - threshold_entry
    jmp display_channels_selected
display_legacy_channels:
    movzbl vbe_mode_info + 32 - threshold_entry, %eax
    movl %eax, boot_omen + 48 - threshold_entry
    movzbl vbe_mode_info + 31 - threshold_entry, %eax
    movl %eax, boot_omen + 52 - threshold_entry
    movzbl vbe_mode_info + 34 - threshold_entry, %eax
    movl %eax, boot_omen + 56 - threshold_entry
    movzbl vbe_mode_info + 33 - threshold_entry, %eax
    movl %eax, boot_omen + 60 - threshold_entry
    movzbl vbe_mode_info + 36 - threshold_entry, %eax
    movl %eax, boot_omen + 64 - threshold_entry
    movzbl vbe_mode_info + 35 - threshold_entry, %eax
    movl %eax, boot_omen + 68 - threshold_entry
display_channels_selected:
    movl boot_omen + 52 - threshold_entry, %eax
    movl boot_omen + 48 - threshold_entry, %edx
    call validate_display_channel
    jc display_fallback
    movl boot_omen + 60 - threshold_entry, %eax
    movl boot_omen + 56 - threshold_entry, %edx
    call validate_display_channel
    jc display_fallback
    movl boot_omen + 68 - threshold_entry, %eax
    movl boot_omen + 64 - threshold_entry, %edx
    call validate_display_channel
    jc display_fallback

    movl boot_omen + 48 - threshold_entry, %eax
    movl boot_omen + 52 - threshold_entry, %edx
    movl boot_omen + 56 - threshold_entry, %ecx
    movl boot_omen + 60 - threshold_entry, %esi
    call validate_display_channel_pair
    jc display_fallback
    movl boot_omen + 48 - threshold_entry, %eax
    movl boot_omen + 52 - threshold_entry, %edx
    movl boot_omen + 64 - threshold_entry, %ecx
    movl boot_omen + 68 - threshold_entry, %esi
    call validate_display_channel_pair
    jc display_fallback
    movl boot_omen + 56 - threshold_entry, %eax
    movl boot_omen + 60 - threshold_entry, %edx
    movl boot_omen + 64 - threshold_entry, %ecx
    movl boot_omen + 68 - threshold_entry, %esi
    call validate_display_channel_pair
    jc display_fallback

    movw $(vbe_mode | 0x4000), %bx
    movw $0x4f02, %ax
    int $0x10
    movw %cs, %dx
    movw %dx, %ds
    movw %dx, %es
    cmpw $vbe_success, %ax
    jne display_fallback

    movl vbe_mode_info + 40 - threshold_entry, %eax
    movl %eax, boot_omen + 24 - threshold_entry
    movl $0, boot_omen + 28 - threshold_entry
    movzwl vbe_mode_info + 18 - threshold_entry, %eax
    movl %eax, boot_omen + 36 - threshold_entry
    movzwl vbe_mode_info + 20 - threshold_entry, %eax
    movl %eax, boot_omen + 40 - threshold_entry
    movzbl vbe_mode_info + 25 - threshold_entry, %eax
    movl %eax, boot_omen + 44 - threshold_entry
    orl $boot_flag_framebuffer, boot_omen + 20 - threshold_entry
    ret

validate_display_channel:
    testl %eax, %eax
    jz invalid_display_channel
    cmpl $16, %eax
    ja invalid_display_channel
    cmpl %ebx, %edx
    jae invalid_display_channel
    addl %edx, %eax
    jc invalid_display_channel
    cmpl %ebx, %eax
    ja invalid_display_channel
    clc
    ret
invalid_display_channel:
    stc
    ret

validate_display_channel_pair:
    addl %eax, %edx
    addl %ecx, %esi
    cmpl %esi, %eax
    jae valid_display_channel_pair
    cmpl %edx, %ecx
    jae valid_display_channel_pair
    stc
    ret
valid_display_channel_pair:
    clc
    ret

display_fallback:
    movw $0x0003, %ax
    int $0x10
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    ret

fnv_1a:
    movl $fnv_offset_basis, %edx
fnv_1a_continue:
    lodsb
    movzbl %al, %eax
    xorl %eax, %edx
    imull $fnv_prime, %edx, %edx
    loop fnv_1a_continue
    ret

fatal_manifest:
    movw $(manifest_error_message - threshold_entry), %si
    jmp fatal_real
fatal_kernel:
    movw $(kernel_error_message - threshold_entry), %si
    jmp fatal_real
fatal_cpu:
    movw $(cpu_error_message - threshold_entry), %si
    jmp fatal_real
fatal_a20:
    movw $(a20_error_message - threshold_entry), %si
    jmp fatal_real
fatal_memory:
    movw $(memory_error_message - threshold_entry), %si
fatal_real:
    movw %cs, %ax
    movw %ax, %ds
fatal_character:
    lodsb
    testb %al, %al
    jz halt_16
    movb $0x0e, %ah
    movw $0x0007, %bx
    int $0x10
    jmp fatal_character
halt_16:
    cli
    hlt
    jmp halt_16

.code32
protected_mode_entry:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movl $0x7000, %esp
    cld

    # Establish both stage-2 and kernel zero-initialization contracts, copying
    # only the independently linked kernel's file-backed extent above 1 MiB.
    movl $bss_start, %edi
    xorl %eax, %eax
    movl $(bss_end - bss_start), %ecx
    rep stosb
    movl $stack_top, %esp
    movl manifest_address + kernel_staging_address_offset, %esi
    movl manifest_address + kernel_load_address_offset, %edi
    movl manifest_address + kernel_file_size_offset, %ecx
    rep movsb
    movl manifest_address + kernel_load_address_offset, %edi
    addl manifest_address + kernel_file_size_offset, %edi
    movl manifest_address + kernel_memory_size_offset, %ecx
    subl manifest_address + kernel_file_size_offset, %ecx
    xorl %eax, %eax
    rep stosb

    movl $p3_table, %eax
    orl $(page_present | page_writable), %eax
    movl %eax, p4_table

    xorl %ecx, %ecx
map_p3:
    movl %ecx, %eax
    shll $12, %eax
    addl $p2_tables, %eax
    orl $(page_present | page_writable), %eax
    movl %eax, p3_table(,%ecx,8)
    movl $0, p3_table+4(,%ecx,8)
    incl %ecx
    cmpl $4, %ecx
    jne map_p3

    xorl %ecx, %ecx
map_p2:
    movl %ecx, %eax
    shll $21, %eax
    orl $(page_present | page_writable | page_huge), %eax
    movl %eax, p2_tables(,%ecx,8)
    movl $0, p2_tables+4(,%ecx,8)
    incl %ecx
    cmpl $2048, %ecx
    jne map_p2

    movl %cr4, %eax
    orl $cr4_pae, %eax
    movl %eax, %cr4
    movl $p4_table, %eax
    movl %eax, %cr3

    movl $efer_msr, %ecx
    rdmsr
    orl $efer_long_mode_enable, %eax
    wrmsr

    movl %cr0, %eax
    orl $cr0_paging, %eax
    movl %eax, %cr0
    ljmp $0x18, $long_mode_entry

.code64
long_mode_entry:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    leaq stack_top(%rip), %rsp
    xorq %rbp, %rbp
    leaq boot_omen(%rip), %rdi
    movl manifest_address + kernel_entry_address_offset, %eax
    call *%rax

halt_64:
    hlt
    jmp halt_64
.size threshold_entry, . - threshold_entry

.section .rodata.threshold,"a"
.balign 8
gdt:
    .quad 0
    .quad 0x00cf9a000000ffff
    .quad 0x00cf92000000ffff
    .quad 0x00af9a000000ffff
gdt_end:
gdt_pointer:
    .word gdt_end - gdt - 1
    .long gdt

manifest_error_message:
    .asciz "Threshold: invalid manifest\r\n"
kernel_error_message:
    .asciz "Threshold: kernel load failed\r\n"
cpu_error_message:
    .asciz "Threshold: x86-64 CPU required\r\n"
a20_error_message:
    .asciz "Threshold: A20 unavailable\r\n"
memory_error_message:
    .asciz "Threshold: memory discovery failed\r\n"

.section .boot_workspace,"aw",@nobits
.balign 16
boot_workspace_start:
boot_omen:
    .skip boot_omen_size
memory_entry_count_work:
    .skip 2
boot_drive_work:
    .skip 1
a20_result_work:
    .skip 1
kernel_chunk_work:
    .skip 2
kernel_hash_size_work:
    .skip 2
vbe_version_work:
    .skip 2
.balign 4
kernel_sectors_remaining_work:
    .skip 4
kernel_bytes_remaining_work:
    .skip 4
kernel_staging_work:
    .skip 4
kernel_hash_work:
    .skip 4
kernel_lba_work:
    .skip 8
.balign 4
disk_address_packet:
    .skip 16
.balign 16
vbe_controller_info:
    .skip 512
.balign 16
vbe_mode_info:
    .skip 256
.balign 16
memory_map:
    .skip maximum_memory_entries * memory_entry_size
boot_workspace_end:

.section .bss,"aw",@nobits
.balign 4096
bss_start:
p4_table:
    .skip 4096
p3_table:
    .skip 4096
p2_tables:
    .skip 4 * 4096

.balign 16
stack_bottom:
    .skip 65536
stack_top:
bss_end:

.section .note.GNU-stack,"",@progbits
