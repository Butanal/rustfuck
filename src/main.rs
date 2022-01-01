use std::io::Read;

use inkwell::{context::Context, AddressSpace, module::Module, values::{FunctionValue, PointerValue}, builder::Builder, IntPredicate, types::{IntType, PointerType}};

#[derive(Clone, Debug)]
enum OpCode {
    IncrementPointer,
    DecrementPointer,
    Increment,
    Decrement,
    Read,
    Write,
    LoopBegin,
    LoopEnd
}

#[derive(Clone, Debug)]
enum Instruction {
    IncrementPointer,
    DecrementPointer,
    Increment,
    Decrement,
    Read,
    Write,
    Loop(Vec<Instruction>)
}

fn lex(source: String) -> Vec<OpCode> {
    let mut operations = Vec::new();

    for symbol in source.chars() {
        let op = match symbol {
            '>' => Some(OpCode::IncrementPointer),
            '<' => Some(OpCode::DecrementPointer),
            '+' => Some(OpCode::Increment),
            '-' => Some(OpCode::Decrement),
            ',' => Some(OpCode::Read),
            '.' => Some(OpCode::Write),
            '[' => Some(OpCode::LoopBegin),
            ']' => Some(OpCode::LoopEnd),
            _ => None
        };

        if let Some(op) = op {
            operations.push(op)
        }
    }

    operations
}

fn parse(opcodes: Vec<OpCode>) -> Vec<Instruction> {
    let mut program: Vec<Instruction> = Vec::new();
    let mut loop_stack = 0;
    let mut loop_start = 0;

    for (i, op) in opcodes.iter().enumerate() {
        if loop_stack == 0 {
            let instr = match op {
                OpCode::IncrementPointer => Some(Instruction::IncrementPointer),
                OpCode::DecrementPointer => Some(Instruction::DecrementPointer),
                OpCode::Increment => Some(Instruction::Increment),
                OpCode::Decrement => Some(Instruction::Decrement),
                OpCode::Read => Some(Instruction::Read),
                OpCode::Write => Some(Instruction::Write),
                OpCode::LoopBegin => {
                    loop_start = i;
                    loop_stack += 1;
                    None
                },
                OpCode::LoopEnd => panic!("loop ending at #{} has no beginning!", i),
            };
            
            if let Some(instr) = instr {
                program.push(instr)
            }
        } else {
            match op {
                OpCode::LoopBegin => {
                    loop_stack += 1;
                },
                OpCode::LoopEnd => {
                    loop_stack -= 1;

                    if loop_stack == 0 {
                        program.push(Instruction::Loop(parse((opcodes[loop_start+1..i]).to_vec())))
                    }
                }
                _ => ()
            }
        }
    }

    program
}

struct ExternalFunctions<'a> {
    getchar: FunctionValue<'a>,
    putchar: FunctionValue<'a>,
}

struct CommonTypes<'a> {
    i8: IntType<'a>,
    ptr: PointerType<'a>,
    ptr_int: IntType<'a>
}

struct CodeGenContext<'a> {
    builder: Builder<'a>,
    context: &'a Context,
    main: FunctionValue<'a>,
    module: Module<'a>,
    tape_head: PointerValue<'a>,
    external_fns: ExternalFunctions<'a>,
    common_types: CommonTypes<'a>
}

impl<'a> CodeGenContext<'_> {
    fn get_head_ptr(&self) -> PointerValue {
        let head_val = self.builder.build_load(self.tape_head, "").into_pointer_value();
        self.builder.build_pointer_cast(head_val, self.common_types.ptr, "")
    }

    fn generate(&mut self, instructions: &[Instruction]) {
        let context = self.context;
    
        // Initialize some values
        let i8_type = self.common_types.i8;
        let ptr_type = self.common_types.ptr;
        let ptr_int_type = self.common_types.ptr_int;
    
        let ptr_one = ptr_int_type.const_int(1, false);
        let byte_one = i8_type.const_int(1, false);
    
        for instr in instructions {
            match instr {
                Instruction::IncrementPointer => {
                    let head_val = self.builder.build_ptr_to_int(self.get_head_ptr(), ptr_int_type, "");
                    let new_head = self.builder.build_int_to_ptr(self.builder.build_int_add(head_val, ptr_one, ""), ptr_type, "");
                    self.builder.build_store(self.tape_head, new_head);
                },
                Instruction::DecrementPointer => {
                    let head_val = self.builder.build_ptr_to_int(self.get_head_ptr(), ptr_int_type, "");
                    let new_head = self.builder.build_int_to_ptr(self.builder.build_int_add(head_val, self.builder.build_int_neg(ptr_one, ""), ""), ptr_type, "");
                    self.builder.build_store(self.tape_head, new_head);
                },
                Instruction::Increment => {
                    let head_val = self.get_head_ptr();
                    let head_content = self.builder.build_load(head_val, "").into_int_value();
                    let new_content = self.builder.build_int_add(head_content, byte_one, "");
                    self.builder.build_store(head_val, new_content);
                },
                Instruction::Decrement => {
                    let head_val = self.get_head_ptr();
                    let head_content = self.builder.build_load(head_val, "").into_int_value();
                    let new_content = self.builder.build_int_add(head_content, self.builder.build_int_neg(byte_one, ""), "");
                    self.builder.build_store(head_val, new_content);
                },
                Instruction::Read => {
                    let char = self.builder.build_call(self.external_fns.getchar, &[], "").try_as_basic_value().expect_left("getchar call returned no value :(");
                    
                    let head_val = self.get_head_ptr();
                    self.builder.build_store(head_val, char);
                },
                Instruction::Write => {
                    let head_val = self.get_head_ptr();
                    let char = self.builder.build_load(head_val, "").into_int_value();
                    let args = [char.into()];
                    self.builder.build_call(self.external_fns.putchar, &args, "");
                },
                Instruction::Loop(nested_instructions) => {
                    let loop_cond = context.append_basic_block(self.main, "loopcond");
                    let loop_body = context.append_basic_block(self.main, "loop");
                    let after_loop = context.append_basic_block(self.main, "endloop");

                    self.builder.build_unconditional_branch(loop_cond);
                    self.builder.position_at_end(loop_cond);

                    let head_val = self.get_head_ptr();
                    
                    let head_content = self.builder.build_load(head_val, "").into_int_value();
                    let should_execute = self.builder.build_int_compare(IntPredicate::NE, head_content, i8_type.const_zero(), "");

                    self.builder.build_conditional_branch(should_execute, loop_body, after_loop);
                    self.builder.position_at_end(loop_body);
                    self.generate(nested_instructions);
                    self.builder.build_unconditional_branch(loop_cond);
                    self.builder.position_at_end(after_loop);
                },
            }
        }
    }
}

fn generate_llvm(instructions: &[Instruction]) {
    let context = Context::create();
    let module = context.create_module("rustfuck");
    
    let builder = context.create_builder();

    let void = context.void_type();
    let func_type = void.fn_type(&[], false);
    let main = module.add_function("main", func_type, None);
    let basic_block = context.append_basic_block(main, "entry");

    builder.position_at_end(basic_block);

    // Initialize types
    let i8_type = context.i8_type();
    let i32_type = context.i32_type();
    let ptr_type = i8_type.ptr_type(AddressSpace::Generic);
    let ptr_int_type = context.i64_type();

    // Initialize memset function
    let param_types = [ptr_type.into(), i32_type.into(), ptr_int_type.into()];
    let memset_type = ptr_type.fn_type(&param_types, false);
    let memset = module.add_function("memset", memset_type, None);

    // Initialize putchar function
    let param_types = [i8_type.into()];
    let putchar_type = i8_type.fn_type(&param_types, false);
    let putchar = module.add_function("putchar", putchar_type, None);

    // Initialize putchar function
    let getchar_type = i8_type.fn_type(&[], false);
    let getchar = module.add_function("getchar", getchar_type, None);

    // Initialize the tape
    let tape_size = ptr_int_type.const_int(1024, false);
    let tape = builder.build_array_alloca(i8_type, tape_size, "tape");
    // Initialize the variable for the tape head
    let tape_head = builder.build_alloca(ptr_type, "");
    builder.build_store(tape_head, tape);
    // Zero out the tape
    let zero = i32_type.const_zero();
    let args = [tape.into(), zero.into(), tape_size.into()];
    builder.build_call(memset, &args, "");

    let mut codegen = CodeGenContext{
        builder,
        context: &context,
        main,
        module,
        tape_head,
        external_fns: ExternalFunctions{
            getchar,
            putchar,
        },
        common_types: CommonTypes { i8: i8_type, ptr: ptr_type, ptr_int: ptr_int_type }
    };

    codegen.generate(instructions);

    codegen.builder.build_return(None);
    codegen.module.print_to_file("out.ll").unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <file.bf>", args[0]);
        std::process::exit(1);
    }

    let mut file = std::fs::File::open(&args[1]).unwrap();
    let mut source = String::new();
    file.read_to_string(&mut source).unwrap();
    

    let opcodes = lex(source);
    let program = parse(opcodes);

    generate_llvm(&program);
}
