use crate::{
    bf::Program,
    bytecode_bf::{ByteCode, ByteCodeProgram},
};

pub struct Parser {}
impl Parser {
    pub fn parse(src_code: String) -> Program {
        Program {
            instructions: src_code
                .as_bytes()
                .into_iter()
                .filter(|x| ['>', '<', '+', '-', '.', ',', '[', ']'].contains(&(**x as char)))
                .map(|x| (*x as char))
                .collect::<Vec<char>>(),
        }
    }

    fn count_contigous(vec: &[char], c: char) -> usize {
        match vec.iter().enumerate().find(|(_, x)| **x != c) {
            Some((i, _)) => i,
            None => vec.len(),
        }
    }

    pub fn parse_to_bytecode(src_code: String) -> ByteCodeProgram {
        let program = Self::parse(src_code);
        let mut bytecode_instrs = vec![];
        let mut index = 0;
        let prog_size = program.instructions.len();
        while index < prog_size {
            bytecode_instrs.push(match program.instructions[index] {
                '[' => ByteCode::JZ,
                ']' => ByteCode::JNZ,
                '.' => ByteCode::Write,
                ',' => ByteCode::Read,
                '+' => {
                    let occ = Self::count_contigous(&program.instructions[index..], '+');
                    index += occ - 1;
                    ByteCode::DataIncr(occ)
                }
                '-' => {
                    let occ = Self::count_contigous(&program.instructions[index..], '-');
                    index += occ - 1;
                    ByteCode::DataDecr(occ)
                }
                '>' => {
                    let occ = Self::count_contigous(&program.instructions[index..], '>');
                    index += occ - 1;
                    ByteCode::DataPointerIncr(occ)
                }
                '<' => {
                    let occ = Self::count_contigous(&program.instructions[index..], '<');
                    index += occ - 1;
                    ByteCode::DataPointerDecr(occ)
                }
                _ => ByteCode::Nop,
            });
            index += 1;
        }
        ByteCodeProgram {
            instructions: bytecode_instrs,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{bytecode_bf::ByteCode, parser::Parser};

    #[test]
    fn bytecode_parser() {
        let code = ">>++<<.,[]--";
        use ByteCode::*;
        assert_eq!(
            Parser::parse_to_bytecode(code.to_owned()).instructions,
            vec![
                DataPointerIncr(2),
                DataIncr(2),
                DataPointerDecr(2),
                Write,
                Read,
                JZ,
                JNZ,
                DataDecr(2)
            ]
        );
    }
}
