use std::mem::transmute_copy;

use crate::{
    jit_utils::{compute_relative_32bit_offset, CodeEmitter, JitProgram},
    parser, MEMORY_SIZE,
};

pub struct SimpleJit {}

impl SimpleJit {
    pub fn parse_and_run(src: String) {
        let mut memory = vec![0 as u8; MEMORY_SIZE];

        // Registers used in the program:
        //
        // r13: the data pointer -- contains the address of memory.data()
        //
        // rax, rdi, rsi, rdx: used for making system calls, per the ABI.

        let mut emitter = CodeEmitter::new();

        let prog = parser::Parser::parse(src);

        let mut open_bracket_stack: Vec<usize> = vec![];

        // movabs <address of memory.data>, %r13
        emitter.emit_bytes(&[0x49, 0xBD]);
        emitter.emit_uint64(memory.as_mut_ptr() as u64);

        for (pc, instr) in prog.instructions.iter().enumerate() {
            match instr {
                // inc %r13
                '>' => emitter.emit_bytes(&[0x49, 0xFF, 0xC5]),
                // dec %r13
                '<' => emitter.emit_bytes(&[0x49, 0xFF, 0xCD]),
                // Our memory is byte-addressable, so using addb/subb for modifying it.
                // addb $1, 0(%r13)
                '+' => emitter.emit_bytes(&[0x41, 0x80, 0x45, 0x00, 0x01]),
                // subb $1, 0(%r13)
                '-' => emitter.emit_bytes(&[0x41, 0x80, 0x6D, 0x00, 0x01]),
                '.' => {
                    // To emit one byte to stdout, call the write syscall with fd=1 (for
                    // stdout), buf=address of byte, count=1.
                    //
                    // mov $1, %rax
                    // mov $1, %rdi
                    // mov %r13, %rsi
                    // mov $1, %rdx
                    // syscall
                    emitter.emit_bytes(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
                    emitter.emit_bytes(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);
                    emitter.emit_bytes(&[0x4C, 0x89, 0xEE]);
                    emitter.emit_bytes(&[0x48, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]);
                    emitter.emit_bytes(&[0x0F, 0x05]);
                }
                ',' => {
                    // To read one byte from stdin, call the read syscall with fd=0 (for
                    // stdin),
                    // buf=address of byte, count=1.
                    emitter.emit_bytes(&[0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00]);
                    emitter.emit_bytes(&[0x48, 0xC7, 0xC7, 0x00, 0x00, 0x00, 0x00]);
                    emitter.emit_bytes(&[0x4C, 0x89, 0xEE]);
                    emitter.emit_bytes(&[0x48, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]);
                    emitter.emit_bytes(&[0x0F, 0x05]);
                }
                '[' => {
                    // cmpb $0, 0(%r13)
                    emitter.emit_bytes(&[0x41, 0x80, 0x7d, 0x00, 0x00]);

                    // Save the location in the stack, and emit JZ (with 32-bit relative
                    // offset) with 4 placeholder zeroes that will be fixed up later.
                    open_bracket_stack.push(emitter.size());
                    emitter.emit_bytes(&[0x0F, 0x84]);
                    emitter.emit_uint32(0);
                }
                ']' => {
                    if open_bracket_stack.is_empty() {
                        panic!("Unmatching closing ] at pc={}", pc);
                    }

                    let last_open_bracket = open_bracket_stack.pop().unwrap();

                    // cmpb $0, 0(%r13)
                    emitter.emit_bytes(&[0x41, 0x80, 0x7d, 0x00, 0x00]);

                    // matching pair jump to instruction right after the matching pair
                    let jump_back_from = emitter.size() + 6;
                    let jump_back_to = last_open_bracket + 6;
                    let offset = compute_relative_32bit_offset(jump_back_from, jump_back_to);

                    //jnz <open bracket location>
                    emitter.emit_bytes(&[0x0F, 0x85]);
                    emitter.emit_uint32(offset);

                    // fix the destination left empty in the [ instruction before this
                    let jump_forward_from = last_open_bracket + 6;
                    let jump_forward_to = emitter.size();
                    let offset = compute_relative_32bit_offset(jump_forward_from, jump_forward_to);
                    emitter.replace_uint32_at_offset(last_open_bracket + 2, offset);
                }

                _ => panic!("Invalid character"),
            }
        }
        emitter.emit_byte(0xC3);
        unsafe {
            let program = JitProgram::new(emitter.code().clone());
            let jit_fn: unsafe extern "C" fn() -> () = transmute_copy(&program.program_memory());
            jit_fn();
        }
        println!("");
    }
}

#[cfg(test)]
mod tests {

    use super::SimpleJit;

    #[test]
    fn hello_world() {
        let code = include_str!("../programs/hello_world.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn mandelbrot() {
        let code = include_str!("../programs/mandelbrot.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn nested_loop() {
        let code = include_str!("../programs/nested_loop.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn number_crunce() {
        let code = include_str!("../programs/number_crunch.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn serpinski() {
        let code = include_str!("../programs/serpinski.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn trivial_loop() {
        let code = include_str!("../programs/trivial_loop.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn trivial_loop2() {
        let code = include_str!("../programs/trivial_loop2.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn z() {
        let code = include_str!("../programs/z.bf");
        SimpleJit::parse_and_run(code.to_owned());
    }
}
