# rustfuck
rustfuck is a basic Brainfuck compiler emitting LLVM IR for cross-architecture compilation.

## Purpose

I created this project to have fun learning how to generate and work with LLVM IR.

## Current state

The compiler fully works, but is not very polished. It depends on `libc` functions for character input and output.

## How to use

Clone the project and compile it:

```bash
$ cargo build --release
```

Now compile your Brainfuck program:

```bash
$ ./target/release/rustfuck helloworld.b
$ llc -filetype=obj out.ll -o out.o
$ clang -o out out.o
```
and run it!

```bash
❯ ./out
Hello World!
```
