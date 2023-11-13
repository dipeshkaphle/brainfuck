use crate::bytecode_bf::{ByteCode, Change};
use crate::{parser::Parser, MEMORY_SIZE};
use inkwell::basic_block::BasicBlock;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::targets::InitializationConfig;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::PointerValue;
use std::alloc::Layout;
use std::io::Read;

extern "C" fn putchar(c: u32) -> u32 {
    unsafe {
        print!("{}", char::from_u32_unchecked(c));
    }
    return c;
}
extern "C" fn getchar() -> u32 {
    let mut buf = vec![0];
    std::io::stdin().read_exact(&mut buf).unwrap();
    return buf[0] as u32;
}

const JIT_FUNC_NAME: &'static str = "__llvm_jit";
const PUTCHAR: &'static str = "putchar";
const GETCHAR: &'static str = "getchar";
#[macro_export]
macro_rules! load {
    ($builder: expr, $data: expr, $type: expr) => {
        $builder.build_load($type, $data, "load")
    };
}

macro_rules! gep {
    ($builder: expr, $memory: expr, $offset: expr, $type: expr) => {
        unsafe { $builder.build_in_bounds_gep($type, $memory, &[$offset], "offset") }
    };
}

pub struct LlvmJit {
    context: inkwell::context::Context,
}

impl LlvmJit {
    fn jit_instr<'a, 'b>(
        &'b self,
        instruction: ByteCode,
        module: &'a inkwell::module::Module<'b>,
        builder: &'a inkwell::builder::Builder<'b>,
        dataptr_addr: PointerValue,
        memory: PointerValue,
        matching_blocks: &'a mut Vec<(BasicBlock<'b>, BasicBlock<'b>)>,
    ) {
        let context = &self.context;
        match instruction {
            ByteCode::Nop => {}
            ByteCode::DataPointerIncr(offset) | ByteCode::DataPointerDecr(offset) => {
                // *dataptr_addr ( +/- )= offset;
                let dataptr = load!(builder, dataptr_addr, context.i64_type());
                let new_dataptr = match instruction {
                    ByteCode::DataPointerIncr(_) => builder.build_int_add(
                        dataptr.into_int_value(),
                        context.i64_type().const_int(offset as u64, false),
                        "incr_dataptr",
                    ),
                    _ => builder.build_int_sub(
                        dataptr.into_int_value(),
                        context.i64_type().const_int(offset as u64, false),
                        "decr_dataptr",
                    ),
                };

                builder.build_store(dataptr_addr, new_dataptr);
            }
            ByteCode::DataIncr(delta) | ByteCode::DataDecr(delta) => {
                // memory[*dataptr_addr] ( +/- )= delta;
                if delta > u8::MAX as usize {
                    panic!("Overflow");
                }
                let dataptr = load!(builder, dataptr_addr, context.i64_type());

                // gep => get element pointer
                let elem_addr = gep!(
                    builder,
                    memory,
                    dataptr.into_int_value(),
                    context.i64_type()
                );
                let elem = load!(builder, elem_addr, context.i8_type());
                let res = match instruction {
                    ByteCode::DataIncr(_) => builder.build_int_add(
                        elem.into_int_value(),
                        context.i8_type().const_int(delta as u64, false),
                        "incr_elem",
                    ),
                    _ => builder.build_int_sub(
                        elem.into_int_value(),
                        context.i8_type().const_int(delta as u64, false),
                        "decr_elem",
                    ),
                };
                builder.build_store(elem_addr, res);
            }
            ByteCode::Write => {
                // putchar((i32) memory[*dataptr_addr] )
                let dataptr = load!(builder, dataptr_addr, context.i64_type());
                let elem_addr = gep!(
                    builder,
                    memory,
                    dataptr.into_int_value(),
                    context.i64_type()
                );
                let elem = load!(builder, elem_addr, context.i8_type());
                let elem_as_i32 = builder.build_int_cast(
                    elem.into_int_value(),
                    context.i32_type().into(),
                    "i32 cast",
                );
                builder.build_direct_call(
                    module.get_function(PUTCHAR).unwrap(),
                    &[elem_as_i32.into()],
                    "write",
                );
            }
            ByteCode::Read => {
                // memory[*dataptr_addr]= getchar();
                let read_result = builder
                    .build_direct_call(module.get_function(GETCHAR).unwrap(), &[], "read")
                    .try_as_basic_value()
                    .left()
                    .unwrap();
                let elem = builder.build_int_cast(
                    read_result.into_int_value(),
                    context.i8_type().into(),
                    "i8 cast from i32",
                );
                let dataptr = load!(builder, dataptr_addr, context.i64_type());
                let elem_addr = gep!(
                    builder,
                    memory,
                    dataptr.into_int_value(),
                    context.i64_type()
                );
                builder.build_store(elem_addr, elem);
            }
            ByteCode::JZ => {
                let dataptr = load!(builder, dataptr_addr, context.i64_type());
                let offset = gep!(
                    builder,
                    memory,
                    dataptr.into_int_value(),
                    context.i64_type()
                );
                let val = load!(builder, offset, context.i8_type());
                let compare = builder.build_int_compare(
                    inkwell::IntPredicate::EQ,
                    val.into_int_value(),
                    context.i8_type().const_int(0, false),
                    "cmp_0",
                );

                let loop_body_bb =
                    context.append_basic_block(module.get_function(JIT_FUNC_NAME).unwrap(), "body");
                let loop_end_bb =
                    context.append_basic_block(module.get_function(JIT_FUNC_NAME).unwrap(), "end");
                builder.build_conditional_branch(compare.into(), loop_end_bb, loop_body_bb);
                builder.position_at_end(loop_body_bb);
                matching_blocks.push((loop_body_bb, loop_end_bb));
            }
            ByteCode::JNZ => {
                let (open_label, close_label) = matching_blocks.pop().expect("Invalid program");

                let dataptr = load!(builder, dataptr_addr, context.i64_type());
                let offset = gep!(
                    builder,
                    memory,
                    dataptr.into_int_value(),
                    context.i64_type()
                );
                let val = load!(builder, offset, context.i8_type());
                let compare = builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    val.into_int_value(),
                    context.i8_type().const_int(0, false),
                    "cmp_0",
                );
                builder.build_conditional_branch(compare.into(), open_label, close_label);
                builder.position_at_end(close_label);
            }
            ByteCode::SETZERO => {
                // memory[*dataptr_addr] = 0
                let dataptr = load!(builder, dataptr_addr, context.i64_type());
                let elem_addr = gep!(
                    builder,
                    memory,
                    dataptr.into_int_value(),
                    context.i64_type()
                );
                builder.build_store(elem_addr, context.i8_type().const_int(0, false));
            }

            ByteCode::MoveInStepUntilZero(chng) => {
                self.jit_instr(
                    ByteCode::JZ,
                    module,
                    builder,
                    dataptr_addr,
                    memory,
                    matching_blocks,
                );
                self.jit_instr(
                    match chng {
                        Change::Incr(x) => ByteCode::DataPointerIncr(x),
                        Change::Decr(x) => ByteCode::DataPointerDecr(x),
                    },
                    module,
                    builder,
                    dataptr_addr,
                    memory,
                    matching_blocks,
                );
                self.jit_instr(
                    ByteCode::JNZ,
                    module,
                    builder,
                    dataptr_addr,
                    memory,
                    matching_blocks,
                );
            }
        }
    }
    pub fn jit(&self, instructions: Vec<ByteCode>) {
        // - Setup context
        // - Setup module
        // - Setup builder
        // - Setup execution engine
        // let context = Context::create();
        // let module = context.create_module("bf_module");
        // let builder = context.create_builder();

        // let execution_engine = module
        //     .create_jit_execution_engine(OptimizationLevel::None)
        //     .expect("Failed to create execution_engine");

        // - Create function for the bf program
        // let context = context::create();

        let context = &self.context;
        let void_type = context.void_type();
        let module = context.create_module("bf_module");
        let builder = context.create_builder();

        let fn_type = void_type.fn_type(&[], false);
        let function = module.add_function(JIT_FUNC_NAME, fn_type, Some(Linkage::External));
        module.add_function(
            PUTCHAR,
            context
                .i32_type()
                .fn_type(&[BasicMetadataTypeEnum::IntType(context.i32_type())], false),
            Some(Linkage::External),
        );
        module.add_function(
            GETCHAR,
            // void_type.fn_type(&[BasicMetadataTypeEnum::IntType(context.i32_type())], false),
            context.i32_type().fn_type(&[], false),
            Some(Linkage::External),
        );
        let entry = context.append_basic_block(function, "entry");

        builder.position_at_end(entry);

        let memory = builder.build_array_alloca(
            context.i8_type(),
            context.i64_type().const_int(MEMORY_SIZE as u64, false),
            "memory",
        );
        builder
            .build_memset(
                memory,
                Layout::new::<i8>().align() as u32,
                context.i8_type().const_int(0, false),
                context.i64_type().const_int(MEMORY_SIZE as u64, false),
            )
            .unwrap();

        // stores the current index
        let dataptr_addr = builder.build_alloca(context.i64_type(), "dataptr_addr");
        builder.build_store(dataptr_addr, context.i64_type().const_int(0, false));

        let mut matching_blocks = vec![];
        for instr in instructions {
            self.jit_instr(
                instr,
                &module,
                &builder,
                dataptr_addr,
                memory,
                &mut matching_blocks,
            );
        }
        builder.build_return(None);

        // println!("{}", module.to_string());

        let execution_engine = module
            .create_jit_execution_engine(inkwell::OptimizationLevel::Aggressive)
            .expect("Failed to create execution engine");

        unsafe {
            let bf_fn = execution_engine
                .get_function::<unsafe extern "C" fn() -> ()>(JIT_FUNC_NAME)
                .unwrap();
            bf_fn.call();
        }
    }
    pub fn parse_and_run(src_code: String) {
        // Get the program parsed to bytecode
        let prog = Parser::parse_to_bytecode(src_code);
        inkwell::targets::Target::initialize_native(&InitializationConfig::default())
            .expect("Failed to initialize native target");
        let context = Context::create();
        let compiler = Self { context };

        compiler.jit(prog.instructions);
    }
}

#[cfg(test)]
mod tests {

    use inkwell::context::Context;

    use super::ByteCode;
    use super::LlvmJit;

    #[test]
    fn test_emitting() {
        let compiler = LlvmJit {
            context: Context::create(),
        };

        compiler.jit(vec![
            ByteCode::DataIncr(104), //'h'
            ByteCode::Write,
            ByteCode::DataIncr(1), // 'i'
            ByteCode::Write,
            ByteCode::DataPointerIncr(1),
            ByteCode::DataIncr(10),
            ByteCode::Write,
        ]); // Works

        // This also works fine so putchar/getchar work fine
        // compiler.jit(vec![ByteCode::Read, ByteCode::Write]); // Works
    }

    #[test]
    fn hello_world() {
        let code = include_str!("../programs/hello_world.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn mandelbrot() {
        let code = include_str!("../programs/mandelbrot.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn nested_loop() {
        let code = include_str!("../programs/nested_loop.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn number_crunce() {
        let code = include_str!("../programs/number_crunch.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn serpinski() {
        let code = include_str!("../programs/serpinski.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }

    #[test]
    fn trivial_loop() {
        let code = include_str!("../programs/trivial_loop.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn trivial_loop2() {
        let code = include_str!("../programs/trivial_loop2.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }
    #[test]
    fn z() {
        let code = include_str!("../programs/z.bf");
        LlvmJit::parse_and_run(code.to_owned());
    }
}
