use std::io::stdin;

use crate::MEMORY_SIZE;

pub struct Program {
    pub instructions: Vec<char>,
}
impl Program {
    pub fn compute_jumptable(&self) -> Vec<usize> {
        let mut pc = 0;
        let prog_size = self.instructions.len();
        let mut jumptable = vec![0; prog_size];
        while pc < prog_size {
            let instr = self.instructions[pc];
            if instr == '[' {
                let mut nesting = 1;
                let mut seek = pc;
                while nesting > 0 && (seek + 1) < prog_size {
                    seek += 1;
                    if self.instructions[seek] == ']' {
                        nesting -= 1;
                    } else if self.instructions[seek] == '[' {
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

    /// https://eli.thegreenplace.net/2017/adventures-in-jit-compilation-part-1-an-interpreter/
    pub fn eval(&self) {
        let mut memory = vec![0 as u8; MEMORY_SIZE];
        let mut data_counter = 0;
        let mut pc = 0;
        let jumptable = self.compute_jumptable();
        while pc < self.instructions.len() {
            let instr = self.instructions[pc];
            match instr {
                '>' => {
                    data_counter += 1;
                }
                '<' => {
                    data_counter -= 1.min(data_counter);
                }
                '+' => {
                    memory[data_counter] += 1;
                }
                '-' => {
                    memory[data_counter] -= 1;
                }
                '.' => {
                    print!("{}", memory[data_counter] as char);
                }
                ',' => {
                    let mut inp = String::new();
                    stdin()
                        .read_line(&mut inp)
                        .expect("Failed to read from stdin");
                    memory[data_counter] = inp.as_bytes()[0];
                }
                '[' => {
                    if memory[data_counter] == 0 {
                        pc = jumptable[pc];
                    }
                }
                ']' => {
                    if memory[data_counter] != 0 {
                        pc = jumptable[pc];
                    }
                }
                _ => unreachable!(),
            }
            pc += 1;
        }
        println!("");
    }
}

#[cfg(test)]
mod tests {

    use crate::parser::Parser;

    #[test]
    fn hello_world() {
        let code = include_str!("../programs/hello_world.bf");
        Parser::parse(code.to_owned()).eval();
    }

    #[test]
    fn mandelbrot() {
        let code = include_str!("../programs/mandelbrot.bf");
        Parser::parse(code.to_owned()).eval();
    }

    #[test]
    fn nested_loop() {
        let code = include_str!("../programs/nested_loop.bf");
        Parser::parse(code.to_owned()).eval();
    }

    #[test]
    fn number_crunce() {
        let code = include_str!("../programs/number_crunch.bf");
        Parser::parse(code.to_owned()).eval();
    }

    #[test]
    fn serpinski() {
        let code = include_str!("../programs/serpinski.bf");
        Parser::parse(code.to_owned()).eval();
    }

    #[test]
    fn trivial_loop() {
        let code = include_str!("../programs/trivial_loop.bf");
        Parser::parse(code.to_owned()).eval();
    }
    #[test]
    fn trivial_loop2() {
        let code = include_str!("../programs/trivial_loop2.bf");
        Parser::parse(code.to_owned()).eval();
    }
    #[test]
    fn z() {
        let code = include_str!("../programs/z.bf");
        Parser::parse(code.to_owned()).eval();
    }
}
