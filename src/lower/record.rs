//! Lowering records: building, reading a field and showing. A record is a block of one 8-byte
//! slot per field, in declaration order.
use super::Lowerer;
use crate::ast::program::RecordDecl;
use crate::ast::stmt::Target;
use crate::ast::{Expr, ExprKind, PlainType};
use crate::check::Ty;
use crate::diag::Diagnostic;
use crate::ir::{Data, Desc, Inst, Reg};

/// What a `set` or verb changes: a variable's slot, or one field of the record in it.
pub struct Place {
    pub slot: u32,
    pub field: Option<i32>,
}

impl<'a> Lowerer<'a> {
    fn record_decl(&self, name: &str) -> &'a RecordDecl {
        let program = self.program;
        let found = program.records().find(|r| r.name == name);
        found.expect("the checker knows every record")
    }

    /// The offset of `field` in a `record` block.
    pub fn field_offset(&self, record: &str, field: &str) -> i32 {
        let index = self
            .record_decl(record)
            .fields()
            .position(|f| f.name == field);
        8 * index.expect("the checker knows every field") as i32
    }

    /// The name of the record type of `e`, when it has one.
    pub fn record_type(&self, e: &Expr) -> Option<&'a str> {
        match self.types.expr_types.get(e.id.0 as usize) {
            Some(Some(Ty::Record(name))) => Some(self.record_decl(name).name.as_str()),
            _ => None,
        }
    }

    /// The descriptor of the record `name`, interned with its fields' descriptors.
    pub fn record_desc(&mut self, name: &str) -> u32 {
        let mut fields = Vec::new();
        for f in self.record_decl(name).fields() {
            let desc = match f.ty {
                PlainType::Number => Desc::Number,
                PlainType::Text => Desc::Text,
                PlainType::Truth => Desc::Truth,
            };
            fields.push((f.name.clone(), self.module.desc(desc)));
        }
        let name = name.to_string();
        self.module.desc(Desc::Record { name, fields })
    }

    /// `Record with f v, …`: allocates the block, then stores each value, in the order written.
    pub fn build_record(
        &mut self,
        record: &str,
        fields: &[(String, Expr)],
    ) -> Result<(), Diagnostic> {
        let block = self.new_temp();
        let size = 8 * self.record_decl(record).fields().count();
        self.emit(Inst::Const(Reg::A, size as i64));
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::Const(Reg::A, i64::from(self.line)));
        self.emit(Inst::Push(Reg::A));
        self.call("alloc");
        self.emit(Inst::StoreSlot(block, Reg::A));
        for (field, value) in fields {
            self.expr(value)?;
            self.emit(Inst::Move {
                to: Reg::B,
                from: Reg::A,
            });
            self.emit(Inst::LoadSlot(Reg::A, block));
            self.emit(Inst::Store {
                base: Reg::A,
                offset: self.field_offset(record, field),
                from: Reg::B,
            });
        }
        self.emit(Inst::LoadSlot(Reg::A, block));
        Ok(())
    }

    /// `field of record`.
    pub fn field_of(&mut self, field: &str, of: &Expr) -> Result<(), Diagnostic> {
        let record = self
            .record_type(of)
            .expect("the checker knows a record here");
        self.expr(of)?;
        let offset = self.field_offset(record, field);
        self.emit(Inst::Load {
            to: Reg::A,
            base: Reg::A,
            offset,
        });
        Ok(())
    }

    /// Shows the record in `a`.
    pub fn show_record(&mut self, record: &str) {
        let desc = self.record_desc(record);
        self.emit(Inst::Push(Reg::A));
        self.emit(Inst::AddressOf(Reg::A, Data::Desc(desc)));
        self.emit(Inst::Push(Reg::A));
        self.call("show_value");
    }

    /// Whether `e` builds a record itself, which `set` stores without a copy.
    pub fn builds_record(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::BuildRecord { .. } => true,
            ExprKind::Paren(inner) => Self::builds_record(inner),
            _ => false,
        }
    }

    /// The place `target` names: a variable, or a field of a record variable.
    pub fn place(&self, target: &Target) -> Option<Place> {
        match target {
            Target::Var(name) => Some(Place {
                slot: self.slots[name],
                field: None,
            }),
            Target::Field { field, of } => {
                let Target::Var(name) = &**of else {
                    return None;
                };
                let Some(Ty::Record(record)) = self.vars.get(name) else {
                    return None;
                };
                Some(Place {
                    slot: self.slots[name],
                    field: Some(self.field_offset(record, field)),
                })
            }
            Target::Item { .. } => None,
        }
    }

    /// Stores the value in `a` into `place`.
    pub fn store_place(&mut self, place: &Place) {
        let Some(offset) = place.field else {
            self.emit(Inst::StoreSlot(place.slot, Reg::A));
            return;
        };
        self.emit(Inst::Move {
            to: Reg::B,
            from: Reg::A,
        });
        self.emit(Inst::LoadSlot(Reg::A, place.slot));
        self.emit(Inst::Store {
            base: Reg::A,
            offset,
            from: Reg::B,
        });
    }
}
