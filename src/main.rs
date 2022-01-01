use std::io::Read;

use inkwell::{context::Context, AddressSpace, module::{Linkage, Module}, values::{BasicValueEnum, IntValue, FunctionValue}, builder::Builder, IntPredicate};

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

        match op {
            Some(op) => operations.push(op),
            None => ()
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
            
            match instr {
                Some(instr) => program.push(instr),
                None => (),
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

fn run(instructions: &Vec<Instruction>, tape: &mut Vec<u8>, data_pointer: &mut usize) {
    for instr in instructions {
        match instr {
            Instruction::IncrementPointer => *data_pointer += 1,
            Instruction::DecrementPointer => *data_pointer -= 1,
            Instruction::Increment => tape[*data_pointer] += 1,
            Instruction::Decrement => tape[*data_pointer] -= 1,
            Instruction::Write => print!("{}", tape[*data_pointer] as char),
            Instruction::Read => {
                let mut input: [u8; 1] = [0; 1];
                std::io::stdin().read_exact(&mut input).expect("failed to read stdin");
                tape[*data_pointer] = input[0];
            },
            Instruction::Loop(nested_instructions) => {
                while tape[*data_pointer] != 0 {
                    run(&nested_instructions, tape, data_pointer)
                }
            }
        }
    }
}

struct ExternalFunctions<'a> {
    getchar: FunctionValue<'a>,
    putchar: FunctionValue<'a>,
}

struct CodeGen<'a> {
    builder: Builder<'a>,
    context: &'a Context,
    main: FunctionValue<'a>,
    module: Module<'a>,
    tape_head: IntValue<'a>,
    external_fns: ExternalFunctions<'a>
}

impl<'a> CodeGen<'_> {
    fn generate(&mut self, instructions: &Vec<Instruction>) {
        let context = self.context;
    
        // Initialize some values
        let i8_type = context.i8_type();
        let i32_type = context.i32_type();
        let ptr_type = context.i64_type();
    
        let ptr_one = ptr_type.const_int(1, false);
        let byte_one = i8_type.const_int(1, false);
    
        for instr in instructions {
            let head_ptr = self.builder.build_int_to_ptr(self.tape_head, i8_type.ptr_type(AddressSpace::Generic), "");
            match instr {
                Instruction::IncrementPointer => {
                    self.tape_head = self.builder.build_int_add(self.tape_head, ptr_one, "");
                },
                Instruction::DecrementPointer => {
                    self.tape_head = self.builder.build_int_add(self.tape_head, self.builder.build_int_neg(ptr_one, ""), "");
                },
                Instruction::Increment => {
                    let old_val = self.builder.build_load(head_ptr, "").into_int_value();
                    let new_val = self.builder.build_int_add(old_val, byte_one, "");
                    self.builder.build_store(head_ptr, new_val);
                },
                Instruction::Decrement => {
                    let old_val = self.builder.build_load(head_ptr, "").into_int_value();
                    let new_val = self.builder.build_int_add(old_val, self.builder.build_int_neg(byte_one, ""), "");
                    self.builder.build_store(head_ptr, new_val);
                },
                Instruction::Read => {
                    let char = self.builder.build_call(self.external_fns.getchar, &[], "").try_as_basic_value().expect_left("getchar call returned no value :(");
                    self.builder.build_store(head_ptr, char);
                },
                Instruction::Write => {
                    let char = self.builder.build_load(head_ptr, "").into_int_value();
                    let args = [char.into()];
                    self.builder.build_call(self.external_fns.putchar, &args, "");
                },
                Instruction::Loop(nested_instructions) => {
                    let value = self.builder.build_load(head_ptr, "").into_int_value();
                    let should_execute = self.builder.build_int_compare(IntPredicate::NE, value, i8_type.const_zero(), "");

                    let loop_cond = context.append_basic_block(self.main, "loopcond");
                    let loop_body = context.append_basic_block(self.main, "loop");
                    let after_loop = context.append_basic_block(self.main, "endloop");

                    self.builder.build_unconditional_branch(loop_cond);

                    self.builder.position_at_end(loop_cond);
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

fn generate_llvm(instructions: &Vec<Instruction>) {
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
    let ptr_type = context.i64_type();

    // Initialize memset function
    let param_types = [i8_type.ptr_type(AddressSpace::Generic).into(), i32_type.into(), ptr_type.into()];
    let memset_type = i8_type.ptr_type(AddressSpace::Generic).fn_type(&param_types, false);
    let memset = module.add_function("memset", memset_type, None);

    // Initialize putchar function
    let param_types = [i8_type.into()];
    let putchar_type = i8_type.fn_type(&param_types, false);
    let putchar = module.add_function("putchar", putchar_type, None);

    // Initialize putchar function
    let getchar_type = i8_type.fn_type(&[], false);
    let getchar = module.add_function("getchar", getchar_type, None);

    // Initialize the tape
    let tape_size = ptr_type.const_int(1024, false);
    let tape = builder.build_array_alloca(i8_type, tape_size, "tape");
    // Zero out the tape
    let zero = i32_type.const_zero();
    let args = [tape.into(), zero.into(), tape_size.into()];
    builder.build_call(memset, &args, "");

    let tape_head = builder.build_ptr_to_int(tape, ptr_type, "");

    let mut codegen = CodeGen{
        builder,
        context: &context,
        main,
        module,
        tape_head,
        external_fns: ExternalFunctions{
            getchar,
            putchar,
        }
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

    /*
    let mut tape: Vec<u8> = vec![0; 1024];
    let mut data_pointer = 512;
    run(&program, &mut tape, &mut data_pointer);
    */

    generate_llvm(&program);
}
