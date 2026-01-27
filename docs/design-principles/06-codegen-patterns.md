# Code Generation & Backends

Quick-reference guide to compilation targets, IR design, and code generation patterns.

---

## Compilation Target Options

### Target Comparison
| Target | Portability | Performance | Complexity | Debug Support |
|--------|-------------|-------------|------------|---------------|
| C code | Excellent | Good | Low | Okay (#line) |
| LLVM IR | Good | Excellent | Medium | Excellent |
| Native | Per-arch | Excellent | High | Varies |
| Bytecode | Excellent | Medium | Medium | Good |
| WASM | Web + WASI | Good | Medium | Improving |

### When to Choose Each

**C Code Generation**
- Maximum portability
- Leverage mature C compilers
- Easier bootstrapping
- Good for: New languages, DSLs, scripting languages

**LLVM IR**
- Professional optimization
- Multi-target support
- Rich debugging
- Good for: Production compilers, performance-critical

**Custom Bytecode**
- Full control
- Fast compilation
- Portable runtime
- Good for: Scripting, embedded, educational

**Native Code**
- Ultimate performance
- No dependencies
- Complex implementation
- Good for: Systems languages, compilers

---

## C Code Generation

### Basic Structure
```rust
struct CCodegen {
    output: String,
    indent: usize,
    temp_counter: u32,
    includes: HashSet<String>,
}

impl CCodegen {
    fn emit(&mut self, s: &str) {
        self.output.push_str(&"    ".repeat(self.indent));
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn fresh_temp(&mut self) -> String {
        let name = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }
}
```

### Expression Codegen
```rust
fn gen_expr(&mut self, expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::FloatLit(n) => format!("{:.17}", n),
        Expr::StringLit(s) => format!("\"{}\"", escape_string(s)),
        Expr::BoolLit(b) => if *b { "1" } else { "0" }.to_string(),

        Expr::Ident(name) => mangle_name(name),

        Expr::Binary(op, left, right) => {
            let l = self.gen_expr(left);
            let r = self.gen_expr(right);
            let c_op = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Eq => "==",
                BinOp::Lt => "<",
                BinOp::And => "&&",
                BinOp::Or => "||",
                // ...
            };
            format!("({} {} {})", l, c_op, r)  // Over-parenthesize!
        }

        Expr::Call(func, args) => {
            let func_name = self.gen_expr(func);
            let arg_strs: Vec<_> = args.iter()
                .map(|a| self.gen_expr(a))
                .collect();
            format!("{}({})", func_name, arg_strs.join(", "))
        }

        Expr::If(cond, then_, else_) => {
            let temp = self.fresh_temp();
            self.emit(&format!("int {};", temp));
            let c = self.gen_expr(cond);
            self.emit(&format!("if ({}) {{", c));
            self.indent += 1;
            let t = self.gen_expr(then_);
            self.emit(&format!("{} = {};", temp, t));
            self.indent -= 1;
            self.emit("} else {");
            self.indent += 1;
            let e = self.gen_expr(else_.as_ref().unwrap());
            self.emit(&format!("{} = {};", temp, e));
            self.indent -= 1;
            self.emit("}");
            temp
        }
        // ...
    }
}
```

### Statement Codegen
```rust
fn gen_stmt(&mut self, stmt: &Stmt) {
    match stmt {
        Stmt::Let(name, init) => {
            let val = self.gen_expr(init);
            let ty = type_to_c(&init.ty);
            self.emit(&format!("{} {} = {};", ty, mangle_name(name), val));
        }

        Stmt::Assign(target, value) => {
            let t = self.gen_expr(target);
            let v = self.gen_expr(value);
            self.emit(&format!("{} = {};", t, v));
        }

        Stmt::Return(expr) => {
            if let Some(e) = expr {
                let v = self.gen_expr(e);
                self.emit(&format!("return {};", v));
            } else {
                self.emit("return;");
            }
        }

        Stmt::While(cond, body) => {
            let c = self.gen_expr(cond);
            self.emit(&format!("while ({}) {{", c));
            self.indent += 1;
            for s in body {
                self.gen_stmt(s);
            }
            self.indent -= 1;
            self.emit("}");
        }
        // ...
    }
}
```

### Function Codegen
```rust
fn gen_function(&mut self, func: &Function) {
    let ret_ty = type_to_c(&func.ret_type);
    let params: Vec<String> = func.params.iter()
        .map(|(name, ty)| format!("{} {}", type_to_c(ty), mangle_name(name)))
        .collect();

    self.emit(&format!("{} {}({}) {{",
        ret_ty,
        mangle_name(&func.name),
        params.join(", ")
    ));

    self.indent += 1;
    for stmt in &func.body {
        self.gen_stmt(stmt);
    }
    self.indent -= 1;

    self.emit("}");
}
```

### Type Mapping
```rust
fn type_to_c(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "int64_t",
        Type::Float => "double",
        Type::Bool => "int",
        Type::String => "char*",
        Type::Void => "void",
        Type::Array(_) => "void*",  // Or generate struct
        Type::Struct(name) => name,  // Forward declare
        _ => "void*",  // Generic pointer
    }
}
```

### Name Mangling
```rust
fn mangle_name(name: &str) -> String {
    // Avoid C reserved words
    let reserved = ["auto", "break", "case", "char", "const", ...];
    if reserved.contains(&name) {
        format!("{}_", name)
    } else {
        format!("__{}", name.replace('-', "_"))
    }
}
```

### Debug Line Directives
```c
#line 42 "source.lang"
int __add(int64_t a, int64_t b) {
    return a + b;
}
```

---

## LLVM IR Generation

### Basic LLVM IR Concepts
```llvm
; Types
i1        ; boolean
i8, i32, i64   ; integers
float, double  ; floats
ptr       ; pointer
[10 x i32]    ; array
{i32, i64}    ; struct

; SSA value
%result = add i32 %a, %b

; Function
define i32 @add(i32 %a, i32 %b) {
entry:
    %sum = add i32 %a, %b
    ret i32 %sum
}

; Control flow
br i1 %cond, label %then, label %else

; Memory
%ptr = alloca i32           ; stack allocation
store i32 %val, ptr %ptr    ; write
%loaded = load i32, ptr %ptr ; read
```

### LLVM-C API Pattern
```rust
// Using llvm-sys or inkwell crate
fn codegen_function(&mut self, func: &Function) {
    let fn_type = self.llvm_fn_type(&func.orig);
    let llvm_fn = self.module.add_function(&func.name, fn_type);

    let entry_bb = self.context.append_basic_block(llvm_fn, "entry");
    self.builder.position_at_end(entry_bb);

    // Create allocas for parameters
    for (i, param) in func.params.iter().enumerate() {
        let alloca = self.builder.build_alloca(param.ty, &param.name);
        let param_val = llvm_fn.get_nth_param(i as u32);
        self.builder.build_store(param_val, alloca);
        self.locals.insert(param.name.clone(), alloca);
    }

    // Generate body
    for stmt in &func.body {
        self.codegen_stmt(stmt);
    }
}

fn codegen_expr(&mut self, expr: &Expr) -> BasicValueEnum {
    match expr {
        Expr::IntLit(n) => self.context.i64_type().const_int(*n as u64, true).into(),

        Expr::Binary(BinOp::Add, left, right) => {
            let l = self.codegen_expr(left).into_int_value();
            let r = self.codegen_expr(right).into_int_value();
            self.builder.build_int_add(l, r, "add").into()
        }

        Expr::If(cond, then_, else_) => {
            let cond_val = self.codegen_expr(cond).into_int_value();

            let then_bb = self.context.append_basic_block(self.current_fn, "then");
            let else_bb = self.context.append_basic_block(self.current_fn, "else");
            let merge_bb = self.context.append_basic_block(self.current_fn, "merge");

            self.builder.build_conditional_branch(cond_val, then_bb, else_bb);

            // Then branch
            self.builder.position_at_end(then_bb);
            let then_val = self.codegen_expr(then_);
            self.builder.build_unconditional_branch(merge_bb);
            let then_bb = self.builder.get_insert_block().unwrap();

            // Else branch
            self.builder.position_at_end(else_bb);
            let else_val = self.codegen_expr(else_);
            self.builder.build_unconditional_branch(merge_bb);
            let else_bb = self.builder.get_insert_block().unwrap();

            // Merge with PHI
            self.builder.position_at_end(merge_bb);
            let phi = self.builder.build_phi(self.context.i64_type(), "iftmp");
            phi.add_incoming(&[(&then_val, then_bb), (&else_val, else_bb)]);
            phi.as_basic_value()
        }
        // ...
    }
}
```

### SSA Form Notes
- Each value defined exactly once
- Use `alloca` for mutable variables (LLVM's `mem2reg` optimizes)
- PHI nodes for values from different branches
- Let LLVM handle register allocation

---

## Bytecode Design

### Instruction Encoding
```rust
#[repr(u8)]
enum OpCode {
    // Stack operations
    Constant = 0x00,  // Push constant from pool
    Pop = 0x01,
    Dup = 0x02,

    // Locals
    GetLocal = 0x10,  // Push local variable
    SetLocal = 0x11,  // Pop and store in local

    // Arithmetic
    Add = 0x20,
    Sub = 0x21,
    Mul = 0x22,
    Div = 0x23,
    Neg = 0x24,

    // Comparison
    Eq = 0x30,
    Lt = 0x31,
    Gt = 0x32,

    // Control flow
    Jump = 0x40,      // Unconditional jump
    JumpIfFalse = 0x41,
    Call = 0x42,
    Return = 0x43,

    // Misc
    Print = 0x50,
    Halt = 0xFF,
}
```

### Chunk/Module Structure
```rust
struct Chunk {
    code: Vec<u8>,           // Bytecode
    constants: Vec<Value>,   // Constant pool
    lines: Vec<u32>,         // Source line per instruction
}

impl Chunk {
    fn write(&mut self, byte: u8, line: u32) {
        self.code.push(byte);
        self.lines.push(line);
    }

    fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() - 1) as u8
    }

    fn write_constant(&mut self, value: Value, line: u32) {
        let idx = self.add_constant(value);
        self.write(OpCode::Constant as u8, line);
        self.write(idx, line);
    }
}
```

### Bytecode Compiler
```rust
fn compile_expr(&mut self, expr: &Expr) {
    match expr {
        Expr::IntLit(n) => {
            let idx = self.chunk.add_constant(Value::Int(*n));
            self.emit(OpCode::Constant);
            self.emit_byte(idx);
        }

        Expr::Binary(op, left, right) => {
            self.compile_expr(left);
            self.compile_expr(right);
            match op {
                BinOp::Add => self.emit(OpCode::Add),
                BinOp::Sub => self.emit(OpCode::Sub),
                BinOp::Mul => self.emit(OpCode::Mul),
                BinOp::Div => self.emit(OpCode::Div),
                _ => {}
            }
        }

        Expr::If(cond, then_, else_) => {
            self.compile_expr(cond);

            // Jump to else if false
            let else_jump = self.emit_jump(OpCode::JumpIfFalse);

            self.emit(OpCode::Pop);  // Pop condition
            self.compile_expr(then_);

            let end_jump = self.emit_jump(OpCode::Jump);

            self.patch_jump(else_jump);
            self.emit(OpCode::Pop);  // Pop condition
            self.compile_expr(else_);

            self.patch_jump(end_jump);
        }
        // ...
    }
}

fn emit_jump(&mut self, op: OpCode) -> usize {
    self.emit(op);
    self.emit_byte(0xFF);  // Placeholder
    self.emit_byte(0xFF);
    self.chunk.code.len() - 2  // Return offset to patch
}

fn patch_jump(&mut self, offset: usize) {
    let jump = self.chunk.code.len() - offset - 2;
    self.chunk.code[offset] = ((jump >> 8) & 0xFF) as u8;
    self.chunk.code[offset + 1] = (jump & 0xFF) as u8;
}
```

### Stack-Based VM
```rust
struct VM {
    chunk: Chunk,
    ip: usize,              // Instruction pointer
    stack: Vec<Value>,
    locals: Vec<Value>,
}

impl VM {
    fn run(&mut self) -> Result<Value, Error> {
        loop {
            let op = self.read_byte();
            match OpCode::try_from(op)? {
                OpCode::Constant => {
                    let idx = self.read_byte() as usize;
                    self.stack.push(self.chunk.constants[idx].clone());
                }

                OpCode::Add => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(Value::Int(a.as_int() + b.as_int()));
                }

                OpCode::JumpIfFalse => {
                    let offset = self.read_u16();
                    if !self.stack.last().unwrap().is_truthy() {
                        self.ip += offset as usize;
                    }
                }

                OpCode::Jump => {
                    let offset = self.read_u16();
                    self.ip += offset as usize;
                }

                OpCode::Return => {
                    return Ok(self.stack.pop().unwrap_or(Value::Nil));
                }

                OpCode::Halt => break,
                // ...
            }
        }
        Ok(Value::Nil)
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.chunk.code[self.ip];
        self.ip += 1;
        byte
    }

    fn read_u16(&mut self) -> u16 {
        let high = self.read_byte() as u16;
        let low = self.read_byte() as u16;
        (high << 8) | low
    }
}
```

---

## Register-Based Bytecode

### vs Stack-Based
| Aspect | Stack-Based | Register-Based |
|--------|-------------|----------------|
| Instructions | Simpler, more | Fewer, complex |
| Dispatch overhead | More | Less |
| Implementation | Simpler | More complex |
| Examples | JVM, Python | Lua 5, Dalvik |

### Register Instructions
```rust
enum OpCode {
    // LOAD r, const   - Load constant into register r
    Load = 0x00,

    // MOVE dst, src   - Copy register
    Move = 0x01,

    // ADD dst, a, b   - dst = a + b
    Add = 0x10,

    // JMP offset      - Unconditional jump
    Jump = 0x20,

    // JMPF r, offset  - Jump if r is false
    JumpIfFalse = 0x21,

    // CALL r, func, nargs - Call func with nargs, result in r
    Call = 0x30,

    // RET r           - Return value in r
    Return = 0x40,
}

// Instruction format: opcode (8) | operands (24)
// ADD: | op | dst | src1 | src2 |
```

---

## Multi-Backend Architecture

### Backend Trait
```rust
trait CodegenBackend {
    fn compile_module(&mut self, module: &Module) -> Result<Vec<u8>, Error>;
    fn emit_function(&mut self, func: &Function);
    fn emit_expr(&mut self, expr: &Expr);
}

struct LLVMBackend { /* ... */ }
struct CBackend { /* ... */ }
struct WasmBackend { /* ... */ }

impl CodegenBackend for LLVMBackend { /* ... */ }
impl CodegenBackend for CBackend { /* ... */ }
impl CodegenBackend for WasmBackend { /* ... */ }
```

### Zig-Style Architecture
1. Frontend → ZIR (Zig IR)
2. Sema → AIR (Analyzed IR)
3. Backends:
   - LLVM backend
   - x86_64 native
   - C backend
   - WASM backend

### Shared IR Benefits
- Single optimization pass
- Consistent semantics
- Backend swappable
- Easier testing

---

## Optimization Basics

### Constant Folding
```rust
fn fold_constants(expr: &Expr) -> Expr {
    match expr {
        Expr::Binary(op, left, right) => {
            let left = fold_constants(left);
            let right = fold_constants(right);

            if let (Expr::IntLit(a), Expr::IntLit(b)) = (&left, &right) {
                return match op {
                    BinOp::Add => Expr::IntLit(a + b),
                    BinOp::Sub => Expr::IntLit(a - b),
                    BinOp::Mul => Expr::IntLit(a * b),
                    _ => Expr::Binary(*op, Box::new(left), Box::new(right)),
                };
            }

            Expr::Binary(*op, Box::new(left), Box::new(right))
        }
        _ => expr.clone(),
    }
}
```

### Dead Code Elimination
```rust
fn eliminate_dead_code(stmts: &[Stmt]) -> Vec<Stmt> {
    let mut used = HashSet::new();

    // Mark phase: find used variables
    for stmt in stmts.iter().rev() {
        mark_used(stmt, &mut used);
    }

    // Sweep phase: keep only used statements
    stmts.iter()
        .filter(|s| is_used(s, &used) || has_side_effect(s))
        .cloned()
        .collect()
}
```

### Inlining
```rust
fn should_inline(func: &Function) -> bool {
    func.body.len() <= 10 &&  // Small
    !func.is_recursive &&      // Not recursive
    func.call_count > 1        // Called multiple times
}

fn inline_call(call: &CallExpr, func: &Function) -> Expr {
    let mut body = func.body.clone();
    // Substitute parameters with arguments
    for (param, arg) in func.params.iter().zip(&call.args) {
        substitute(&mut body, param, arg);
    }
    Expr::Block(body)
}
```

---

## Debug Information

### DWARF Generation (with LLVM)
```rust
fn emit_debug_info(&mut self, func: &Function) {
    let di_builder = self.module.create_debug_info_builder();

    let file = di_builder.create_file(&func.source_file);
    let compile_unit = di_builder.create_compile_unit(
        DW_LANG_C99,
        file,
        "my-compiler",
        false,
        "",
        0,
    );

    let func_type = di_builder.create_subroutine_type(file, &[]);
    let di_func = di_builder.create_function(
        compile_unit,
        &func.name,
        None,
        file,
        func.line,
        func_type,
        true,
        true,
        func.line,
    );

    self.builder.set_current_debug_location(
        self.context.create_debug_location(func.line, 0, di_func, None)
    );
}
```

### Source Maps (for transpilers)
```json
{
  "version": 3,
  "file": "output.js",
  "sources": ["input.lang"],
  "mappings": "AAAA,SAAS,GAAG..."
}
```

---

## WebAssembly Target

### WASM Module Structure
```wat
(module
  (func $add (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add
  )
  (export "add" (func $add))
)
```

### WASM Codegen
```rust
fn emit_wasm_func(&mut self, func: &Function) {
    // Function type
    self.emit_func_type(&func.params, &func.ret_type);

    // Function body
    self.emit_byte(0x00);  // Local count
    for stmt in &func.body {
        self.emit_wasm_stmt(stmt);
    }
    self.emit_byte(0x0B);  // end
}

fn emit_wasm_expr(&mut self, expr: &Expr) {
    match expr {
        Expr::IntLit(n) => {
            self.emit_byte(0x41);  // i32.const
            self.emit_leb128(*n);
        }
        Expr::Binary(BinOp::Add, left, right) => {
            self.emit_wasm_expr(left);
            self.emit_wasm_expr(right);
            self.emit_byte(0x6A);  // i32.add
        }
        Expr::Var(name) => {
            self.emit_byte(0x20);  // local.get
            self.emit_leb128(self.local_index(name));
        }
        // ...
    }
}
```

---

## Code Generation Checklist

### C Backend
- [ ] Type mapping (primitives, structs, arrays)
- [ ] Name mangling (avoid reserved words)
- [ ] Expression codegen (over-parenthesize!)
- [ ] Statement codegen
- [ ] Function codegen
- [ ] #line directives for debugging
- [ ] Header file generation
- [ ] Memory management (malloc/free or GC)

### LLVM Backend
- [ ] Module and context setup
- [ ] Type mapping to LLVM types
- [ ] Function declaration and definition
- [ ] Basic block structure
- [ ] SSA form (use allocas for simplicity)
- [ ] PHI nodes for control flow merge
- [ ] Debug info generation
- [ ] Optimization passes

### Bytecode
- [ ] Opcode design (stack or register)
- [ ] Constant pool
- [ ] Jump patching
- [ ] Line number table
- [ ] VM dispatch loop
- [ ] Call frame management

---

## Key References
- LLVM Kaleidoscope: https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/LangImpl03.html
- Crafting Interpreters (Bytecode): https://craftinginterpreters.com/chunks-of-bytecode.html
- C as target: https://github.com/dbohdan/compilers-targeting-c
- Zig compiler: `src/codegen/llvm.zig`, `src/codegen/c/`
