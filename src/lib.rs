const MEMORY_SIZE: usize = 30000;
pub mod bf;
pub mod bytecode_bf;
pub mod jit_utils;
pub mod llvm_jit;
pub mod optbytecode_jit;
pub mod parser;
pub mod simple_jit;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
