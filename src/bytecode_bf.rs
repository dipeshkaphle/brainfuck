use std::{io::stdin, mem::replace};

use crate::MEMORY_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Change {
    Incr(usize),
    Decr(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ByteCode {
    Nop,
    DataPointerIncr(usize),      // >
    DataPointerDecr(usize),      //<
    DataIncr(usize),             // +
    DataDecr(usize),             // -
    Write,                       // Write Stdout
    Read,                        // Read Stdin
    JZ,                          //  Jump Zero
    JNZ,                         // Jump not Zero
    SETZERO,                     // Set Current Cell to Zero , [+] or [-]
    MoveInStepUntilZero(Change), // Moves the data_counter in certain increments until it encounters a cell which is zero [>>>>] or [<<<<] instructions
}

pub struct ByteCodeProgram {
    pub instructions: Vec<ByteCode>,
}

impl ByteCodeProgram {
    fn compute_jumptable(&self) -> Vec<usize> {
        let mut pc = 0;
        let prog_size = self.instructions.len();
        let mut jumptable = vec![0; prog_size];
        while pc < prog_size {
            let instr = self.instructions[pc];
            if instr == ByteCode::JZ {
                let mut nesting = 1;
                let mut seek = pc;
                while nesting > 0 && (seek + 1) < prog_size {
                    seek += 1;
                    if self.instructions[seek] == ByteCode::JNZ {
                        nesting -= 1;
                    } else if self.instructions[seek] == ByteCode::JZ {
                        nesting += 1;
                    }
                }
                if nesting == 0 {
                    jumptable[pc] = seek;
                    jumptable[seek] = pc;
                } else {
                    panic!("unmatched '[' at pc= {}", pc);
                }
            }
            pc += 1;
        }
        jumptable
    }

    fn is_set_zero(instructions: &[ByteCode]) -> bool {
        if instructions.len() >= 3 {
            match (instructions[0], instructions[1], instructions[2]) {
                (ByteCode::JZ, ByteCode::DataIncr(_) | ByteCode::DataDecr(_), ByteCode::JNZ) => {
                    return true;
                }
                _ => {
                    return false;
                }
            }
        }
        return false;
    }

    fn is_move_until_zero(instructions: &[ByteCode]) -> Option<Change> {
        if instructions.len() >= 3 {
            match (instructions[0], instructions[1], instructions[2]) {
                (ByteCode::JZ, ByteCode::DataPointerIncr(x), ByteCode::JNZ) => {
                    return Some(Change::Incr(x));
                }
                (ByteCode::JZ, ByteCode::DataPointerDecr(x), ByteCode::JNZ) => {
                    return Some(Change::Decr(x));
                }
                _ => {
                    return None;
                }
            }
        }
        return None;
    }

    pub fn opt_pass_1(&mut self) {
        //
        let mut index = 0;
        let prog_size = self.instructions.len();
        let mut new_instructions = vec![];
        while index < prog_size {
            new_instructions.push(match self.instructions[index] {
                ByteCode::JZ => {
                    if Self::is_set_zero(&self.instructions[index..]) {
                        index += 2;
                        ByteCode::SETZERO
                    } else {
                        let change = Self::is_move_until_zero(&self.instructions[index..]);
                        if let Some(chng) = change {
                            index += 2;
                            ByteCode::MoveInStepUntilZero(chng)
                        } else {
                            ByteCode::JZ
                        }
                    }
                }
                instr => instr,
            });
            index += 1;
        }
        let _ = replace(&mut self.instructions, new_instructions);
    }
    pub fn eval(&self) {
        let mut memory = vec![0 as u8; MEMORY_SIZE];
        let mut data_counter = 0;
        let mut pc = 0;
        let jumptable = self.compute_jumptable();
        while pc < self.instructions.len() {
            let instr = self.instructions[pc];
            match instr {
                ByteCode::DataPointerIncr(x) => {
                    data_counter += x;
                }
                ByteCode::DataPointerDecr(x) => {
                    data_counter -= x.min(data_counter);
                }
                ByteCode::DataIncr(x) => {
                    memory[data_counter] = (memory[data_counter] as usize + x) as u8;
                }
                ByteCode::DataDecr(x) => {
                    memory[data_counter] = (memory[data_counter] as usize
                        - x.min(memory[data_counter] as usize))
                        as u8;
                }
                ByteCode::Write => {
                    print!("{}", memory[data_counter] as char);
                }
                ByteCode::Read => {
                    let mut inp = String::new();
                    stdin()
                        .read_line(&mut inp)
                        .expect("Failed to read from stdin");
                    memory[data_counter] = inp.as_bytes()[0];
                }
                ByteCode::JZ => {
                    if memory[data_counter] == 0 {
                        pc = jumptable[pc];
                    }
                }
                ByteCode::JNZ => {
                    if memory[data_counter] != 0 {
                        pc = jumptable[pc];
                    }
                }
                ByteCode::SETZERO => {
                    memory[data_counter] = 0;
                }
                ByteCode::MoveInStepUntilZero(chng) => {
                    let cur_dc = &mut data_counter;
                    while memory[*cur_dc] != 0 {
                        *cur_dc = match chng {
                            Change::Incr(x) => *cur_dc + x,
                            Change::Decr(x) => *cur_dc - x,
                        }
                    }
                }
                _ => unreachable!(),
            }
            pc += 1;
        }
        println!("");
        //
    }
}

#[cfg(test)]
mod tests {

    use crate::parser::Parser;

    #[test]
    fn hello_world() {
        let code = include_str!("../programs/hello_world.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }

    #[test]
    fn mandelbrot() {
        let code = include_str!("../programs/mandelbrot.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }

    #[test]
    fn nested_loop() {
        let code = include_str!("../programs/nested_loop.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }

    #[test]
    fn number_crunce() {
        let code = include_str!("../programs/number_crunch.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }

    #[test]
    fn serpinski() {
        let code = include_str!("../programs/serpinski.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }

    #[test]
    fn trivial_loop() {
        let code = include_str!("../programs/trivial_loop.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }
    #[test]
    fn trivial_loop2() {
        let code = include_str!("../programs/trivial_loop2.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }
    #[test]
    fn z() {
        let code = include_str!("../programs/z.bf");
        let mut prog = Parser::parse_to_bytecode(code.to_owned());
        prog.opt_pass_1();
        prog.eval();
    }
}
