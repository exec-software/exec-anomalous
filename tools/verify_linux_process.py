#!/usr/bin/env python3
"""Independent real-artifact oracle for inert Linux image/startup reports.

This runs xtask and readelf only, never the supplied foreign executable. Input
artifacts remain external to the repository. The output directory must be new.
Example arguments are documented in ADR 0074. No assertions imply guest launch.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import subprocess

image_budget = 512 * 1024 * 1024
mapped_page_budget = 131072


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_bounded(path, limit):
    with path.open('rb') as source:
        metadata = os.fstat(source.fileno())
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= limit, 'regular source within byte budget')
        data = source.read(limit + 1)
    require(len(data) <= limit, 'source grew beyond byte budget')
    return data


def check_disjoint(ranges):
    for index, (start, stop) in enumerate(ranges):
        require(start <= stop, 'ordered process range')
        if start == stop:
            continue
        require(all(other_start == other_stop or stop <= other_start or other_stop <= start
                    for other_start, other_stop in ranges[:index]), 'disjoint process mappings')


def check_image(report, path):
    data = read_bounded(path, image_budget)
    require(report['source_bytes'] == len(data) and report['sha256'] == sha(data), 'source binding')
    require(bytes.fromhex(report['path_hex']) == os.fsencode(path), 'exact source path')
    fields = struct.unpack_from('<16sHHIQQQIHHHHHH', data)
    ident, kind, machine, version, entry, phoff, _, _, _, phentsize, count, _, _, _ = fields
    require(ident[:7] == b'\x7fELF\x02\x01\x01' and machine == 62 and version == 1, 'ELF profile')
    require(report['elf_type'] == {2: 'ET_EXEC', 3: 'ET_DYN'}[kind], 'ELF type')
    require(phentsize == 56 and 0 < count <= 128 and report['program_header_count'] == count, 'program header count')
    keys = ('kind', 'flags', 'file_offset', 'virtual_address', 'physical_address', 'file_size', 'memory_size', 'alignment')
    headers = [dict(zip(keys, struct.unpack_from('<IIQQQQQQ', data, phoff + n * phentsize))) for n in range(count)]
    require(report['program_headers'] == headers, 'raw metadata preservation')
    bias = report['load_bias']
    interp = [data[h['file_offset']:h['file_offset'] + h['file_size'] - 1] for h in headers if h['kind'] == 3]
    require(report['interpreter_hex'] == (interp[0].hex() if interp else None), 'interpreter byte path')
    loads = [h for h in headers if h['kind'] == 1]
    require(0 < len(loads) <= 32 and len(loads) == len(report['loads']), 'load count')
    pages = 0
    phdr = None
    for header, actual in zip(loads, report['loads']):
        offset, size, memory = (header[k] for k in ('file_offset', 'file_size', 'memory_size'))
        start = header['virtual_address'] + bias
        end = start + memory
        map_start = start // 4096 * 4096
        map_end = (end + 4095) // 4096 * 4096 if memory else map_start
        require(0 <= size <= memory and offset + size <= len(data), 'source load extent')
        pages += (map_end - map_start) // 4096
        require(pages <= mapped_page_budget, 'materialization page budget')
        copy_start = offset // 4096 * 4096 if size else offset
        copy_end = offset + size
        dest_end = start + size if size else map_start
        expected = dict(flags=header['flags'], file=[offset, offset + size], memory=[start, end],
                        initialized=[start, start + size], zero_tail=[start + size, end],
                        mapped=[map_start, map_end], copy_file=[copy_start, copy_end],
                        copy_destination=[map_start, dest_end], zero_fill=[dest_end, map_end])
        require(actual == expected, 'exact load/copy/zero geometry')
        require(dest_end - map_start == copy_end - copy_start, 'equal copy lengths')
        # Independently materialize each proposed copy into zeroed owned storage;
        # the initialized bytes and BSS must match the source's declared spans.
        materialized = bytearray(map_end - map_start)
        materialized[:copy_end - copy_start] = data[copy_start:copy_end]
        require(materialized[start - map_start:start - map_start + size] == data[offset:offset + size], 'initialized bytes')
        require(not any(materialized[start - map_start + size:]), 'zero tail and page slack')
        if offset <= phoff and phoff + count * 56 <= offset + size:
            require(phdr is None, 'unique loaded PHDR')
            phdr = start + phoff - offset
    require(report['phdr_address'] == phdr and report['mapped_pages'] == pages, 'PHDR and mapping budget')
    if report['role'] == 'executable':
        require(report['entry'] == entry + bias, 'relocated entry')
        require(any(h['flags'] & 1 and h['virtual_address'] <= entry < h['virtual_address'] + h['file_size'] for h in loads), 'initialized executable entry')
    else:
        require(report['entry'] is None, 'shared object does not select an entry')
    return dict(path=str(path), sha256=sha(data), bytes=len(data), loads=len(loads), mapped_pages=pages)


def check_stack(report, stack, random, argv):
    base, end = report['stack']['base'], report['stack']['end']
    require(end - base == len(stack) == report['stack']['bytes'], 'stack extent')
    require(sha(stack) == report['stack']['sha256'], 'stack digest')
    rsp = report['initial_rsp']
    require(base <= rsp < end and rsp % 16 == 0, 'aligned initial RSP')

    def word(address):
        require(base <= address <= end - 8, 'stack word pointer')
        return struct.unpack_from('<Q', stack, address - base)[0]

    def string(address):
        require(base <= address < end, 'stack string pointer')
        offset = address - base
        return stack[offset:stack.index(0, offset)]

    require(word(rsp) == 1, 'argc')
    require(string(word(rsp + 8)) == argv and word(rsp + 16) == 0, 'argv and terminator')
    require(word(rsp + 24) == 0, 'empty environment')
    auxiliary = {}
    cursor = rsp + 32
    while True:
        kind, value = word(cursor), word(cursor + 8)
        cursor += 16
        if kind == 0:
            require(value == 0, 'AT_NULL value')
            break
        require(kind not in auxiliary, 'duplicate auxiliary entry')
        auxiliary[kind] = value
    main, loader = report['program'], report['interpreter']
    require(set(auxiliary) == {3, 4, 5, 6, 7, 9, 25, 31}, 'only truthful core facts')
    require({key: auxiliary[key] for key in (3, 4, 5, 6, 7, 9)} == {
        3: main['phdr_address'], 4: 56, 5: main['program_header_count'], 6: 4096,
        7: loader['load_bias'], 9: main['entry']}, 'main/interpreter auxiliary facts')
    require(base <= auxiliary[25] <= end - 16 and stack[auxiliary[25] - base:auxiliary[25] - base + 16] == random, 'AT_RANDOM bytes')
    require(string(auxiliary[31]) == argv, 'AT_EXECFN')
    require(report['initial_rip'] == loader['entry'] and report['initial_rip'] != main['entry'], 'enter interpreter before main')
    require(report['initial_rdx'] == 0 and not report['executed'] and not report['admitted'], 'inert review profile')
    ranges = [tuple(load['mapped']) for image in (main, loader) for load in image['loads']] + [(base, end)]
    check_disjoint(ranges)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ('xtask', 'output', 'program', 'interpreter', 'library', 'electron'):
        parser.add_argument('--' + argument, type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir()
    random = os.urandom(16)
    random_path = args.output / 'random.bin'
    random_path.write_bytes(random)
    process = args.output / 'process'
    subprocess.run([str(args.xtask), 'linux-process-inspect', str(args.program), str(args.interpreter), str(random_path), str(process)], check=True)
    report = json.loads((process / 'report.json').read_text())
    evidence = [check_image(report['program'], args.program), check_image(report['interpreter'], args.interpreter)]
    check_stack(report, read_bounded(process / 'stack.bin', 64 * 1024), random, os.fsencode(args.program))
    for name, path, bias, role in [('libc', args.library, '0x70000000', 'shared-object'), ('electron', args.electron, '0x40000000', 'executable')]:
        raw = subprocess.check_output([str(args.xtask), 'linux-image-inspect', str(path), bias, role])
        (args.output / (name + '.json')).write_bytes(raw)
        evidence.append(check_image(json.loads(raw), path))
    for name, path in [('program', args.program), ('interpreter', args.interpreter), ('libc', args.library), ('electron', args.electron)]:
        # Independent conventional inspection is retained for human review too.
        raw = subprocess.check_output(['readelf', '-W', '-h', '-l', str(path)])
        (args.output / (name + '.readelf.txt')).write_bytes(raw)
    result = dict(scope='host-only structural image and stack qualification; no foreign execution', images=evidence,
                  initial_rip=report['initial_rip'], main_entry=report['program']['entry'], stack_sha256=report['stack']['sha256'])
    (args.output / 'receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
