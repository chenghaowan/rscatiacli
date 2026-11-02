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

/// 生成属性读/写方法对（成员名与 CATIA/pycatia 一致；`set_xxx` 为 put）。
macro_rules! prop_pair {
    ($get:ident, $set:ident, $member:literal, f64) => {
        /// 属性读。
        pub fn $get(&self) -> $crate::error::ComResult<f64> {
            self.com().get($member)?.as_f64()
        }
        /// 属性写。
        pub fn $set(&self, value: f64) -> $crate::error::ComResult<()> {
            self.com().put($member, $crate::dispatch::Arg::F64(value))
        }
    };
    ($get:ident, $set:ident, $member:literal, i32) => {
        /// 属性读。
        pub fn $get(&self) -> $crate::error::ComResult<i32> {
            self.com().get($member)?.as_i32()
        }
        /// 属性写。
        pub fn $set(&self, value: i32) -> $crate::error::ComResult<()> {
            self.com().put($member, $crate::dispatch::Arg::I32(value))
        }
    };
    ($get:ident, $set:ident, $member:literal, bool) => {
        /// 属性读。
        pub fn $get(&self) -> $crate::error::ComResult<bool> {
            self.com().get($member)?.as_bool()
        }
        /// 属性写。
        pub fn $set(&self, value: bool) -> $crate::error::ComResult<()> {
            self.com().put($member, $crate::dispatch::Arg::Bool(value))
        }
    };
    ($get:ident, $set:ident, $member:literal, str) => {
        /// 属性读。
        pub fn $get(&self) -> $crate::error::ComResult<String> {
            self.com().get($member)?.as_string()
        }
        /// 属性写。
        pub fn $set(&self, value: &str) -> $crate::error::ComResult<()> {
            self.com()
                .put($member, $crate::dispatch::Arg::Str(value.to_string()))
        }
    };
    ($get:ident, $set:ident, $member:literal, ref_) => {
        /// 属性读（Reference）。
        pub fn $get(&self) -> $crate::error::ComResult<$crate::Reference> {
            Ok($crate::Reference::new(self.com().get($member)?.as_obj()?))
        }
        /// 属性写（Reference）。
        pub fn $set(&self, value: &$crate::Reference) -> $crate::error::ComResult<()> {
            self.com()
                .put($member, $crate::dispatch::Arg::Obj(value.com().clone()))
        }
    };
    ($get:ident, $set:ident, $member:literal, obj) => {
        /// 属性读（原始对象）。
        pub fn $get(&self) -> $crate::error::ComResult<$crate::dispatch::ComObj> {
            self.com().get($member)?.as_obj()
        }
        /// 属性写（原始对象）。
        pub fn $set(
            &self,
            value: &impl $crate::dispatch::ComObject,
        ) -> $crate::error::ComResult<()> {
            self.com()
                .put($member, $crate::dispatch::Arg::Obj(value.com().clone()))
        }
    };
    // 具名类型化对象属性（如 Point2D / Line2D / Limit；类型须从 objects 导出）。
    ($get:ident, $set:ident, $member:literal, $ty:ident) => {
        /// 属性读（类型化对象）。
        pub fn $get(&self) -> $crate::error::ComResult<$crate::objects::$ty> {
            Ok($crate::objects::$ty::new(
                self.com().get($member)?.as_obj()?,
            ))
        }
        /// 属性写（类型化对象）。
        pub fn $set(&self, value: &$crate::objects::$ty) -> $crate::error::ComResult<()> {
            self.com()
                .put($member, $crate::dispatch::Arg::Obj(value.com().clone()))
        }
    };
}

/// 生成只读属性方法。
macro_rules! prop_get {
    ($get:ident, $member:literal, f64) => {
        /// 属性读（只读）。
        pub fn $get(&self) -> $crate::error::ComResult<f64> {
            self.com().get($member)?.as_f64()
        }
    };
    ($get:ident, $member:literal, i32) => {
        /// 属性读（只读）。
        pub fn $get(&self) -> $crate::error::ComResult<i32> {
            self.com().get($member)?.as_i32()
        }
    };
    ($get:ident, $member:literal, str) => {
        /// 属性读（只读）。
        pub fn $get(&self) -> $crate::error::ComResult<String> {
            self.com().get($member)?.as_string()
        }
    };
    ($get:ident, $member:literal, ref_) => {
        /// 属性读（只读，Reference）。
        pub fn $get(&self) -> $crate::error::ComResult<$crate::Reference> {
            Ok($crate::Reference::new(self.com().get($member)?.as_obj()?))
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
pub mod in_interfaces;
pub mod inertia;
pub mod instance_factory;
pub mod knowledge;
pub mod material;
pub mod mec_mod;
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
pub use in_interfaces::{
    Camera, Camera2D, Camera3D, Cameras, DraftingPageSetup, File, FileComponent, FileSystem, Files,
    Folder, Folders, LightSource, LightSources, Move, PageSetup, Position, Printer, Printers,
    References, SelectionSets, SendToService, SpecsAndGeomWindow, SpecsViewer, SystemConfiguration,
    TextStream, Viewer, Viewer2D, Viewers, Viewpoint2D, Windows,
};
pub use inertia::Inertia;
pub use instance_factory::InstanceFactory;
pub use knowledge::{
    Angle, BoolParam, Check, ConstraintSatisfaction, DesignTable, Dimension, EnumParam,
    FeatureGenerator, Formula, FreeParameter, FreeParameters, IntParam, KnowledgeActivateObject,
    KnowledgeObject, Law, Length, List, ListParameter, Loop, Optimization, OptimizationConstraint,
    OptimizationConstraints, Optimizations, ParameterSet, ParameterSets, RealParam, Relation,
    Relations, Rule, SetOfEquation, StrParam, Unit, Units,
};
pub use material::{
    Material, MaterialDocument, MaterialFamilies, MaterialFamily, MaterialManager, Materials,
};
pub use mec_mod::{
    BiDimFeatEdge, Boundary, CylindricalFace, Edge, Face, Factory, FixTogether, FixTogethers,
    GeometricElements, HybridShape, HybridShapeInstance, MonoDimFeatEdge,
    NotWireBoundaryMonoDimFeatVertex, OrderedGeometricalSet, OrderedGeometricalSets, PartServices,
    PlanarFace, RectilinearBiDimFeatEdge, RectilinearMonoDimFeatEdge, RectilinearTriDimFeatEdge,
    Shape, ShapeInstance, Solid, TriDimFeatEdge, TriDimFeatVertexOrBiDimFeatVertex, Vertex,
    ZeroDimFeatVertexOrWireBoundaryMonoDimFeatVertex,
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
    Axis2D, Circle2D, Constraint, Constraints, ControlPoint2D, Curve2D, Ellipse2D, Factory2D,
    GeometricElement, Geometry2D, Hyperbola2D, Line2D, Parabola2D, Point2D, Sketch, Sketches,
    Spline2D, CAT_CST_TYPE_DISTANCE, CAT_CST_TYPE_HORONTALITY, CAT_CST_TYPE_LENGTH,
    CAT_CST_TYPE_PARALLELISM, CAT_CST_TYPE_VERTICALITY,
};
pub use spa_workbench::{Measurable, SPAWorkbench};
pub use system_service::SystemService;
pub use viewer::{Viewer3D, Viewpoint3D, Window};
