//! `Part` 及其下的 Body / 几何图形集 / 轴系 / 基准元素。

use crate::collections::ComCollection;
use crate::dispatch::{Arg, ComObj, ComObject, FromComObj};
use crate::error::ComResult;

use super::hybrid_shape_factory::HybridShapeFactory;
use super::reference::Reference;
use super::shape_factory::ShapeFactory;
use super::system_service::SystemService;

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

/// 仅需要名字/父级的元素集合（Shapes / HybridShapes）。
pub type Shapes = Entities;
pub type HybridShapes = Entities;

impl ComCollection for Bodies {
    type Item = Body;
}

impl Bodies {
    /// 新建实体 Body（包围盒集合体）。
    pub fn add(&self) -> ComResult<Body> {
        Ok(Body::new(self.com().call("Add", &[])?.as_obj()?))
    }
}

impl ComCollection for HybridBodies {
    type Item = HybridBody;
}

impl HybridBodies {
    /// 新建几何图形集（`Add` 无参，返回新节点）。
    pub fn add(&self) -> ComResult<HybridBody> {
        Ok(HybridBody::new(self.com().call("Add", &[])?.as_obj()?))
    }

    /// 按名字取几何图形集（`Item("definition_points")`，包围盒定点集）。
    pub fn item_by_name(&self, name: &str) -> ComResult<HybridBody> {
        Ok(HybridBody::new(
            self.com()
                .call("Item", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }
}

impl ComCollection for AxisSystems {
    type Item = AxisSystem;
}

impl ComCollection for Entities {
    type Item = Entity;
}

impl Entities {
    /// 按名字取元素（`Item("AbsoluteAxis")` 取草图绝对轴）。
    pub fn item_by_name(&self, name: &str) -> ComResult<Entity> {
        Ok(Entity::new(
            self.com()
                .call("Item", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
    }
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

    /// 零件级约束集合（包围盒 dz 长度约束挂这里）。
    pub fn constraints(&self) -> ComResult<crate::objects::sketch::Constraints> {
        child(self.com(), "Constraints")
    }

    /// 知识工程参数集合（Bbox_dx/dy/dz）。
    pub fn parameters(&self) -> ComResult<crate::objects::parameter::Parameters> {
        child(self.com(), "Parameters")
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

    /// 按 BRep 名字创建引用（如轴系 XY 面
    /// `Selection_RSur:(Face:(Brp:(<名>;1);None:());<名>)`，包围盒 sketch 载体）。
    pub fn create_reference_from_name(&self, name: &str) -> ComResult<Reference> {
        Ok(Reference::new(
            self.com()
                .call("CreateReferenceFromName", &[Arg::Str(name.to_string())])?
                .as_obj()?,
        ))
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

    /// Part Design 实体特征工厂（AddNewSplit 等）。
    pub fn shape_factory(&self) -> ComResult<ShapeFactory> {
        child(self.com(), "ShapeFactory")
    }

    /// 设定当前工作对象（新特征插入的位置）。
    pub fn set_in_work_object(&self, object: &impl ComObject) -> ComResult<()> {
        self.com()
            .put("InWorkObject", Arg::Obj(object.com().clone()))
    }

    /// 只更新单个对象（批量偏置/裁剪时避免整树重算）。
    pub fn update_object(&self, object: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("UpdateObject", &[Arg::Obj(object.com().clone())])
            .map(|_| ())
    }

    /// Create Datum（datum 化去参的开关）。
    pub fn set_create_datum(&self, enabled: bool) -> ComResult<()> {
        self.com().put("CreateDatum", Arg::Bool(enabled))
    }

    /// No Keep Mode（datum 化后不保留原特征）。
    pub fn set_no_keep_mode(&self, enabled: bool) -> ComResult<()> {
        self.com().put("NoKeepMode", Arg::Bool(enabled))
    }
}

impl Body {
    /// Body 内的几何图形集（直线可能挂在这里）。
    pub fn hybrid_bodies(&self) -> ComResult<HybridBodies> {
        child(self.com(), "HybridBodies")
    }

    pub fn shapes(&self) -> ComResult<Entities> {
        child(self.com(), "Shapes")
    }

    pub fn sketches(&self) -> ComResult<Entities> {
        child(self.com(), "Sketches")
    }

    /// Body 内的几何图形（曲面可挂在 Body 下）。
    pub fn hybrid_shapes(&self) -> ComResult<Entities> {
        child(self.com(), "HybridShapes")
    }

    /// 是否处于布尔运算中（这类 Body 不可直接 Split）。
    pub fn in_boolean_operation(&self) -> ComResult<bool> {
        self.com().get("InBooleanOperation")?.as_bool()
    }

    /// 把 datum 插入 PartDesign Body（几何图形集用 append_hybrid_shape）。
    pub fn insert_hybrid_shape(&self, shape: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("InsertHybridShape", &[Arg::Obj(shape.com().clone())])
            .map(|_| ())
    }
}

impl HybridBody {
    pub fn hybrid_shapes(&self) -> ComResult<Entities> {
        child(self.com(), "HybridShapes")
    }

    /// 子几何图形集（嵌套遍历用）。
    pub fn hybrid_bodies(&self) -> ComResult<HybridBodies> {
        child(self.com(), "HybridBodies")
    }

    /// Body 内的草图集合（包围盒 sketch 建在这里）。
    pub fn hybrid_sketches(&self) -> ComResult<crate::objects::sketch::Sketches> {
        child(self.com(), "HybridSketches")
    }

    /// 把 datum 追加进本图形集。
    pub fn append_hybrid_shape(&self, shape: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("AppendHybridShape", &[Arg::Obj(shape.com().clone())])
            .map(|_| ())
    }

    /// 从集合中移除特征。部分 CATIA 版本不支持该成员（join_surface 的 Python
    /// 版同样 try/except 容忍失败），调用方应按可失败处理。
    pub fn remove(&self, shape: &impl ComObject) -> ComResult<()> {
        self.com()
            .call("Remove", &[Arg::Obj(shape.com().clone())])
            .map(|_| ())
    }
}

impl AxisSystems {
    /// 新建轴系（工具ing axis 共享基准）。
    pub fn add(&self) -> ComResult<AxisSystem> {
        Ok(AxisSystem::new(self.com().call("Add", &[])?.as_obj()?))
    }
}

impl AxisSystem {
    /// 原点定义方式：catAxisSystemOriginByCoordinates = 1（去参基准）。
    pub fn set_origin_type(&self, value: i32) -> ComResult<()> {
        self.com().put("OriginType", Arg::I32(value))
    }

    pub fn origin_type(&self) -> ComResult<i32> {
        self.com().get("OriginType")?.as_i32()
    }

    /// 轴定义方式：catAxisSystemAxisByCoordinates = 1（恒定平行）。
    pub fn set_x_axis_type(&self, value: i32) -> ComResult<()> {
        self.com().put("XAxisType", Arg::I32(value))
    }

    pub fn set_y_axis_type(&self, value: i32) -> ComResult<()> {
        self.com().put("YAxisType", Arg::I32(value))
    }

    pub fn set_z_axis_type(&self, value: i32) -> ComResult<()> {
        self.com().put("ZAxisType", Arg::I32(value))
    }

    /// 设为当前轴系。
    pub fn set_is_current(&self, value: bool) -> ComResult<()> {
        self.com().put("IsCurrent", Arg::Bool(value))
    }

    /// 写原点坐标（PutOrigin，SAFEARRAY 入参）。
    pub fn put_origin(&self, origin: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutOrigin",
                &[Arg::SafeArray(
                    origin.iter().map(|v| Arg::F64(*v)).collect(),
                )],
            )
            .map(|_| ())
    }

    pub fn put_x_axis(&self, axis: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutXAxis",
                &[Arg::SafeArray(axis.iter().map(|v| Arg::F64(*v)).collect())],
            )
            .map(|_| ())
    }

    pub fn put_y_axis(&self, axis: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutYAxis",
                &[Arg::SafeArray(axis.iter().map(|v| Arg::F64(*v)).collect())],
            )
            .map(|_| ())
    }

    pub fn put_z_axis(&self, axis: [f64; 3]) -> ComResult<()> {
        self.com()
            .call(
                "PutZAxis",
                &[Arg::SafeArray(axis.iter().map(|v| Arg::F64(*v)).collect())],
            )
            .map(|_| ())
    }

    fn axis_triple(&self, ss: &SystemService, member: &str) -> ComResult<[f64; 3]> {
        let function = "rscatiacli_axis_triple";
        let code = format!(
            r#"
        Public Function {function}(axis_system)
            Dim aReturn(2) as Variant
            axis_system.{member} aReturn
            {function} = aReturn
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(crate::error::ComError::runtime(format!(
                "{member} 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
    }

    /// X 轴方向（VBA 中转，出参数组）。
    pub fn get_x_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.axis_triple(ss, "GetXAxis")
    }

    pub fn get_y_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.axis_triple(ss, "GetYAxis")
    }

    pub fn get_z_axis(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        self.axis_triple(ss, "GetZAxis")
    }

    /// 轴系原点（模型坐标 mm）。`GetOrigin` 是 SAFEARRAY 出参，经 VBA 中转取回
    /// （与 pycatia axis_system.get_origin 同一条实测路径）。
    pub fn get_origin(&self, ss: &SystemService) -> ComResult<[f64; 3]> {
        let function = "rscatiacli_axis_origin";
        let code = format!(
            r#"
        Public Function {function}(axis_system)
            Dim oOrigin(2)
            axis_system.GetOrigin oOrigin
            {function} = oOrigin
        End Function
        "#
        );
        let values = ss
            .evaluate(&code, 0, function, &[Arg::Obj(self.com().clone())])?
            .as_f64_array()?;
        if values.len() < 3 {
            return Err(crate::error::ComError::runtime(format!(
                "AxisSystem.GetOrigin 返回了 {} 个分量，期望 3",
                values.len()
            )));
        }
        Ok([values[0], values[1], values[2]])
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
