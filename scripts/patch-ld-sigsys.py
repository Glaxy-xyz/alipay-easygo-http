#!/usr/bin/env python3
"""在 armhf ld.so 入口装上 SIGSYS -> ENOSYS 处理，供 Android 应用 seccomp 使用。"""
import struct
import sys
from pathlib import Path

STUB_VA = 0x1C6BC
OLD_ENTRY = 0x10890
OLD_FILESZ = 0x1C6BC
PAGE_END = 0x1D000
FILTER_VA = 0x1C900
GATEWAY_VA = 0x141A0
ROOT = Path(__file__).resolve().parents[1]
STUB_PATH = ROOT / "android" / "tracer" / "ld-sigsys.bin"
FILTER_PATH = ROOT / "android" / "tracer" / "syscall-filter.bin"


def patch(data: bytearray, stub: bytes) -> bytearray:
    if data[:4] != b"\x7fELF" or data[4] != 1:
        raise SystemExit("不是 32 位 ELF")
    e_entry = struct.unpack_from("<I", data, 24)[0]
    if e_entry == STUB_VA:
        data[STUB_VA:STUB_VA + len(stub)] = stub
        return data
    if e_entry != OLD_ENTRY:
        raise SystemExit(f"入口 {e_entry:#x} 与预期 {OLD_ENTRY:#x} 不符，拒绝修补")
    if len(stub) == 0 or STUB_VA + len(stub) > PAGE_END:
        raise SystemExit("stub 超出 ld.so 可执行页")
    e_phoff = struct.unpack_from("<I", data, 28)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", data, 42)
    load_index = None
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, p_offset, p_vaddr, _, p_filesz, p_memsz, _, _ = struct.unpack_from("<IIIIIIII", data, off)
        if p_type == 1 and p_offset == 0 and p_vaddr == 0:
            if p_filesz != OLD_FILESZ or p_memsz != OLD_FILESZ:
                raise SystemExit(f"LOAD 大小 {p_filesz:#x} 与预期不符")
            load_index = off
            break
    if load_index is None:
        raise SystemExit("找不到第一个 LOAD 段")
    data[STUB_VA:STUB_VA + len(stub)] = stub
    new_sz = STUB_VA + len(stub)
    struct.pack_into("<I", data, 24, STUB_VA)
    struct.pack_into("<I", data, load_index + 16, new_sz)
    struct.pack_into("<I", data, load_index + 20, new_sz)
    return data


def encode_thumb_b_w(src: int, dest: int) -> bytes:
    off = dest - (src + 4)
    if off % 2:
        raise SystemExit(f"thumb 分支未对齐: {src:#x} -> {dest:#x}")
    v = off >> 1
    if off < 0:
        v &= (1 << 23) - 1
    imm11 = v & 0x7FF
    imm10 = (v >> 11) & 0x3FF
    i2 = (v >> 21) & 1
    i1 = (v >> 22) & 1
    s = 1 if off < 0 else 0
    j1 = 1 ^ (i1 ^ s)
    j2 = 1 ^ (i2 ^ s)
    hw1 = 0xF000 | (s << 10) | imm10
    hw2 = (0b10 << 14) | (j1 << 13) | (1 << 12) | (j2 << 11) | imm11
    return struct.pack("<HH", hw1, hw2)


def install_gateway_filter(data: bytearray, gateway_va: int, filter_va: int, filt: bytes) -> None:
    """把 glibc 的 syscall 网关改成先查禁止表，命中则直接返回 -ENOSYS。"""
    if data[gateway_va:gateway_va + 8] != bytes.fromhex("80b5674600df80bd"):
        raise SystemExit(f"网关 {gateway_va:#x} 字节不符，拒绝修补")
    if filter_va + len(filt) > len(data):
        raise SystemExit("过滤代码超出文件")
    data[filter_va:filter_va + len(filt)] = filt
    branch = encode_thumb_b_w(gateway_va, filter_va)
    data[gateway_va:gateway_va + 4] = branch
    data[gateway_va + 4:gateway_va + 8] = bytes.fromhex("00bf00bf")


def extend_load(data: bytearray, new_sz: int) -> None:
    e_phoff = struct.unpack_from("<I", data, 28)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", data, 42)
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, p_offset, p_vaddr = struct.unpack_from("<III", data, off)
        if p_type == 1 and p_offset == 0 and p_vaddr == 0:
            filesz, memsz = struct.unpack_from("<II", data, off + 16)
            if new_sz < filesz:
                return
            struct.pack_into("<I", data, off + 16, new_sz)
            struct.pack_into("<I", data, off + 20, max(memsz, new_sz))
            return
    raise SystemExit("找不到第一个 LOAD 段")


def retarget_word(data: bytearray, off: int, old: bytes, new: bytes, name: str) -> None:
    got = bytes(data[off:off + 4])
    if got == new:
        return
    if got != old:
        raise SystemExit(f"{name} {off:#x} 是 {got.hex()}，不是 {old.hex()}")
    data[off:off + 4] = new


def retarget_socket_io(data: bytearray) -> None:
    """这台 POS 的 seccomp 会杀掉 recv(291) 和 send(289)。
    参数顺序与 read(3)、write(4) 的前三个相同，套接字已是非阻塞的。"""
    read = bytes.fromhex("40f2030c")
    write = bytes.fromhex("40f2040c")
    for off in (0xB5C84, 0xB5CB0):
        retarget_word(data, off, bytes.fromhex("40f2231c"), read, "recv")
    for off in (0xB5EBC, 0xB5EE8):
        retarget_word(data, off, bytes.fromhex("40f2211c"), write, "send")


def patch_libc(path: Path) -> None:
    filt = FILTER_PATH.read_bytes()
    data = bytearray(path.read_bytes())
    retarget_socket_io(data)
    gateway = 0x1E650
    filter_va = 0x1096C4
    if data[filter_va:filter_va + len(filt)] != b"\x00" * len(filt):
        # 允许重复打补丁：过滤代码已经在那里。
        if data[filter_va:filter_va + 2] != filt[:2]:
            raise SystemExit("libc 代码空洞不是空的，拒绝修补")
    install_gateway_filter(data, gateway, filter_va, filt)
    extend_load(data, filter_va + len(filt))
    path.write_bytes(data)
    print(f"patched {path} gateway={gateway:#x} filter={filter_va:#x}")


def main() -> None:
    known = encode_thumb_b_w(GATEWAY_VA, FILTER_VA)
    if known != bytes.fromhex("08f0aebb"):
        raise SystemExit(f"分支编码自检失败: {known.hex()}")
    if len(sys.argv) == 3 and sys.argv[1] == "--libc":
        patch_libc(Path(sys.argv[2]))
        return
    if len(sys.argv) != 2:
        raise SystemExit(f"用法: {sys.argv[0]} ld.so | --libc libc.so.6")
    path = Path(sys.argv[1])
    stub = STUB_PATH.read_bytes()
    filt = FILTER_PATH.read_bytes()
    patched = patch(bytearray(path.read_bytes()), stub)
    install_gateway_filter(patched, GATEWAY_VA, FILTER_VA, filt)
    extend_load(patched, FILTER_VA + len(filt))
    path.write_bytes(patched)
    print(f"patched {path} entry={STUB_VA:#x} stub={len(stub)} filter={FILTER_VA:#x}")


if __name__ == "__main__":
    main()
