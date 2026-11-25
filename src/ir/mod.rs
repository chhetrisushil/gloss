//! The Gloss IR: a target-neutral machine with two registers, `a` and `b`, numbered local
//! slots, and a stack. Backends translate it to machine code; tests interpret it.

mod rodata;
mod text;

pub use rodata::{Rodata, rodata};
pub use text::{function_text, inst_text, module_text};

/// One of the two registers. Numeric instructions work on `A` and `B` and leave the result
/// in `A`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    A,
    B,
}

/// A jump target within a function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label(pub u32);

/// A signed comparison between `a` and `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Wrapping integer operations: `a = a op b`. `Div` and `Rem` are signed and truncating and
/// require a divisor greater than 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
}

/// A shift of `a`: arithmetic left, arithmetic right, or logical right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Left,
    Right,
    RightLogical,
}

/// How `Prim::Open` opens a path. Every mode resolves the path beneath the folder it is given
/// and refuses symbolic links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    /// Read only.
    Read,
    /// A folder handle, for later opens and `Prim::Sync`.
    Folder,
    /// Write only; creates the file or truncates it. `Open` takes a third argument, the
    /// permissions of a created file (the process's umask applies); a file that exists keeps
    /// its own.
    Create,
    /// No access: only shows whether the name exists and is not a link.
    Probe,
}

/// The folder argument of `Prim::Open` and `Prim::Rename` that means the current folder.
pub const CWD: i64 = -100;

/// The IR's error vocabulary: a primitive gives a negative `-errno`, with Linux's numbers;
/// other targets map their own errors to these. No such file or folder.
pub const NOT_FOUND: i64 = -2;
/// A path component is a symbolic link.
pub const LINK: i64 = -40;
/// The path leads outside the folder it is relative to.
pub const ESCAPES: i64 = -18;

/// A primitive the host provides. The arguments were pushed left to right; `System`
/// consumes them and leaves the result in `a`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    /// `Write(fd, addr, len)`: bytes written, or a negative error.
    Write,
    /// `Read(fd, addr, len)`: bytes read, 0 at end of input, negative on error.
    Read,
    /// `Map(len)`: the address of `len` fresh zeroed bytes, or 0.
    Map,
    /// `Exit(status)`: does not return.
    Exit,
    /// `StackSize()`: the soft stack limit in bytes, or -1 when unlimited.
    StackSize,
    /// `Open(folder_fd, path)`, and for `Create` `Open(folder_fd, path, permissions)`: a file descriptor, or a negative error. `folder_fd` is from
    /// `Open(OpenMode::Folder)` or is `CWD`; `path` is the address of NUL-terminated bytes.
    Open(OpenMode),
    /// `Close(fd)`: 0, or a negative error.
    Close,
    /// `Sync(fd)`: 0, or a negative error, once the file or folder is on disk.
    Sync,
    /// `Mode(fd)`: the permission bits (0 to 0o777) of the file or folder `fd` is on, or a
    /// negative error. Works on a `Probe` descriptor, which cannot be read.
    Mode,
    /// `SetMode(fd, permissions)`: sets the permission bits of the file `fd` is open on
    /// exactly (`fchmod`, the umask does not apply); 0, or a negative error.
    SetMode,
    /// `Rename(folder_fd, from, to)`: 0, or a negative error; both names are in that folder
    /// and an existing `to` is replaced.
    Rename,
    /// `Fork()`: in the parent the child's process id (positive), in the child 0, a negative
    /// error on failure. The child is a copy of the process (registers, stack, memory and
    /// globals as they were) and shares the open files and the output descriptors.
    Fork,
    /// `Wait(pid)`: waits for that child. The result is its exit status (0 to 255) when it
    /// exited, `256 + signal number` when a signal ended it, or a negative error (`-10`, no
    /// such child). Each target decodes its own wait status to this.
    Wait,
    /// `DefaultChildSignal()`: gives `SIGCHLD` its default disposition, so a process that
    /// inherited it as ignored can `Wait` for its children (an ignored `SIGCHLD` makes the
    /// kernel reap them, and every `Wait` gives `-10`). 0, or a negative error.
    DefaultChildSignal,
}

/// A runtime global, addressed with `Data::Global`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Global {
    HeapNext,
    HeapEnd,
    StackLimit,
    OutLen,
    OutBuf,
    InLen,
    InPos,
    InBuf,
    /// The address of the tables block: two words per table, in declaration order, the
    /// table's list and its loaded flag. Set by `tables_init`.
    Tables,
    /// Scratch for formatting a number as a text; see `NUMBER_BUF_BYTES`.
    NumBuf,
    /// The stack pointer at program entry, saved by the target's entry code before `_start`
    /// runs: it points at `argc`, then argv, envp and the auxiliary vector (Linux layout).
    EntrySp,
    /// The state of a `gloss test` run; see `TEST_STATE_BYTES`.
    TestState,
}

/// The size in bytes of `Global::OutBuf` and of `Global::InBuf`.
pub const IO_BUF_BYTES: usize = 4096;

/// The size in bytes of `Global::NumBuf`: a length and the longest number text.
pub const NUMBER_BUF_BYTES: usize = 32;

/// The size in bytes of `Global::TestState`.
pub const TEST_STATE_BYTES: usize = 2048;

impl Global {
    /// The snake-case name both the IR text and the machine code text print.
    pub fn name(self) -> &'static str {
        match self {
            Global::HeapNext => "heap_next",
            Global::HeapEnd => "heap_end",
            Global::StackLimit => "stack_limit",
            Global::OutLen => "out_len",
            Global::OutBuf => "out_buf",
            Global::InLen => "in_len",
            Global::InPos => "in_pos",
            Global::InBuf => "in_buf",
            Global::NumBuf => "num_buf",
            Global::EntrySp => "entry_sp",
            Global::Tables => "tables",
            Global::TestState => "test_state",
        }
    }
}

/// Something whose address `AddressOf` can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    /// A read-only text constant, by index into `Module::texts`.
    Text(u32),
    /// A type descriptor, by index into `Module::descs`.
    Desc(u32),
    Global(Global),
}

/// One instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum Inst {
    /// Marks where source line `n`'s code starts.
    Line(u32),
    Const(Reg, i64),
    /// Slots are numbered from 1.
    LoadSlot(Reg, u32),
    /// Stores a register into a slot; slots are numbered from 1.
    StoreSlot(u32, Reg),
    /// Arguments are numbered from 1, left to right.
    LoadArg(Reg, u32),
    Move {
        to: Reg,
        from: Reg,
    },
    Push(Reg),
    Pop(Reg),
    AddressOf(Reg, Data),
    StackPointer(Reg),
    Load {
        to: Reg,
        base: Reg,
        offset: i32,
    },
    Store {
        base: Reg,
        offset: i32,
        from: Reg,
    },
    LoadByte {
        to: Reg,
        base: Reg,
        offset: i32,
    },
    StoreByte {
        base: Reg,
        offset: i32,
        from: Reg,
    },
    /// `a = a + b`; jumps to `range` on overflow. The `Num*` instructions mean exactly the
    /// `Number` methods. On the failure jump of any of them, `a` is undefined and `b` is
    /// unchanged (M2-R8: on x86 the wrapped sum is already written).
    NumAdd {
        range: Label,
    },
    /// `a = a - b`; jumps to `range` on overflow. On the failure jump, `a` is undefined and `b` is unchanged.
    NumSub {
        range: Label,
    },
    /// `a = -a`; jumps to `range` on overflow. On the failure jump, `a` is undefined and `b` is unchanged.
    NumNeg {
        range: Label,
    },
    /// `a = mul_round(a, b)`. On the failure jump, `a` is undefined and `b` is unchanged.
    NumMul {
        range: Label,
    },
    /// `a = div_round(a, b)`; division by zero jumps to `zero`, an out-of-range result to
    /// `range`. On the failure jump, `a` is undefined and `b` is unchanged.
    NumDiv {
        zero: Label,
        range: Label,
    },
    /// `a = rem(a, b)`; a zero divisor jumps to `zero`. On the failure jump, `a` is undefined and `b` is unchanged.
    NumRem {
        zero: Label,
    },
    /// `a = whole(a)`.
    NumWhole,
    /// `a = a op b`, wrapping.
    Int(IntOp),
    /// `r = r + n`.
    AddImm(Reg, i32),
    /// `a = a << n`, or `a >> n` (arithmetic or logical), with `n` from 0 to 63.
    Shift(Shift, u8),
    /// Compares `a` with `b`, for the next `JumpIf` or `SetIf`: they must directly follow
    /// `Compare` (a `Line` marker may come between).
    Compare,
    /// Jumps when `a cond b`. Must directly follow `Compare` (a `Line` marker may come between).
    JumpIf(Cond, Label),
    /// Sets `a` to 1 when `a cond b`, else 0. Must directly follow `Compare` (a `Line` marker
    /// may come between).
    SetIf(Cond),
    Jump(Label),
    Mark(Label),
    /// The caller pushes the arguments left to right, then calls; the backend drops them
    /// (the callee's `params` says how many). The result is in `a`. `b` and the caller's
    /// slots survive the call; `a` does not.
    Call(String),
    /// Leaves the function with `a` as its result.
    Return,
    /// The first instruction of every function: reserves `n` zeroed 8-byte slots.
    Enter(u32),
    /// Placed just before a call: jumps to `fail` when the stack pointer minus `bytes` is
    /// below `Global::StackLimit` (unsigned), that is, when the callee's frame would not fit.
    StackCheck {
        bytes: u32,
        fail: Label,
    },
    System(Prim),
}

/// A function: `params` arguments, `slots` local slots, and its instructions.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: u32,
    pub slots: u32,
    pub body: Vec<Inst>,
}

/// A type descriptor the runtime uses to print values.
///
/// In memory (read-only), every field is 8 bytes. A descriptor starts with its kind: 0
/// number, 1 text, 2 truth, 3 record, 4 list. A record continues with the address of its
/// name text, its field count, then per field the address of the field-name text and of the
/// field's descriptor. A list continues with the address of its item descriptor. Values:
/// a text is the address of its 8-byte length, followed by the bytes; a list is the address
/// of a header `[count, capacity, items]` whose `items` is the address of `capacity` slots;
/// a record is one slot per declared field, in declaration order.
#[derive(Clone, Debug, PartialEq)]
pub enum Desc {
    Number,
    Text,
    Truth,
    /// Fields name other descriptors by index.
    Record {
        name: String,
        fields: Vec<(String, u32)>,
    },
    /// The element descriptor, by index.
    List(u32),
}

/// A whole program.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    /// The entry routine `_start` is among them.
    pub functions: Vec<Function>,
    /// Read-only text constants, by index.
    pub texts: Vec<Vec<u8>>,
    /// Type descriptors, by index.
    pub descs: Vec<Desc>,
}

impl Module {
    /// Adds a text constant, or finds it: the same bytes give the same index.
    pub fn text(&mut self, bytes: &[u8]) -> u32 {
        let index = match self.texts.iter().position(|t| t == bytes) {
            Some(index) => index,
            None => {
                self.texts.push(bytes.to_vec());
                self.texts.len() - 1
            }
        };
        index as u32
    }

    /// Adds a descriptor, or finds it: an equal descriptor gives the same index.
    pub fn desc(&mut self, d: Desc) -> u32 {
        let index = match self.descs.iter().position(|e| *e == d) {
            Some(index) => index,
            None => {
                self.descs.push(d);
                self.descs.len() - 1
            }
        };
        index as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_interned() {
        let mut m = Module::default();
        let a = m.text(b"a");
        assert_eq!(m.text(b"a"), a);
        let b = m.text(b"b");
        assert_ne!(a, b);
        assert_eq!(m.texts, vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn desc_is_interned() {
        let mut m = Module::default();
        let n = m.desc(Desc::Number);
        assert_eq!(m.desc(Desc::Number), n);
        let list = m.desc(Desc::List(n));
        assert_ne!(list, n);
        assert_eq!(m.desc(Desc::List(n)), list);
        assert_eq!(m.descs.len(), 2);
    }
}
