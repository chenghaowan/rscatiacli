//! `Part` 及其下的 Body / 几何图形集 / 轴系 / 基准元素。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject, FromComObj};
use crate::error::ComResult;

use super::hybrid_shape_factory::HybridShapeFactory;
use super::reference::Reference;

com_wrapper!(Part);
com_wrapper!(Bodies);
com_wrapper!(Body);
com_wrapper!(HybridBodies);
com_wrapper!(HybridBody);
com_wrapper!(AxisSystems);
com_wrapper!(AxisSystem);
com_wrapper!(OriginElements);
com_wrapper!(Entities);
com_wrapper!(Entity);

/// 仅需要名字/父级的元素集合（Shapes / Sketches / HybridShapes / Constraints）。
pub type Shapes = Entities;
pub type Sketches = Entities;
pub type HybridShapes = Entities;
pub type Constraints = Entities;

impl ComCollection for Bodies {
    type Item = Body;
}

impl ComCollection for HybridBodies {
    type Item = HybridBody;
}

impl HybridBodies {
    /// 新建几何图形集（`Add` 无参，返回新节点）。
    pub fn add(&self) -> ComResult<HybridBody> {
        Ok(HybridBody::new(self.com().call("Add", &[])?.as_obj()?))
    }
}

impl ComCollection for AxisSystems {
    type Item = AxisSystem;
}

impl ComCollection for Entities {
    type Item = Entity;
}

/// 取对象的子集合属性，例如 `Part.Bodies`。
fn child<T: FromComObj>(obj: &ComObj, property: &str) -> ComResult<T> {
    Ok(T::from_com(obj.get(property)?.as_obj()?))
}

impl Part {
    pub fn bodies(&self) -> ComResult<Bodies> {
        child(self.com(), "Bodies")
    }

    pub fn hybrid_bodies(&self) -> ComResult<HybridBodies> {
        child(self.com(), "HybridBodies")
    }

    pub fn axis_systems(&self) -> ComResult<AxisSystems> {
        child(self.com(), "AxisSystems")
    }

    /// 约束集合（`Search` 不可用时的兜底枚举）。
    pub fn constraints(&self) -> ComResult<Entities> {
        child(self.com(), "Constraints")
    }

    /// 参考坐标系三平面（CATIA 的 Part 没有 ReferenceElements）。
    pub fn origin_elements(&self) -> ComResult<OriginElements> {
        child(self.com(), "OriginElements")
    }

    pub fn update(&self) -> ComResult<()> {
        self.com().call("Update", &[]).map(|_| ())
    }

    /// 把指定 Body 设为 PartBody（`MainBody`）。
    pub fn set_main_body(&self, body: &Body) -> ComResult<()> {
        self.com().put("MainBody", Arg::Obj(body.com().clone()))
    }

    /// 主 Body（PartBody）。
    pub fn main_body(&self) -> ComResult<Body> {
        Ok(Body::new(self.com().get("MainBody")?.as_obj()?))
    }

    /// 为对象创建 `Reference`（datum 化 / 测量的入口）。
    pub fn create_reference_from_object(&self, object: &impl ComObject) -> ComResult<Reference> {
        Ok(Reference::new(
            self.com()
                .call(
                    "CreateReferenceFromObject",
                    &[Arg::Obj(object.com().clone())],
                )?
                .as_obj()?,
        ))
    }

    /// 几何 datum 工厂。
    pub fn hybrid_shape_factory(&self) -> ComResult<HybridShapeFactory> {
        child(self.com(), "HybridShapeFactory")
    }
}

impl Body {
    pub fn shapes(&self) -> ComResult<Entities> {
        child(self.com(), "Shapes")
    }

    pub fn sketches(&self) -> ComResult<Entities> {
        child(self.com(), "Sketches")
    }
}

impl HybridBody {
    pub fn hybrid_shapes(&self) -> ComResult<Entities> {
        child(self.com(), "HybridShapes")
    }

    /// 把 datum 追加进本图形集。
    pub fn append_hybrid_shape(&self, shape: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("AppendHybridShape", &[Arg::Obj(shape.com().clone())])
            .map(|_| ())
    }
}

impl OriginElements {
    pub fn plane_xy(&self) -> ComResult<Entity> {
        child(self.com(), "PlaneXY")
    }

    pub fn plane_yz(&self) -> ComResult<Entity> {
        child(self.com(), "PlaneYZ")
    }

    pub fn plane_zx(&self) -> ComResult<Entity> {
        child(self.com(), "PlaneZX")
    }
}
