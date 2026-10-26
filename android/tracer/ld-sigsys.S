/* 插在 ld.so 入口：接住 SIGSYS，把返回值改成 -ENOSYS 再跳过 svc。
 * 同时往 stdout 打几行，用来区分「没装上」和「装上了但没接住」。
 * 全部用 PC 相对地址。stub 必须放在 vaddr 0x1c6bc。
 */
.syntax unified
.arm
.global stub_entry

.equ STUB_VA, 0x1c6bc
.equ OLD_ENTRY, 0x10890

stub_entry:
    b       9f
msg_stub:
    .ascii  "stub\n"
    .balign 4
9:
    push    {r0, r1, r2, r3, r4, r5, r7, lr}
    adr     r0, msg_stub
    mov     r1, #5
    bl      emit

    /* 父进程可能带着被屏蔽的 SIGSYS 进来，先解开。 */
    sub     sp, sp, #8
    mov     r0, #0x40000000
    str     r0, [sp]
    mov     r0, #0
    str     r0, [sp, #4]
    mov     r0, #1
    mov     r1, sp
    mov     r2, #0
    mov     r3, #8
    mov     r7, #175
    svc     #0
    add     sp, sp, #8
    bl      write_hex

    sub     sp, sp, #24
    mov     r4, sp
    adr     r0, sigsys_handler
    str     r0, [r4]
    /* SA_SIGINFO|SA_RESTORER|SA_NODEFER。返回必须用 OABI 的 svc 立即数。 */
    mov     r0, #4
    orr     r0, r0, #0x04000000
    orr     r0, r0, #0x40000000
    str     r0, [r4, #4]
    adr     r0, sigsys_restorer
    str     r0, [r4, #8]
    mov     r0, #0
    str     r0, [r4, #12]
    str     r0, [r4, #16]
    mov     r0, #31
    mov     r1, r4
    mov     r2, #0
    mov     r3, #8
    mov     r7, #174
    svc     #0
    add     sp, sp, #24
    bl      write_hex

    /* send 被改成 write 后，对端先断开时不能被 SIGPIPE 杀掉。 */
    sub     sp, sp, #24
    mov     r4, sp
    mov     r0, #1
    str     r0, [r4]
    mov     r0, #0
    str     r0, [r4, #4]
    str     r0, [r4, #8]
    str     r0, [r4, #12]
    str     r0, [r4, #16]
    mov     r0, #13
    mov     r1, r4
    mov     r2, #0
    mov     r3, #8
    mov     r7, #174
    svc     #0
    add     sp, sp, #24

    pop     {r0, r1, r2, r3, r4, r5, r7, lr}
    ldr     r3, stub_delta
    add     r12, pc, r3
    bx      r12

stub_delta:
    .word   OLD_ENTRY - (STUB_VA + stub_delta - stub_entry)

/* r0 = 指针，r1 = 长度。写到 fd 1。 */
emit:
    push    {r2, r3, r7, lr}
    mov     r2, r1
    mov     r1, r0
    mov     r0, #1
    mov     r7, #4
    svc     #0
    pop     {r2, r3, r7, lr}
    bx      lr

/* r0 = 数值，打成 8 位十六进制加换行。 */
write_hex:
    push    {r0, r1, r2, r3, r4, r5, r6, r7, lr}
    mov     r4, r0
    adr     r5, hexdigits
    sub     sp, sp, #16
    mov     r6, sp
    mov     r3, #8
1:
    mov     r2, r4, lsr #28
    and     r2, r2, #0xf
    ldrb    r2, [r5, r2]
    strb    r2, [r6], #1
    mov     r4, r4, lsl #4
    subs    r3, r3, #1
    bne     1b
    mov     r2, #0x0a
    strb    r2, [r6]
    mov     r0, #1
    mov     r1, sp
    mov     r2, #9
    mov     r7, #4
    svc     #0
    add     sp, sp, #16
    pop     {r0, r1, r2, r3, r4, r5, r6, r7, lr}
    bx      lr

sigsys_handler:
    push    {r0, r1, r2, r3, r4, r5, r6, lr}
    ldr     r0, [r1, #16]
    bl      write_hex
    ldr     r4, [r2, #92]
    ldr     r5, [r2, #96]
    tst     r5, #0x20
    bne     1f
    bic     r4, r4, #3
    ldr     r5, [r4]
    ldr     r3, arm_svc
    cmp     r5, r3
    bne     2f
    add     r4, r4, #4
    b       2f
1:
    bic     r4, r4, #1
    ldrh    r5, [r4]
    movw    r3, #0xdf00
    cmp     r5, r3
    bne     2f
    add     r4, r4, #2
2:
    mvn     r5, #37
    str     r5, [r2, #32]
    str     r4, [r2, #92]
    adr     r0, msg_hit
    mov     r1, #2
    bl      emit
    pop     {r0, r1, r2, r3, r4, r5, r6, lr}
    bx      lr

sigsys_restorer:
    /* 与内核 sigpage 里的 ARM rt_sigreturn 相同：svc 立即数带 OABI 基址。 */
    mov     r7, #0xad
    svc     #0x9000ad

msg_hit:
    .ascii  "H\n"
    .balign 4

arm_svc:
    .word   0xef000000

hexdigits:
    .ascii  "0123456789abcdef"
