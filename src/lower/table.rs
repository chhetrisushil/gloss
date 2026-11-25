//! Tables in memory: `Global::Tables` holds the address of a block of two words per table,
//! in declaration order: the table's list, then its loaded flag.
use super::{Lowerer, Mode};
use crate::ast::program::Program;
use crate::diag::Diagnostic;
use crate::ir::{Cond, Data, Function, Global, Inst, Reg};

/// The size in bytes of one table's entry in the block.
const ENTRY: i32 = 16;
/// The suffix of the temporary file `save` writes beside the table.
const TEMP_SUFFIX: &str = ".gloss-new";

/// A table path split for `save`: the folder (`.` for the current one), the file name, and the
/// temporary name.
#[derive(Debug, PartialEq)]
pub struct SaveNames {
    pub folder: String,
    pub name: String,
    pub temp: String,
}

/// Splits a table path at its last `/`: the folder (`.` for the current one) and the name.
fn split_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or((".", path))
}

/// Holds what the runtime relies on, for `load` and `save` alike: the file name and the
/// temporary name `save` would use are one plain name each, and the folder is relative and has
/// no `..`. The checker already guarantees it; this keeps a checker change from reaching the
/// file system.
pub fn check_table_path(path: &str) -> Result<(), String> {
    let (folder, name) = split_path(path);
    let temp = format!("{name}{TEMP_SUFFIX}");
    let plain = |part: &str| !part.is_empty() && !part.contains(['/', '\0']);
    let dotted = |part: &str| part == "." || part == "..";
    let folder_ok = !path.contains('/')
        || (!folder.starts_with('/')
            && !folder.contains('\0')
            && folder.split('/').all(|c| !c.is_empty() && !dotted(c)));
    match plain(name) && !dotted(name) && plain(&temp) && folder_ok {
        true => Ok(()),
        false => Err(format!(
            "internal error: the table path {path:?} is not safe to use"
        )),
    }
}

/// The names `save` needs for a table path that `check_table_path` accepts.
pub fn save_names(path: &str) -> Result<SaveNames, String> {
    check_table_path(path)?;
    let (folder, name) = split_path(path);
    Ok(SaveNames {
        folder: folder.to_string(),
        name: name.to_string(),
        temp: format!("{name}{TEMP_SUFFIX}"),
    })
}

/// A table's list starts with room for this many rows, as `list of` does.
const ROOM: i64 = 4;

impl Lowerer<'_> {
    /// The index of the table `name`, when it names one; a variable never reuses a table's name.
    pub fn table_index(&self, name: &str) -> Option<usize> {
        self.program.tables().position(|t| t.name == name)
    }

    /// Puts the list of table `index` in `a`.
    pub fn load_table(&mut self, index: usize) {
        self.emit(Inst::AddressOf(Reg::A, Data::Global(Global::Tables)));
        for offset in [0, ENTRY * index as i32] {
            self.emit(Inst::Load {
                to: Reg::A,
                base: Reg::A,
                offset,
            });
        }
    }

    /// Makes the list in `slot` the table's and sets the loaded flag to 1.
    fn install_list(&mut self, index: usize, slot: u32) {
        self.emit(Inst::AddressOf(Reg::A, Data::Global(Global::Tables)));
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset: 0,
        });
        self.emit(Inst::LoadSlot(Reg::B, slot));
        let at = ENTRY * index as i32;
        self.emit(Inst::Store {
            base: Reg::A,
            offset: at,
            from: Reg::B,
        });
        self.emit(Inst::Const(Reg::B, 1));
        self.emit(Inst::Store {
            base: Reg::A,
            offset: at + 8,
            from: Reg::B,
        });
    }

    /// `clear t`: a new empty list replaces the table's (a list a caller copied earlier keeps
    /// its rows), and the loaded flag becomes 1.
    pub fn clear_table(&mut self, index: usize) {
        let fresh = self.new_temp();
        self.emit(Inst::Const(Reg::A, ROOM));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("list_new");
        self.emit(Inst::StoreSlot(fresh, Reg::A));
        self.install_list(index, fresh);
    }

    /// `load t`: `table_read` builds a new list from the file, with the path and its display
    /// form as compile-time texts. Only when it has read the whole file is the list stored as
    /// the table's, and the loaded flag set; a failure ends the program before that.
    pub fn read_table(&mut self, index: usize) -> Result<(), Diagnostic> {
        let table = self
            .program
            .tables()
            .nth(index)
            .expect("a table of that index");
        check_table_path(&table.path)
            .map_err(|message| Diagnostic::error(self.src, self.line, &message))?;
        self.push_text(&[table.path.as_bytes(), b"\0"].concat());
        self.push_text(crate::permissions::display_name(&table.path).as_bytes());
        let desc = self.record_desc(&table.record);
        self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("table_read");
        let fresh = self.new_temp();
        self.emit(Inst::StoreSlot(fresh, Reg::A));
        self.install_list(index, fresh);
        Ok(())
    }

    /// `save t`: failure 8 when the table's loaded flag is 0, then `table_save`, with the folder,
    /// file name, temporary name and display names as compile-time texts.
    pub fn save_table(&mut self, index: usize) -> Result<(), Diagnostic> {
        let table = self
            .program
            .tables()
            .nth(index)
            .expect("a table of that index");
        let names = save_names(&table.path)
            .map_err(|message| Diagnostic::error(self.src, self.line, &message))?;
        let unloaded = self.fail_label(
            8,
            "",
            &table.name,
            " is saved here but was not loaded or cleared first",
        );
        self.emit(Inst::AddressOf(Reg::A, Data::Global(Global::Tables)));
        for offset in [0, ENTRY * index as i32 + 8] {
            self.emit(Inst::Load {
                to: Reg::A,
                base: Reg::A,
                offset,
            });
        }
        self.emit(Inst::Const(Reg::B, 0));
        self.emit(Inst::Compare);
        self.emit(Inst::JumpIf(Cond::Eq, unloaded));
        self.load_table(index);
        self.emit(Inst::Push(Reg::A));
        let desc = self.record_desc(&table.record);
        self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
        self.emit(Inst::Push(Reg::A));
        let shown = |s: &str| crate::permissions::display_name(s).into_bytes();
        let nul = |s: &str| [s.as_bytes(), b"\0"].concat();
        for text in [
            nul(&names.folder),
            nul(&names.name),
            nul(&names.temp),
            shown(&table.path),
            shown(&names.folder),
        ] {
            self.push_text(&text);
        }
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("table_save");
        Ok(())
    }

    /// A slot holding the list `name` names: a variable's own slot, or a hidden one loaded
    /// from the table.
    pub fn list_slot(&mut self, name: &str) -> u32 {
        let Some(index) = self.table_index(name) else {
            return self.slots[name];
        };
        let slot = self.new_temp();
        self.load_table(index);
        self.emit(Inst::StoreSlot(slot, Reg::A));
        slot
    }
}

/// `tables_init`: allocates the block, gives each table a new empty list and a loaded flag of 0
/// (a fresh block is zeroed), or of 1 in a test build, where every table starts cleared.
/// A program with no tables allocates nothing. Slot 1 is the block.
pub fn tables_init(program: &Program, called: &mut Vec<String>, mode: Mode) -> Function {
    let n = program.tables().count() as i32;
    let mut body = vec![Inst::Enter(1)];
    if n > 0 {
        for name in ["alloc", "list_new"] {
            called.push(name.to_string());
        }
        let push = |body: &mut Vec<Inst>, value: i64| {
            body.push(Inst::Const(Reg::A, value));
            body.push(Inst::Push(Reg::A));
        };
        push(&mut body, i64::from(ENTRY * n));
        push(&mut body, 0);
        body.push(Inst::Call("alloc".to_string()));
        body.push(Inst::StoreSlot(1, Reg::A));
        for i in 0..n {
            push(&mut body, ROOM);
            push(&mut body, 0);
            body.push(Inst::Call("list_new".to_string()));
            body.push(Inst::LoadSlot(Reg::B, 1));
            body.push(Inst::Store {
                base: Reg::B,
                offset: ENTRY * i,
                from: Reg::A,
            });
            if mode == Mode::Test {
                body.push(Inst::Const(Reg::A, 1));
                body.push(Inst::Store {
                    base: Reg::B,
                    offset: ENTRY * i + 8,
                    from: Reg::A,
                });
            }
        }
        body.push(Inst::LoadSlot(Reg::A, 1));
        body.push(Inst::AddressOf(Reg::B, Data::Global(Global::Tables)));
        body.push(Inst::Store {
            base: Reg::B,
            offset: 0,
            from: Reg::A,
        });
    }
    body.push(Inst::Return);
    Function {
        name: "tables_init".to_string(),
        params: 0,
        slots: 1,
        body,
    }
}
