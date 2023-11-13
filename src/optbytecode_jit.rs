use std::mem::transmute_copy;

use dynasmrt::{dynasm, DynasmApi, DynasmLabelApi};

use crate::{
    bytecode_bf::{ByteCode, Change},
    parser::Parser,
    MEMORY_SIZE,
};

macro_rules! my_dynasm {
    ($ops:ident $($t:tt)*) => {
        dynasm!($ops
            ; .arch x64
            ; .alias a_current, r13
            $($t)*
        )
    }
}
pub struct BytecodeJit {}

impl BytecodeJit {
    pub fn parse_and_run(src: String) {
        let prog = Parser::parse_to_bytecode(src);
        let mut ops = dynasmrt::x64::Assembler::new().unwrap();
        let mut memory = vec![0 as u8; MEMORY_SIZE];
        let x = memory.as_mut_ptr();

        let mut open_bracket_stack = vec![];
        let start = ops.offset();

        my_dynasm!(ops
        ;mov r13, QWORD x as _
        );

        for (pc, instr) in prog.instructions.iter().enumerate() {
            match instr {
                ByteCode::DataPointerIncr(delta) => {
                    my_dynasm!(ops
                    ; add a_current , *delta as _
                    );
                }
                ByteCode::DataPointerDecr(delta) => {
                    my_dynasm!(ops
                    ; sub a_current, *delta as _
                    );
                }
                ByteCode::DataIncr(delta) => {
                    if *delta > u8::MAX as usize {
                        panic!("Overflow");
                    }
                    my_dynasm!(ops
                    ; add BYTE [a_current + 0], *delta as _
                    );
                }
                ByteCode::DataDecr(delta) => {
                    if *delta > u8::MAX as usize {
                        panic!("Overflow");
                    }
                    my_dynasm!(ops
                    ; sub BYTE [a_current + 0], *delta as _
                    );
                }
                ByteCode::JZ => {
                    my_dynasm!(ops
                    ; cmp BYTE [a_current + 0] , 0
                    );
                    let open_label = ops.new_dynamic_label();
                    let close_label = ops.new_dynamic_label();
                    my_dynasm!(ops
                    ; jz => close_label
                    ; => open_label
                    );
                    open_bracket_stack.push((open_label, close_label));
                }
                ByteCode::JNZ => {
                    if open_bracket_stack.is_empty() {
                        panic!("Not matching ] at pc= {}", pc);
                    }
                    let (open_label, close_label) = open_bracket_stack.pop().unwrap();
                    my_dynasm!(ops
                    ; cmp BYTE [a_current + 0] , 0
                    ; jnz => open_label
                    ; => close_label
                    );
                }
                ByteCode::SETZERO => {
                    my_dynasm!(ops
                    ; mov BYTE [a_current + 0], 0
                    );
                }
                ByteCode::MoveInStepUntilZero(chng) => {
                    let start_loop = ops.new_dynamic_label();
                    let end_loop = ops.new_dynamic_label();
                    my_dynasm!(ops
                    ; => start_loop
                    ;  cmp BYTE [a_current + 0] ,0
                    ; jz =>end_loop
                    );

                    match chng {
                        Change::Incr(x) => {
                            my_dynasm!(ops
                                    ; add BYTE [a_current + 0] , *x as _
                                    ; jmp =>start_loop // (should have this??)
                            );
                        }
                        Change::Decr(x) => {
                            my_dynasm!(ops
                                    ; sub BYTE [a_current + 0] , *x as _
                                    ; jmp =>start_loop // (should jump back too?)
                            );
                        }
                    }

                    my_dynasm!(ops
                        ; => end_loop);
                }
                ByteCode::Write => {
                    // mov $1, %rax
                    // mov $1, %rdi
                    // mov %r13, %rsi
                    // mov $1, %rdx
                    // syscall
                    dynasm!(ops
                    ; mov rax , 1
                    ; mov rdi , 1
                    ; mov rsi, r13
                    ; mov rdx, 1
                    ; syscall
                    );
                }
                ByteCode::Read => {
                    // mov $0, %rax
                    // mov $0, %rdi
                    // mov %r13, %rsi
                    // mov $1, %rdx
                    // syscall
                    dynasm!(ops
                    ; mov rax , 0
                    ; mov rdi , 0
                    ; mov rsi, r13
                    ; mov rdx, 1
                    ; syscall
                    );
                }
                ByteCode::Nop => {}
                _ => unimplemented!(),
            }
        }
        my_dynasm!(ops
        ;ret
        );

        let cmt = ops.commit();
        if cmt.is_err() {
            println!("{:?}", cmt.err());
            return;
        }

        let code = ops.finalize();
        match code {
            Ok(prog) => unsafe {
                let jit_fn: unsafe extern "C" fn() -> () = transmute_copy(&prog.ptr(start));
                jit_fn();
            },
            Err(e) => println!("{:?}", e),
        }
        // let code = ops.finalize().unwrap();
        // unsafe {}
        println!("");
    }
}

#[cfg(test)]
mod tests {

    use super::BytecodeJit;

    #[test]
    fn hello_world() {
        let code = include_str!("../programs/hello_world.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn mandelbrot() {
        let code = include_str!("../programs/mandelbrot.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn nested_loop() {
        let code = include_str!("../programs/nested_loop.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn number_crunce() {
        let code = include_str!("../programs/number_crunch.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn serpinski() {
        let code = include_str!("../programs/serpinski.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn trivial_loop() {
        let code = include_str!("../programs/trivial_loop.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn trivial_loop2() {
        let code = include_str!("../programs/trivial_loop2.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn z() {
        let code = include_str!("../programs/z.bf");
        BytecodeJit::parse_and_run(code.to_owned());
    }
}
