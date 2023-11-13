use std::{ffi::c_void, fmt::format, ptr::null_mut};

use nix::{
    libc::{memcpy, munmap},
    sys::mman::{mprotect, MapFlags, ProtFlags},
};

fn alloc_rw_mem(sz: usize) -> *mut c_void {
    unsafe {
        let addr: *mut c_void = null_mut();
        let mem = nix::sys::mman::mmap(
            addr,
            sz,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE | MapFlags::MAP_ANON,
            -1,
            0,
        );
        match mem {
            Ok(x) => x,
            Err(e) => {
                panic!("{}", e.to_string());
            }
        }
    }
}
fn make_mem_executable(mem: &mut *mut c_void, sz: usize) {
    unsafe {
        let x = mprotect(*mem, sz, ProtFlags::PROT_READ | ProtFlags::PROT_EXEC);
        if x.is_err() {
            panic!("{}", x.err().unwrap().to_string());
        }
    }
}

pub struct JitProgram {
    pub prog_size: usize,
    pub _program_memory: *mut c_void,
}
impl Drop for JitProgram {
    fn drop(&mut self) {
        unsafe {
            if munmap(self._program_memory, self.prog_size) < 0 {
                panic!("Unable to unmap memory");
            }
        }
    }
}

impl JitProgram {
    pub fn new(code: Vec<u8>) -> Self {
        let mut memory = alloc_rw_mem(code.len());
        unsafe {
            memcpy(memory, code.as_ptr() as *const c_void, code.len());
        }
        make_mem_executable(&mut memory, code.len());

        Self {
            prog_size: code.len(),
            _program_memory: memory,
        }
    }
    pub fn program_size(&self) -> usize {
        self.prog_size
    }
    pub fn program_memory(&self) -> *mut c_void {
        return self._program_memory;
    }
}

pub struct CodeEmitter {
    _code: Vec<u8>,
}
impl CodeEmitter {
    pub fn new() -> Self {
        CodeEmitter { _code: vec![] }
    }
    pub fn emit_byte(&mut self, v: u8) {
        self._code.push(v);
    }
    pub fn emit_bytes(&mut self, bytes: &[u8]) {
        bytes.iter().for_each(|x| self._code.push(*x));
    }
    pub fn emit_uint32(&mut self, v: u32) {
        self.emit_byte((v & 0xff) as u8);
        self.emit_byte(((v >> 8) & 0xff) as u8);
        self.emit_byte(((v >> 16) & 0xff) as u8);
        self.emit_byte(((v >> 24) & 0xff) as u8);
    }
    pub fn emit_uint64(&mut self, v: u64) {
        self.emit_uint32((v & 0xffffffff) as u32);
        self.emit_uint32(((v >> 32) & 0xffffffff) as u32);
    }
    pub fn replace_byte_at_offset(&mut self, offset: usize, v: u8) {
        if offset >= self._code.len() {
            panic!("offset invalid");
        }
        self._code[offset] = v;
    }
    pub fn replace_uint32_at_offset(&mut self, offset: usize, v: u32) {
        self.replace_byte_at_offset(offset, (v & 0xff) as u8);
        self.replace_byte_at_offset(offset + 1, ((v >> 8) & 0xff) as u8);
        self.replace_byte_at_offset(offset + 2, ((v >> 16) & 0xff) as u8);
        self.replace_byte_at_offset(offset + 3, ((v >> 24) & 0xff) as u8);
    }
    pub fn size(&self) -> usize {
        return self._code.len();
    }
    pub fn code(&self) -> &Vec<u8> {
        return &self._code;
    }
}

pub fn compute_relative_32bit_offset(jump_from: usize, jump_to: usize) -> u32 {
    if jump_to >= jump_from {
        let diff = jump_to - jump_from;
        if diff > (u32::MAX as usize) {
            panic!("Not possible to convert to 32 bits");
        }
        return diff as u32;
    } else {
        let diff = jump_from - jump_to;
        if diff > (u32::MAX as usize + 1) {
            panic!("Not possible to convert to 32 bits");
        }
        return !(diff as u32) + 1;
    }
}

#[cfg(test)]
mod tests {
    use std::mem::transmute_copy;

    use crate::jit_utils::compute_relative_32bit_offset;

    use super::{CodeEmitter, JitProgram};

    #[test]
    fn compute_relative_offset() {
        let f = &compute_relative_32bit_offset;
        assert_eq!(f(20, 30), 10);
        assert_eq!(f(40, 30), 0xfffffff6);
        assert_eq!(f(20, 20), 0);
        assert_eq!(f(100, 101), 1);
        assert_eq!(f(101, 100), 0xffffffff);
        assert_eq!(f(1000, 1256), 256);
        assert_eq!(f(1256, 1000), 0xffffff00);
        // assert_eq!(f(0x))
        assert_eq!(f(0xFFFFFFFF, 0x10000000C), 13);
        assert_eq!(f(0x10000000C, 0xFFFFFFFF), 0xFFFFFFF3);

        assert!(f(0x7FFFFFFF, 0x80000001) == 2);
        assert!(f(0x80000001, 0x7FFFFFFF) == 0xFFFFFFFE);

        assert!(f(0x2020202000000000, 0x202020207FFFFFFF) == 0x7FFFFFFF);
        assert!(f(0x2020202080000000, 0x2020202000000000) == 0x80000000);
    }

    #[test]
    fn test_emitter() {
        let mut em1 = CodeEmitter::new();
        em1.emit_byte(0x20);
        assert!(em1.size() == 1);
        assert!(em1.code()[0] == 0x20);

        em1.emit_byte(0x30);
        assert!(em1.size() == 2);
        assert!(em1.code()[0] == 0x20);
        assert!(em1.code()[1] == 0x30);

        em1.emit_uint32(0xA0B0C0D0);
        assert!(em1.size() == 6);
        assert!(em1.code()[2] == 0xD0);
        assert!(em1.code()[3] == 0xC0);
        assert!(em1.code()[4] == 0xB0);
        assert!(em1.code()[5] == 0xA0);

        em1.emit_uint64(0x1112131415161718);
        assert!(em1.size() == 14);
        assert!(em1.code()[6] == 0x18);
        assert!(em1.code()[13] == 0x11);

        let mut em2 = CodeEmitter::new();
        em2.emit_bytes(&[0x01, 0x03, 0x05]);
        assert!(em2.size() == 3);
        assert!(em2.code()[0] == 0x01);
        assert!(em2.code()[1] == 0x03);
        assert!(em2.code()[2] == 0x05);

        // Now test the replacement methods
        let mut em3 = CodeEmitter::new();
        em3.emit_bytes(&[
            0x01, 0x02, 0x03, 0x04, 0x11, 0x12, 0x13, 0x14, 0x21, 0x22, 0x23, 0x24, 0x31, 0x32,
            0x33, 0x34,
        ]);
        assert!(em3.size() == 16);
        assert!(em3.code()[10] == 0x23);
        assert!(em3.code()[15] == 0x34);

        em3.replace_byte_at_offset(10, 0x55);
        assert!(em3.size() == 16);
        assert!(em3.code()[10] == 0x55);

        em3.replace_uint32_at_offset(12, 0xF2E2D2C2);
        assert!(em3.size() == 16);
        assert!(em3.code()[15] == 0xF2);
    }

    #[test]
    fn test_jit() {
        let code: Vec<u8> = vec![
            0x48, 0x89, 0xf8, // mov %rdi, %rax
            0x48, 0x83, 0xc0, 0x04, // add $4, %rax
            0xc3, // ret
        ];
        let program = JitProgram::new(code);

        unsafe {
            let jit_fn: unsafe extern "C" fn(u64) -> u64 =
                transmute_copy(&program.program_memory());
            assert_eq!(jit_fn(21), 25);
        }
    }
}
// void test_jit_program() {
// // Tests that JitProgram works for emitting and running a simple function.
// std::vector<uint8_t> code{
// 0x48, 0x89, 0xf8,       // mov %rdi, %rax
// 0x48, 0x83, 0xc0, 0x04, // add $4, %rax
// 0xc3                    // ret
// };

// JitProgram jit_program(code);

// using JittedFunc = long (*)(long);
// JittedFunc func = (JittedFunc)jit_program.program_memory();
// assert(func(21) == 25);
// }
