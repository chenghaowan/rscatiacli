//! CATIA 对象模型的类型化封装。
//!
//! 命名与 pycatia 对齐（Application / Documents / Document / Part / Bodies / Body /
//! HybridBodies / HybridBody / AxisSystems / OriginElements / Product / Products /
//! Selection / VisProperties），但只覆盖当前工具实际用到的成员。
//! 未封装的成员通过 `ComObject::com()` 取回原始对象继续派发。

/// 为 CATIA 对象生成零成本 newtype 包装：`new`、`ComObject`、`FromComObj`。
macro_rules! com_wrapper {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $name($crate::dispatch::ComObj);

        impl $name {
            pub fn new(obj: $crate::dispatch::ComObj) -> Self {
                Self(obj)
            }
        }

        impl $crate::dispatch::ComObject for $name {
            fn com(&self) -> &$crate::dispatch::ComObj {
                &self.0
            }
        }

        impl $crate::dispatch::FromComObj for $name {
            fn from_com(obj: $crate::dispatch::ComObj) -> Self {
                Self(obj)
            }
        }
    };
}

pub mod application;
pub mod documents;
pub mod hybrid_shape_factory;
pub mod part;
pub mod product;
pub mod publication;
pub mod reference;
pub mod selection;
pub mod spa_workbench;

pub use application::Application;
pub use documents::{Document, Documents, PartDocument, ProductDocument};
pub use hybrid_shape_factory::HybridShapeFactory;
pub use part::{
    AxisSystem, AxisSystems, Bodies, Body, Constraints, Entities, Entity, HybridBodies, HybridBody,
    HybridShapes, OriginElements, Part, Shapes, Sketches,
};
pub use product::{Product, Products};
pub use publication::{Publication, Publications};
pub use reference::Reference;
pub use selection::{Selection, SelectionItem, VisProperties};
pub use spa_workbench::{Measurable, SPAWorkbench};
