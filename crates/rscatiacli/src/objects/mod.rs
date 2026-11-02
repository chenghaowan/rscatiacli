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

pub mod analyze;
pub mod annotation;
pub mod application;
pub mod assembly_convertor;
pub mod distances;
pub mod documents;
pub mod drafting;
pub mod groups;
pub mod hybrid_shape_factory;
pub mod hybrid_shapes;
pub mod inertia;
pub mod instance_factory;
pub mod material;
pub mod parameter;
pub mod part;
pub mod part_interfaces;
pub mod plane;
pub mod product;
pub mod publication;
pub mod reference;
pub mod selection;
pub mod shape_factory;
pub mod sketch;
pub mod spa_workbench;
pub mod system_service;
pub mod viewer;

pub use analyze::Analyze;
pub use annotation::{
    Annotation, AnnotationFactory, AnnotationSets, AnnotationText, Annotations, UserSurface,
    UserSurfaces,
};
pub use application::Application;
pub use assembly_convertor::AssemblyConvertor;
pub use distances::{Distance, Distances, CAT_DISTANCE_BETWEEN_TWO, CAT_DISTANCE_MEASURE_MINIMUM};
pub use documents::{Document, Documents, PartDocument, ProductDocument};
pub use drafting::{
    DrawingDocument, DrawingSheet, DrawingSheets, DrawingText, DrawingTexts, DrawingView,
    DrawingViewGenerativeBehavior, DrawingViews, PAPER_A0, PAPER_A1, PAPER_A2, PAPER_A3, PAPER_A4,
};
pub use groups::{Group, Groups};
pub use hybrid_shape_factory::HybridShapeFactory;
pub use hybrid_shapes::{
    Extract, HybridShapeAffinity, HybridShapeJoin, HybridShapeLinePtPt, HybridShapeOffset,
    HybridShapeTranslate,
};
pub use inertia::Inertia;
pub use instance_factory::InstanceFactory;
pub use material::{
    Material, MaterialDocument, MaterialFamilies, MaterialFamily, MaterialManager, Materials,
};
pub use parameter::{Parameter, Parameters};
pub use part::{
    AxisSystem, AxisSystems, Bodies, Body, Entities, Entity, HybridBodies, HybridBody,
    HybridShapes, OriginElements, Part, Planes, Shapes,
};
pub use part_interfaces::{
    Add, Affinity, AngularRepartition, Assemble, AutoDraft, AutoFillet, AxisToAxis, BooleanShape,
    Chamfer, CircPattern, CloseSurface, ConstRadEdgeFillet, Defeaturing, DefeaturingFilletFilter,
    DefeaturingFilter, DefeaturingFilterWithRange, DefeaturingFilters, DefeaturingHoleFilter,
    Draft, DraftDomain, DraftDomains, DressUpShape, EdgeFillet, FaceFillet, Fillet, Groove, Hole,
    Intersect, Limit, LinearRepartition, Loft, Mirror, Pad, Pattern, Pocket, Prism, RectPattern,
    Remove, RemoveFace, Repartition, ReplaceFace, Revolution, Rib, Rotate, Scaling, Scaling2,
    SewSurface, Shaft, Shell, SketchBasedShape, Slot, SolidCombine, Split, Stiffener,
    SurfaceBasedShape, Sweep, Symmetry, ThickSurface, Thickness, Thread, TransformationShape,
    Translate, Trim, TritangentFillet, UserPattern, UserRepartition, VarRadEdgeFillet,
};
pub use plane::Plane;
pub use product::{Product, Products};
pub use publication::{Publication, Publications};
pub use reference::Reference;
pub use selection::{Selection, SelectionItem, VisProperties};
pub use shape_factory::ShapeFactory;
pub use sketch::{
    Constraint, Constraints, Factory2D, Line2D, Point2D, Sketch, Sketches, CAT_CST_TYPE_DISTANCE,
    CAT_CST_TYPE_HORONTALITY, CAT_CST_TYPE_LENGTH, CAT_CST_TYPE_PARALLELISM,
    CAT_CST_TYPE_VERTICALITY,
};
pub use spa_workbench::{Measurable, SPAWorkbench};
pub use system_service::SystemService;
pub use viewer::{Viewer3D, Viewpoint3D, Window};
