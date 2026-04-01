use crate::ast::{CiprType, NodeId};
use crate::type_checker::TypeChecker;

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_array(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let mut elem_type = CiprType::Unknown;

        for c in children.iter().flatten() {
            let t = self.check(*c);
            if elem_type == CiprType::Unknown {
                elem_type = t;
            } else if t != CiprType::Unknown && t != elem_type {
                self.error(
                    self.arena[id].token.line,
                    "Array elements must have the same type.",
                );
            }
        }

        CiprType::Array(Box::new(elem_type))
    }

    pub(crate) fn check_index_get(&mut self, id: NodeId) -> CiprType {
        let children = self.arena[id].children.clone();
        let line = self.arena[id].token.line;

        let target_type = self.check_child(children[0]);
        let index_type = self.check_child(children[1]);

        if index_type != CiprType::Int && index_type != CiprType::Unknown {
            self.error(line, "Array index must be an Int.");
        }

        match target_type {
            CiprType::Array(inner) => *inner,
            CiprType::Unknown => CiprType::Unknown,
            _ => {
                self.error(line, "Only arrays can be indexed.");
                CiprType::Unknown
            }
        }
    }
}
